// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_apply(modifier='', report=False, merge_customdata=True,
//! single_user=False, all_keyframes=False, use_selected_objects=False)`, tier T2 (the result
//! is the modifier's evaluator). Port of `modifier_apply_poll`, `modifier_apply_exec_ex`,
//! `ed::object::modifier_apply`, `modifier_apply_obdata` and `create_applied_mesh_for_modifier`
//! (`editors/object/object_modifier.cc`) and `BKE_mesh_merge_customdata_for_apply_modifier`
//! (`blenkernel/intern/mesh_merge_customdata.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - the modifier is evaluated **alone on the original mesh** (not on the stack before it),
//!   with viewport settings, even when `show_viewport` is off; applying one that is not first
//!   adds the INFO `Applied modifier was not first, result may not be as expected`;
//! - data used by several objects: `RuntimeError: Error: Modifiers cannot be applied to
//!   multi-user data` (nothing changes); `single_user=True` first gives the object its own copy
//!   (`Cube.001`); users are `ID_REAL_USERS`: a fake user does not make the data multi-user
//!   (it applies, and `single_user` makes no copy);
//! - a disabled modifier (a NODES modifier without a group): `RuntimeError: Error: Modifier is
//!   disabled, skipping apply`, the modifier stays;
//! - in Edit Mode the poll fails: `This modifier operation is not allowed from Edit mode`; an
//!   active empty: `Modifiers cannot be applied on empty object type`;
//! - an unknown or empty `modifier` is `CANCELLED` without a message; `report=True` adds
//!   `Applied modifier: <name>` when nothing else was reported;
//! - a mesh with shape keys refuses every non-`NonGeometrical` modifier (`Modifier cannot be
//!   applied to a mesh with shape keys`);
//! - `merge_customdata` merges, per vertex, UVs that a constructive modifier left within 12
//!   ULPs of each other.
//!
//! A modifier frap cannot evaluate (no evaluator for its type, a node group other than Smooth
//! by Angle, an unsupported input) answers `OP_NOT_IMPLEMENTED`. With several objects
//! (`use_selected_objects`) a failure rolls the whole statement back (deviations.md D-85).

use frap_eval::EvalCache;
use frap_mesh::{AttrData, Domain, Mesh};
use frap_modifiers::stack::{self, edit};
use frap_modifiers::{ModKind, ModifierType, registry};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdCode, IdRef, MeshId, ObjectId};
use frap_scene::{ChangeLog, Context, Main, ObjectMode};
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::modifier_add::shared;

fn apply_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, "modifier_apply", false)?;
    match poll::active(c) {
        Some((_, ob)) if ob.object_type() == "EMPTY" => Err(PollFail {
            message: Some(shared::poll_message(
                "modifier_apply",
                "Modifiers cannot be applied on empty object type",
            )),
            unmet: smallvec![PollReq::ActiveType("MESH")],
            hint: None,
        }),
        _ => Ok(()),
    }
}

/// `modifier_apply_poll`.
const APPLY_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: apply_poll,
};

/// `compare_ff_relative(a, b, max_diff, max_ulps)`.
fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: u32) -> bool {
    if (a - b).abs() <= max_diff {
        return true;
    }
    ulp_diff_ff(a, b) <= max_ulps
}

/// `ulp_diff_ff`.
fn ulp_diff_ff(a: f32, b: f32) -> u32 {
    let (ai, bi) = (a.to_bits(), b.to_bits());
    let sign = 0x8000_0000u32;
    let infinity = 0x7f80_0000u32;
    let (a_sign, b_sign) = (ai & sign, bi & sign);
    let (a_abs, b_abs) = (ai & !sign, bi & !sign);
    if a_abs > infinity || b_abs > infinity {
        return 0xffff_ffff;
    }
    if a_sign == b_sign {
        a_abs.max(b_abs) - a_abs.min(b_abs)
    } else {
        a_abs.wrapping_add(b_abs)
    }
}

#[derive(PartialEq)]
enum Cmp {
    Close,
    Equal,
    Apart,
}

/// `compare_v2_classify`.
fn compare_v2_classify(a: [f32; 2], b: [f32; 2]) -> Cmp {
    if a == b {
        return Cmp::Equal;
    }
    if compare_ff_relative(a[0], b[0], 1e-12, 12) && compare_ff_relative(a[1], b[1], 1e-12, 12) {
        return Cmp::Close;
    }
    Cmp::Apart
}

/// `merge_uvs_for_vertex`.
fn merge_uvs_for_vertex(loops: &[usize], uv: &mut [[f32; 2]]) {
    if loops.len() <= 1 {
        return;
    }
    let mut merge: Vec<usize> = loops.to_vec();
    while merge.len() > 1 {
        let mut i_last = merge.len() - 1;
        let src = uv[merge[0]];
        let mut i = 1;
        while i <= i_last {
            match compare_v2_classify(src, uv[merge[i]]) {
                Cmp::Apart => i += 1,
                c => {
                    if c == Cmp::Close {
                        uv[merge[i]] = src;
                    }
                    merge[i] = merge[i_last];
                    i_last -= 1;
                }
            }
        }
        merge[0] = merge[i_last];
        merge.truncate(i_last);
    }
}

/// `BKE_mesh_merge_customdata_for_apply_modifier`: for every FLOAT2 corner attribute (UV
/// maps), UVs of one vertex that are equal or within 12 ULPs are made identical.
pub(crate) fn merge_customdata_for_apply_modifier(mesh: &mut Mesh) {
    if mesh.corners_num() == 0 {
        return;
    }
    let names: Vec<_> = mesh
        .attributes()
        .iter()
        .filter(|a| a.domain == Domain::Corner && matches!(a.data, AttrData::Float2(_)))
        .map(|a| a.name.clone())
        .collect();
    if names.is_empty() {
        return;
    }
    // `vert_to_corner_map`: corners per vertex, ascending.
    let mut vert_corners: Vec<Vec<usize>> = vec![Vec::new(); mesh.verts_num()];
    for (c, &v) in mesh.corner_verts().iter().enumerate() {
        if let Some(l) = vert_corners.get_mut(v as usize) {
            l.push(c);
        }
    }
    for name in names {
        let Some(attr) = mesh.attributes_mut().get_mut(&name) else {
            continue;
        };
        let AttrData::Float2(v) = &mut attr.data else {
            continue;
        };
        let uv = std::sync::Arc::make_mut(v);
        for loops in &vert_corners {
            merge_uvs_for_vertex(loops, uv);
        }
    }
}

/// `single_obdata_user_make` for a mesh: the object gets its own copy (named like
/// `ID.copy()`).
fn single_user(main: &mut Main, log: &mut ChangeLog, ob: IdRef<ObjectId>, me: IdRef<MeshId>) {
    let Some(mut copy) = main.meshes.get(me).cloned() else {
        return;
    };
    let name = copy.header.name.clone();
    copy.header.use_fake_user = false;
    copy.header.library = None;
    let new = main.meshes.insert(&name, copy, log);
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.data = Some(new.any());
    }
}

/// `ID_REAL_USERS`: the users of `me`, the fake user not counted.
fn real_users(main: &Main, me: IdRef<MeshId>) -> u32 {
    let fake = main.meshes.get(me).is_some_and(|m| m.header.use_fake_user);
    main.users(me.any()).saturating_sub(u32::from(fake))
}

/// The object's mesh, when its data is a mesh.
fn mesh_of(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    let d: IdAny = main.objects.get(ob)?.data?;
    (d.code == IdCode::ME).then(|| d.typed()).flatten()
}

#[operator(
    poll = APPLY_POLL,
    tier = T2,
    support = Partial(
        "meshes only; a type without a frap evaluator cannot be applied (NotImplementedError); all_keyframes is for grease pencil"
    )
)]
fn exec(cx: &mut OpCtx, p: object::ModifierApply) -> OpResult {
    let name = p.modifier.value.clone();
    let objects = shared::edit_objects(&cx.ctx(), p.use_selected_objects.value);
    let reports_before = 0usize;
    let mut changed = false;
    let mut errors: Vec<String> = Vec::new();
    let mut reported = reports_before;
    for ob in objects {
        let Some((i, ty, in_edit)) = cx.main().objects.get(ob).and_then(|o| {
            shared::find(o, &name).map(|i| {
                (
                    i,
                    ModifierType::from_identifier(o.modifiers[i].settings.type_identifier()),
                    o.mode == ObjectMode::Edit,
                )
            })
        }) else {
            continue;
        };
        let kind = ty.map(|t| registry::meta(t).kind);
        // `single_user`: make the data single-user first.
        if p.single_user.value
            && let Some(me) = mesh_of(cx.main(), ob)
            && real_users(cx.main(), me) > 1
        {
            let (main, log) = cx.main_mut();
            single_user(main, log, ob, me);
        }
        // `ed::object::modifier_apply`.
        if in_edit {
            errors.push("Modifiers cannot be applied in edit mode".into());
            continue;
        }
        let Some(me) = mesh_of(cx.main(), ob) else {
            errors.push("Cannot apply modifier for this object type".into());
            changed = true;
            continue;
        };
        if real_users(cx.main(), me) > 1 {
            errors.push("Modifiers cannot be applied to multi-user data".into());
            continue;
        }
        if i != 0 {
            cx.report(
                ReportType::Info,
                "Applied modifier was not first, result may not be as expected",
            );
            reported += 1;
        }
        changed = true;
        // `modifier_apply_obdata`.
        let has_shape_keys = cx
            .main()
            .meshes
            .get(me)
            .is_some_and(|m| m.shape_keys.is_some());
        let disabled = {
            let o = cx.main().objects.get(ob).expect("invariant: found above");
            let m = &o.modifiers[i];
            ty.and_then(registry::find)
                .and_then(|info| (info.is_disabled)(&m.settings))
                .is_some()
        };
        if disabled {
            errors.push("Modifier is disabled, skipping apply".into());
            continue;
        }
        if has_shape_keys && kind != Some(ModKind::NonGeometrical) {
            errors.push("Modifier cannot be applied to a mesh with shape keys".into());
            continue;
        }
        let applied = {
            let (host, _) = cx.split_eval();
            EvalCache::new().apply_modifier(host, ob, i)
        };
        let applied = match applied {
            Ok(a) => a,
            // A frap limitation (no evaluator, a node group frap cannot evaluate, an input it
            // does not support) is not Blender's error.
            Err(frap_modifiers::ModError::Error(m)) if stack::is_frap_limitation(&m) => {
                return Err(OpError::NotImplemented(if m.starts_with("frap:") {
                    m
                } else {
                    format!("frap: {m}; it cannot be applied")
                }));
            }
            Err(e) => return Err(OpError::Runtime(e.to_string())),
        };
        for r in applied.reports {
            cx.report(r.level, r.message);
            reported += 1;
        }
        let Some(mut result) = applied.mesh else {
            continue;
        };
        // `BKE_mesh_nomain_to_mesh`: the data keeps its ID, takes the result's geometry and
        // materials (`BKE_object_material_from_eval_data`).
        let mesh = cx.mesh_mut(me)?;
        result.header = mesh.header.clone();
        *mesh = result;
        if p.merge_customdata.value
            && matches!(kind, Some(ModKind::Constructive | ModKind::Nonconstructive))
        {
            merge_customdata_for_apply_modifier(cx.mesh_mut(me)?);
        }
        edit::remove(&mut cx.object_mut(ob)?.modifiers, i);
    }
    if !errors.is_empty() {
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    if !changed {
        return Ok(OpStatus::Cancelled);
    }
    if p.report.value && reported == reports_before {
        cx.report(ReportType::Info, format!("Applied modifier: {name}"));
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uv_merge_follows_blender() {
        let close = 0.5f32;
        let near = f32::from_bits(close.to_bits() + 3);
        let mut uv = vec![[close, 0.25], [near, 0.25], [0.75, 0.25], [close, 0.25]];
        merge_uvs_for_vertex(&[0, 1, 2, 3], &mut uv);
        assert_eq!(uv[1], [close, 0.25], "within 12 ULPs: merged");
        assert_eq!(uv[2], [0.75, 0.25], "apart: kept");
        let far = f32::from_bits(close.to_bits() + 13);
        let mut uv = vec![[close, 0.0], [far, 0.0]];
        merge_uvs_for_vertex(&[0, 1], &mut uv);
        assert_eq!(uv[1], [far, 0.0]);
        assert_eq!(ulp_diff_ff(1.0, -1.0), 2 * 1.0f32.to_bits());
    }
}
