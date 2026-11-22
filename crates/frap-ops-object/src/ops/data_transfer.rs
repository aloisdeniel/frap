// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.data_transfer(use_reverse_transfer=False, use_freeze=False,
//! data_type='VGROUP_WEIGHTS', use_create=True, vert_mapping='NEAREST', edge_mapping='NEAREST',
//! loop_mapping='NEAREST_POLYNOR', poly_mapping='NEAREST', use_auto_transform=False,
//! use_object_transform=True, use_max_distance=False, max_distance=1.0, ray_radius=0.0,
//! islands_precision=0.1, layers_select_src='ACTIVE', layers_select_dst='ACTIVE',
//! mix_mode='REPLACE', mix_factor=1.0)`, tier T2. Port of `data_transfer_exec`
//! (`editors/object/object_data_transfer.cc`, Blender 5.2.4, GPL-2.0-or-later) over the
//! transfer of the DATA_TRANSFER modifier (`frap_modifiers::data_transfer`, WP 8.3.13).
//!
//! Transfers `data_type` from the evaluated mesh of the active object to the mesh data of every
//! other selected editable mesh object (with `use_reverse_transfer`, from each selected object
//! to the active one), creating the destination layers with `use_create` (new vertex groups
//! become active). Each mesh data is processed once: a second object sharing it gets the
//! WARNING `Skipping object 'B', data 'A' has already been processed with a previous object`;
//! a selected object that is not a mesh counts in `Failed to transfer mesh data to N objects`.
//! The transfer's errors (`'Topology' mapping cannot be used in this case`, …) are ERROR
//! reports. `use_freeze` only reports `Operator is frozen, changes to its settings won't take
//! effect until you unfreeze it` (INFO). The layer selections are the static items
//! (`ACTIVE`/`ALL`/`BONE_SELECT`/`BONE_DEFORM` → `ACTIVE`/`NAME`/`INDEX`); the per-layer items
//! Blender adds at run time (a vertex group or UV map by name) are not offered.
//!
//! Poll: an active mesh object with data (`data_transfer_poll`).

use frap_modifiers::data_transfer::modes::{self, DT_MULTILAYER_INDEX_MAX};
use frap_modifiers::data_transfer::{Destination, Source, SpaceTransform, TransferParams, transfer};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::Context;
use frap_scene::EvaluatedObject;
use frap_spec_ops_object::object;
use std::collections::HashSet;
use std::sync::Arc;

use crate::support::weights;

/// `data_transfer_poll`: an active mesh object with data.
pub(crate) fn transfer_poll(c: &Context) -> Result<(), PollFail> {
    let Some((_, ob)) = poll::active(c) else {
        return Err(poll::fail(PollReq::ActiveObject, "make a mesh object active"));
    };
    if ob.object_type() != "MESH" || ob.data.is_none() {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh object active",
        ));
    }
    Ok(())
}

/// `data_transfer_poll`
pub(crate) const TRANSFER_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveType("MESH")],
    check: transfer_poll,
};

/// The mesh data of a mesh object.
pub(crate) fn mesh_of(cx: &OpCtx, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    let o = cx.main().objects.get(ob)?;
    if o.object_type() != "MESH" {
        return None;
    }
    let me = o.data?.typed::<MeshId>()?;
    cx.main().meshes.get(me).map(|_| me)
}

/// `float[4][4]` of an evaluated object's world matrix.
fn world(eo: &EvaluatedObject) -> [[f32; 4]; 4] {
    eo.matrix_world.to_cols_array_2d()
}

/// The source side of a transfer from `ob` (its evaluated mesh `eo`).
pub(crate) fn source<'a>(c: &Context, ob: IdRef<ObjectId>, eo: &'a EvaluatedObject) -> Option<Source<'a>> {
    let mesh = eo.mesh.as_deref()?;
    let active = c.main.objects.get(ob).map_or(-1, |o| o.vertex_groups_active_index);
    let bone_select = Some(weights::subset(c, ob, "BONE_SELECT").0);
    let bone_deform = Some(weights::subset(c, ob, "BONE_DEFORM").0);
    Some(Source {
        mesh,
        vgroup_names: &mesh.vertex_group_names,
        active_vgroup: active,
        bone_select,
        bone_deform,
        matrix_world: world(eo),
    })
}

/// Runs `f` on the mesh data of `ob_dst` (a COW copy written back) with the object's active
/// group index.
pub(crate) fn with_destination<R>(
    cx: &mut OpCtx,
    ob_dst: IdRef<ObjectId>,
    matrix_world: [[f32; 4]; 4],
    f: impl FnOnce(&mut Destination<'_>) -> R,
) -> Result<R, OpError> {
    let me = mesh_of(cx, ob_dst).ok_or_else(|| OpError::Runtime("object has no mesh".into()))?;
    let mut mesh = cx
        .main()
        .meshes
        .get(me)
        .cloned()
        .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))?;
    let mut active = cx.main().objects.get(ob_dst).map_or(0, |o| o.vertex_groups_active_index);
    let before = active;
    let r = {
        let mut dst = Destination {
            mesh: &mut mesh,
            active_vgroup: &mut active,
            matrix_world,
            is_modifier: false,
        };
        f(&mut dst)
    };
    *cx.mesh_mut(me)? = mesh;
    if active != before {
        cx.object_mut(ob_dst)?.vertex_groups_active_index = active;
    }
    Ok(r)
}

/// `data_transfer_exec_preprocess_objects` + `data_transfer_exec_is_object_valid` (without
/// reverse transfer): the selected editable mesh objects other than `ob_src`, each mesh data
/// once (the WARNING for the others), and the count of the selected objects that are not
/// meshes.
pub(crate) fn destinations(
    cx: &mut OpCtx,
    ob_src: IdRef<ObjectId>,
    objects: &[IdRef<ObjectId>],
) -> (Vec<IdRef<ObjectId>>, usize) {
    let mut done: HashSet<IdRef<MeshId>> = HashSet::new();
    let mut valid = Vec::new();
    let mut invalid = 0usize;
    for &ob in objects {
        if ob == ob_src {
            continue;
        }
        let Some(me) = mesh_of(cx, ob) else {
            invalid += 1;
            continue;
        };
        if done.insert(me) {
            valid.push(ob);
        } else {
            let ob_name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            let me_name = cx.main().meshes.name_of(me).unwrap_or_default().to_owned();
            cx.report(
                ReportType::Warning,
                format!(
                    "Skipping object '{ob_name}', data '{me_name}' has already been processed \
                     with a previous object"
                ),
            );
            invalid += 1;
        }
    }
    (valid, invalid)
}

/// The layer selections of a multi-layer data type (zeros elsewhere, as Blender's arrays).
pub(crate) fn layer_arrays(
    data_type: u32,
    layers_src: i32,
    layers_dst: i32,
) -> ([i32; DT_MULTILAYER_INDEX_MAX], [i32; DT_MULTILAYER_INDEX_MAX]) {
    let mut src = [0i32; DT_MULTILAYER_INDEX_MAX];
    let mut dst = [0i32; DT_MULTILAYER_INDEX_MAX];
    let idx = modes::srcdst_index(data_type);
    if idx != modes::DT_MULTILAYER_INDEX_INVALID {
        src[idx as usize] = layers_src;
        dst[idx as usize] = layers_dst;
    }
    (src, dst)
}

#[operator(poll = TRANSFER_POLL, tier = T2)]
fn exec(cx: &mut OpCtx, p: object::DataTransfer) -> OpResult {
    let Some(ob_src) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    if p.use_freeze.value {
        cx.report(
            ReportType::Info,
            "Operator is frozen, changes to its settings won't take effect until you unfreeze it",
        );
        return Ok(OpStatus::Finished);
    }
    let reverse = p.use_reverse_transfer.value;
    let data_type = modes::data_type_bits(p.data_type.value.identifier());
    let mut layers_src = modes::layers_select_src(p.layers_select_src.value.identifier());
    let mut layers_dst = modes::layers_select_dst(p.layers_select_dst.value.identifier());
    if reverse {
        // The two enums swap their values (`std::swap(layers_src, layers_dst)`): the codes of
        // ACTIVE/ALL/NAME… keep their numbers.
        std::mem::swap(&mut layers_src, &mut layers_dst);
    }
    let (fromlayers, tolayers) = layer_arrays(data_type, layers_src, layers_dst);
    let use_auto_transform = p.use_auto_transform.value;
    let use_object_transform = p.use_object_transform.value && !use_auto_transform;
    let params = |st: Option<SpaceTransform>| TransferParams {
        data_types: data_type,
        use_create: p.use_create.value,
        vert_mode: modes::vert_mapping(p.vert_mapping.value.identifier()),
        edge_mode: modes::edge_mapping(p.edge_mapping.value.identifier()),
        loop_mode: modes::loop_mapping(p.loop_mapping.value.identifier()),
        poly_mode: modes::poly_mapping(p.poly_mapping.value.identifier()),
        space_transform: st,
        auto_transform: use_auto_transform,
        max_distance: if p.use_max_distance.value {
            p.max_distance.value
        } else {
            f32::MAX
        },
        ray_radius: p.ray_radius.value,
        islands_precision: p.islands_precision.value,
        layers_src: fromlayers,
        layers_dst: tolayers,
        mix_mode: modes::mix_mode(p.mix_mode.value.identifier()),
        mix_factor: p.mix_factor.value,
        vgroup_name: None,
        invert_vgroup: false,
    };
    let objects = cx.ctx().selected_editable_objects();
    // The evaluated objects, before anything changes (the depsgraph of the call).
    let mut evaluated: Vec<(IdRef<ObjectId>, Arc<EvaluatedObject>)> = Vec::new();
    for &ob in objects.iter().chain(std::iter::once(&ob_src)) {
        if evaluated.iter().any(|(o, _)| *o == ob) || mesh_of(cx, ob).is_none() {
            continue;
        }
        let eo = cx.evaluated(ob)?;
        evaluated.push((ob, eo));
    }
    let eval_of = |ob: IdRef<ObjectId>| {
        evaluated
            .iter()
            .find(|(o, _)| *o == ob)
            .map(|(_, e)| Arc::clone(e))
    };
    let active = ob_src;
    let mut done: HashSet<IdRef<MeshId>> = HashSet::new();
    let mut invalid = 0usize;
    let mut any_changed = false;
    let mut errors: Vec<String> = Vec::new();
    for &ob in &objects {
        let (src, dst) = if reverse { (ob, active) } else { (active, ob) };
        // data_transfer_exec_is_object_valid
        let (Some(_), Some(me_dst)) = (mesh_of(cx, src), mesh_of(cx, dst)) else {
            if src != dst {
                invalid += 1;
            }
            continue;
        };
        if src == dst {
            continue;
        }
        if !reverse && !done.insert(me_dst) {
            let ob_name = cx.main().objects.name_of(dst).unwrap_or_default().to_owned();
            let me_name = cx.main().meshes.name_of(me_dst).unwrap_or_default().to_owned();
            cx.report(
                ReportType::Warning,
                format!(
                    "Skipping object '{ob_name}', data '{me_name}' has already been processed \
                     with a previous object"
                ),
            );
            invalid += 1;
            continue;
        }
        let (Some(src_eo), Some(dst_eo)) = (eval_of(src), eval_of(dst)) else {
            invalid += 1;
            continue;
        };
        let Some(source) = source(&cx.ctx(), src, &src_eo) else {
            invalid += 1;
            continue;
        };
        let st = use_object_transform
            .then(|| SpaceTransform::from_matrices(&world(&dst_eo), &world(&src_eo)));
        let pr = params(st);
        let out = with_destination(cx, dst, world(&dst_eo), |d| transfer(&pr, &source, d))?;
        any_changed |= out.changed;
        errors.extend(out.errors);
    }
    if invalid > 0 {
        cx.report(
            ReportType::Warning,
            format!("Failed to transfer mesh data to {invalid} objects"),
        );
    }
    // Blender's ERROR reports make the call raise `RuntimeError` (the changes stay). frap
    // raises when nothing was transferred (the roll-back then changes nothing), and keeps the
    // ERROR reports of a call that also transferred something (D-158).
    if !errors.is_empty() && !any_changed {
        return Err(OpError::Runtime(errors.join("\nError: ")));
    }
    for e in errors {
        cx.report(ReportType::Error, e);
    }
    Ok(OpStatus::Finished)
}
