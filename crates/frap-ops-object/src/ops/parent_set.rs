// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.parent_set(type='OBJECT', xmirror=False, keep_transform=False)`, tier T0
//! (`tests/golden/cases/object/parent/*.py`, `tests/golden/cases/armature/deform/*.py`). Port
//! of `parent_set_exec`, `parent_set_nonvertex_parent` and `parent_set_with_depsgraph`
//! (`editors/object/object_relations.cc`) with `BKE_object_calc_parent`
//! (`blenkernel/intern/object.cc`) and `ED_object_vgroup_calc_from_armature`
//! (`editors/armature/armature_skinning.cc`, through `support::deform`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - the active object becomes the parent of every other selected editable object; the
//!   selection and the active object stay; `matrix_parent_inverse` becomes the inverse of the
//!   parent matrix and the child's channels are kept, so a child that had no parent does not
//!   move, while one that had another parent jumps to its own channels unless
//!   `keep_transform=True` (then its channels first take its world matrix, as evaluated before
//!   the operator);
//! - a parent loop: `RuntimeError: Error: Loop in parents` (Blender stops at that child;
//!   frap rolls the whole operator back, D-44 candidate);
//! - `CURVE`, `LATTICE` and the `ARMATURE*` types parent like `OBJECT` (`parent_type` stays
//!   `'OBJECT'`) and also add a `Curve`, `Lattice` or `Armature` modifier pointing at the
//!   parent to every child that can have modifiers (a mesh) and is not deformed by it yet
//!   (virtual armature-parent modifier included, `support::deform::deformed_by`), whatever the
//!   parent's type; `FOLLOW`/`PATH_CONST` without a curve parent: `CANCELLED`, nothing changes;
//! - with an armature parent and a mesh child, `ARMATURE_NAME` adds an empty vertex group per
//!   deforming bone and `ARMATURE_ENVELOPE` fills one per deforming bone with envelope
//!   weights (`xmirror` mirrors them along X); the last group added becomes active;
//! - `BONE` and `BONE_RELATIVE` parent to the armature's active bone (`parent_type = 'BONE'`,
//!   `parent_bone`), which must be in a visible bone collection (`Error: No active bone`, also
//!   for a parent that is not an armature); they clear / set the bone's
//!   `use_relative_parent`, and `matrix_parent_inverse` inverts the bone's evaluated world
//!   matrix (moved to the bone's tail, or the bone's deform matrix when relative);
//! - poll `ED_operator_object_active_editable` (no active object: `poll() failed, context is
//!   incorrect`).
//!
//! Not in frap: vertex parents (`VERTEX`, `VERTEX_TRI`, Edit Mode parenting) and
//! `ARMATURE_AUTO` (bone heat weighting, M8) answer `NotImplementedError`; so does
//! `ARMATURE_ENVELOPE` on a mesh with a SUBSURF modifier (Blender weights the limit surface).

use std::collections::BTreeMap;

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{ArmatureId, IdCode, IdRef, MeshId, ObjectId};
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::transform::would_cycle;
use frap_spec_ops_object::object::{self, parent_set::Type};
use glam::Mat4;

use crate::support::deform;
use crate::support::xform;

#[operator(
    poll = poll::ACTIVE_EDITABLE,
    tier = T0,
    support = Partial(
        "type='VERTEX'/'VERTEX_TRI' (Edit Mode) and 'ARMATURE_AUTO' (bone heat, M8) answer \
         NotImplementedError"
    ),
    notes = [
        "A parent loop (Error: Loop in parents) rolls the whole operator back; Blender keeps the \
         children parented before the loop.",
        "ARMATURE_ENVELOPE on a mesh with a Subdivision Surface modifier answers \
         NotImplementedError (Blender weights the limit surface)."
    ]
)]
fn exec(cx: &mut OpCtx, p: object::ParentSet) -> OpResult {
    let ty = p.r#type.value;
    let keep = p.keep_transform.value;
    let xmirror = p.xmirror.value;
    let (par, children) = {
        let c = cx.ctx();
        let Some(par) = c.active_object() else {
            return Err(OpError::Runtime("no active object".into()));
        };
        let children: Vec<_> = c
            .selected_editable_objects()
            .into_iter()
            .filter(|o| *o != par)
            .collect();
        (par, children)
    };
    if children.is_empty() {
        return Ok(OpStatus::Finished);
    }
    let arm_ref: Option<IdRef<ArmatureId>> = cx
        .main()
        .objects
        .get(par)
        .and_then(|o| o.data)
        .filter(|d| d.code == IdCode::AR)
        .and_then(|d| d.typed());
    let is_mesh = |cx: &OpCtx, ob: IdRef<ObjectId>| {
        cx.main()
            .objects
            .get(ob)
            .is_some_and(deform::supports_deform_modifiers)
    };
    // The active bone of `BKE_pose_channel_active_if_bonecoll_visible`.
    let mut bone = None;
    match ty {
        Type::Vertex | Type::VertexTri => {
            return Err(OpError::NotImplemented(
                "frap: vertex parenting (type='VERTEX'/'VERTEX_TRI') needs the parent in Edit \
                 Mode, which frap does not parent from yet"
                    .into(),
            ));
        }
        Type::Bone | Type::BoneRelative => {
            let active = arm_ref
                .and_then(|a| cx.main().armatures.get(a))
                .and_then(|arm| {
                    let i = arm.active_bone.filter(|&i| i < arm.bones.len())?;
                    arm.bone_collections_visible(&arm.bones[i])
                        .then(|| (i, arm.bones[i].name.clone()))
                });
            match active {
                Some(b) => bone = Some(b),
                None => return Err(OpError::Runtime("No active bone".into())),
            }
        }
        Type::Follow | Type::PathConst => {
            // Needs a curve parent (frap has no curves): `parent_set` returns false silently.
            cx.report(
                ReportType::Warning,
                format!(
                    "frap: parent_set(type='{}') needs a curve parent; nothing changed",
                    ty.identifier()
                ),
            );
            return Ok(OpStatus::Cancelled);
        }
        Type::ArmatureAuto if arm_ref.is_some() && children.iter().any(|&o| is_mesh(cx, o)) => {
            return Err(OpError::NotImplemented(
                "frap: parent_set(type='ARMATURE_AUTO') needs bone heat weighting, which arrives \
                 in M8; use 'ARMATURE_ENVELOPE' or 'ARMATURE_NAME' and assign weights"
                    .into(),
            ));
        }
        Type::ArmatureEnvelope if arm_ref.is_some() => {
            let subsurf = children.iter().any(|&o| {
                cx.main().objects.get(o).is_some_and(|ob| {
                    deform::supports_deform_modifiers(ob)
                        && ob.modifiers.iter().any(|m| {
                            m.settings.type_identifier() == ModifierType::Subsurf.identifier()
                        })
                })
            });
            if subsurf {
                return Err(OpError::NotImplemented(
                    "frap: parent_set(type='ARMATURE_ENVELOPE') on a mesh with a Subdivision \
                     Surface modifier weights the limit surface, which frap does not compute"
                        .into(),
                ));
            }
        }
        _ => {}
    }
    let modifier = deform::parent_type_modifier(ty.identifier());
    let selected: Vec<frap_rna::IdAny> = cx
        .ctx()
        .selected_objects()
        .into_iter()
        .map(Into::into)
        .collect();
    // World matrices and the parent's pose as evaluated before the operator.
    let mut worlds: BTreeMap<IdRef<ObjectId>, Mat4> = BTreeMap::new();
    for &o in children.iter().chain(std::iter::once(&par)) {
        let m = cx.evaluated(o)?.matrix_world;
        worlds.insert(o, m);
    }
    let par_eval = cx.evaluated(par)?;
    let par_world = par_eval.matrix_world;
    for ob in children {
        if would_cycle(&cx.main().objects, ob, par) {
            return Err(OpError::Runtime("Loop in parents".into()));
        }
        let ob_world = worlds.get(&ob).copied().unwrap_or(Mat4::IDENTITY);
        let Some(mut o) = cx.main().objects.get(ob).cloned() else {
            continue;
        };
        if keep {
            xform::apply_mat4(&mut o, &ob_world, false, None);
        }
        o.parent = Some(par);
        o.matrix_parent_inverse = Mat4::IDENTITY;
        o.parent_bone = bone
            .as_ref()
            .map(|(_, n)| n.clone())
            .unwrap_or_default();
        o.parent_type = match ty {
            Type::Bone | Type::BoneRelative => "BONE".into(),
            _ => "OBJECT".into(),
        };
        if let Some(mt) = modifier {
            deform::add_parent_deform(cx.main(), &mut o, mt, par.into(), &|t| {
                selected.contains(&t)
            });
        }
        // `BONE_RELATIVE_PARENTING` on the parent bone.
        if let (Some(a), Some((i, _))) = (arm_ref, &bone) {
            let relative = ty == Type::BoneRelative;
            let (main, log) = cx.main_mut();
            if let Some(arm) = main.armatures.get_mut(a, log)
                && let Some(b) = arm.bones.get_mut(*i)
            {
                b.use_relative_parent = relative;
            }
        }
        // Vertex groups from the armature (mesh child, armature parent).
        let me: Option<IdRef<MeshId>> = o.data.filter(|d| d.code == IdCode::ME).and_then(|d| d.typed());
        if let (Some(a), Some(me)) = (arm_ref, me) {
            let arm = cx.main().armatures.get(a).cloned();
            if let Some(arm) = arm {
                let active = match ty {
                    Type::ArmatureName => {
                        deform::groups_from_bone_names(cx.mesh_mut(me)?, &arm)
                    }
                    Type::ArmatureEnvelope => {
                        let inp = deform::EnvelopeInput {
                            arm: &arm,
                            arm_world: par_world,
                            ob_world,
                            mirror: xmirror,
                        };
                        deform::envelope_weights(cx.mesh_mut(me)?, &inp)
                    }
                    _ => None,
                };
                if let Some(g) = active {
                    o.vertex_groups_active_index = g as i32;
                }
            }
        }
        // `invert_m4_m4(ob->parentinv, BKE_object_calc_parent(…))`.
        let parent_mat = if o.parent_type == "BONE" {
            let arm = arm_ref.and_then(|a| cx.main().armatures.get(a));
            frap_eval::bone_parent_matrix(&o, &par_world, par_eval.pose.as_deref(), arm)
        } else {
            par_world
        };
        o.matrix_parent_inverse = parent_mat.inverse();
        let (main, log) = cx.main_mut();
        if let Some(slot) = main.objects.get_mut(ob, log) {
            // Keep the generation `get_mut` just gave the object (the clone has the old one).
            let generation = slot.header.generation;
            *slot = o;
            slot.header.generation = generation;
        }
    }
    let (_, log) = cx.main_mut();
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
