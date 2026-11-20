// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.shape_propagate_to_all()`, tier T1 (`tests/golden/cases/weights_shapes/
//! shape_propagate.py`). Port of `edbm_shape_propagate_to_all_exec` and `shape_propagate`
//! (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4, GPL-2.0-or-later: in every mesh in Edit
//! Mode (unique data) with selected vertices, each selected visible vertex's position is
//! copied into every shape layer, so every key block takes it on leaving Edit Mode. A mesh
//! with a locked block is skipped (`The object Cube has locked shape keys`). Errors (`CANCELLED`):
//! `No selected vertex`, `Mesh(es) do not have shape keys`. X-mirror editing is not done.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::ShapePropagateToAll) -> OpResult {
    let mut tot_shapekeys = 0;
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
        let any_locked = cx
            .main()
            .meshes
            .get(me)
            .and_then(|m| m.shape_keys)
            .and_then(|k| cx.main().shape_keys.get(k))
            .is_some_and(|k| k.key_blocks.iter().any(|kb| kb.lock_shape));
        if any_locked {
            let name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            locked_msg = format!("The object {name} has locked shape keys");
            cx.report(ReportType::Error, locked_msg.clone());
            tot_locked += 1;
            continue;
        }
        tot_selected += 1;
        let has_layers = {
            let (_, edit) = cx.split_edit();
            edit.meshes
                .get(&me.key())
                .is_some_and(|em| !em.bm.shape_layers.is_empty())
        };
        if !has_layers {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        for v in bm.vert_ids() {
            let f = bm.vert(v).flag;
            if !f.contains(ElemFlag::SELECT) || f.contains(ElemFlag::HIDDEN) {
                continue;
            }
            let co = bm.vert(v).co;
            for l in &mut bm.shape_layers {
                if let Some(c) = l.co.get_mut(v.slot()) {
                    *c = co;
                }
            }
        }
        tot_shapekeys += 1;
    }
    if tot_selected == 0 {
        if tot_locked == 0 {
            return Err(OpError::Runtime("No selected vertex".into()));
        }
        // Blender's error report makes the call raise.
        return Err(OpError::Runtime(locked_msg));
    }
    if tot_shapekeys == 0 {
        return Err(OpError::Runtime("Mesh(es) do not have shape keys".into()));
    }
    Ok(OpStatus::Finished)
}
