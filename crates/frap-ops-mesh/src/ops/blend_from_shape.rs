// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.blend_from_shape(shape='', blend=1.0, add=True)`, tier T1
//! (`tests/golden/cases/weights_shapes/blend_from_shape.py`). Port of
//! `edbm_blend_from_shape_exec` and `shape_itemf` (`editors/mesh/editmesh_tools.cc`) and
//! `shape_key_report_if_locked` (`editors/object/object_shapekey.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! `shape` names a key block of the edit object's mesh (a dynamic enum; unset: the basis).
//! In every mesh in Edit Mode (unique data) with a block of that name, each selected visible
//! vertex moves toward the block's coordinates (its Edit Mode shape layer): with `add`, by
//! `blend × (block − its relative key)`, else `lerp(co, block, blend)`. A mesh whose active
//! block is locked is skipped (`The active shape key of Cube is locked`). Errors: `Active mesh
//! does not have shape keys`, `No selected vertex`. X-mirror editing is not done (frap meshes
//! have no `use_mirror_x`).

use frap_bmesh::ElemFlag;
use frap_op::prelude::*;
use frap_op::{EnumItemOwned, operator};
use frap_rna::MeshId;
use frap_scene::Context;
use frap_spec_ops_mesh::mesh;

/// `shape_itemf`: the shape layers of the edit object (its key's block names).
fn shape_items(c: &Context) -> Vec<EnumItemOwned> {
    poll::edit_object(c)
        .and_then(|(_, o)| o.data?.typed::<MeshId>())
        .and_then(|me| c.main.meshes.get(me)?.shape_keys)
        .and_then(|k| c.main.shape_keys.get(k))
        .map(|k| {
            k.key_blocks
                .iter()
                .map(|kb| EnumItemOwned {
                    identifier: kb.name.clone(),
                    name: kb.name.clone(),
                    description: "".into(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `shape_key_report_if_locked`: the active block of `ob`'s key is locked.
pub(crate) fn active_block_locked(cx: &OpCtx, ob: frap_rna::IdRef<frap_rna::ObjectId>) -> bool {
    let main = cx.main();
    let Some(o) = main.objects.get(ob) else {
        return false;
    };
    o.data
        .and_then(|d| d.typed::<MeshId>())
        .and_then(|me| main.meshes.get(me)?.shape_keys)
        .and_then(|k| main.shape_keys.get(k))
        .and_then(|k| {
            k.key_blocks
                .get(usize::try_from(o.active_shape_key_index).ok()?)
        })
        .is_some_and(|kb| kb.lock_shape)
}

#[operator(poll = poll::EDITMESH, tier = T1, dyn_enums = [("shape", shape_items)])]
fn exec(cx: &mut OpCtx, p: mesh::BlendFromShape) -> OpResult {
    let (blend, use_add) = (p.blend.value, p.add.value);
    let Some(me_ref) = poll::edit_object(&cx.ctx()).and_then(|(_, o)| o.data?.typed::<MeshId>())
    else {
        return Ok(OpStatus::Cancelled);
    };
    let totshape_ref = {
        let (_, edit) = cx.split_edit();
        edit.meshes
            .get(&me_ref.key())
            .map_or(0, |em| em.bm.shape_layers.len())
    };
    let names: Vec<_> = shape_items(&cx.ctx())
        .into_iter()
        .map(|i| i.identifier)
        .collect();
    let mut shape_ref = if p.shape.is_set {
        names
            .iter()
            .position(|n| *n == p.shape.value)
            .map_or(-1, |i| i as i64)
    } else {
        0
    };
    if totshape_ref == 0 || shape_ref < 0 {
        return Err(OpError::Runtime(
            "Active mesh does not have shape keys".into(),
        ));
    }
    if shape_ref >= totshape_ref as i64 {
        shape_ref = 0;
    }
    let Some(kb_ref_name) = names.get(shape_ref as usize).cloned() else {
        return Err(OpError::Runtime(
            "Active mesh does not have shape keys".into(),
        ));
    };
    let mut tot_selected = 0;
    let mut tot_locked = 0;
    let mut locked_msg = String::new();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let selected = {
            let (_, edit) = cx.split_edit();
            edit.meshes
                .get(&me.key())
                .is_some_and(|em| em.bm.selected_counts().0 > 0)
        };
        if !selected {
            continue;
        }
        if active_block_locked(cx, ob) {
            let name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            locked_msg = format!("The active shape key of {name} is locked");
            cx.report(ReportType::Error, locked_msg.clone());
            tot_locked += 1;
            continue;
        }
        tot_selected += 1;
        let Some(key) = cx
            .main()
            .meshes
            .get(me)
            .and_then(|m| m.shape_keys)
            .and_then(|k| cx.main().shape_keys.get(k))
        else {
            continue;
        };
        let Some(shape) = key.find_block(&kb_ref_name) else {
            continue;
        };
        let relative = key.key_blocks[shape].relative_key;
        let bm = cx.bmesh_mut(me)?;
        if shape >= bm.shape_layers.len() {
            continue;
        }
        for v in bm.vert_ids() {
            let f = bm.vert(v).flag;
            if !f.contains(ElemFlag::SELECT) || f.contains(ElemFlag::HIDDEN) {
                continue;
            }
            let s = v.slot();
            let mut co = bm.shape_layers[shape].co[s];
            let eco = bm.vert(v).co;
            let new = if use_add {
                let rco = bm.shape_layers.get(relative).map_or([0.0; 3], |l| l.co[s]);
                co = [co[0] - rco[0], co[1] - rco[1], co[2] - rco[2]];
                [
                    eco[0] + co[0] * blend,
                    eco[1] + co[1] * blend,
                    eco[2] + co[2] * blend,
                ]
            } else {
                let t = blend;
                let u = 1.0 - t;
                [
                    u * eco[0] + t * co[0],
                    u * eco[1] + t * co[1],
                    u * eco[2] + t * co[2],
                ]
            };
            bm.vert_mut(v).co = new;
        }
        bm.normals_update();
    }
    if tot_selected == 0 && tot_locked == 0 {
        return Err(OpError::Runtime("No selected vertex".into()));
    }
    if tot_selected == 0 {
        // Blender's error report makes the call raise.
        return Err(OpError::Runtime(locked_msg));
    }
    Ok(if tot_selected > 0 {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
