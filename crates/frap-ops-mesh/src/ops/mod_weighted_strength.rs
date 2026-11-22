// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.mod_weighted_strength(set=False, face_strength='MEDIUM')`, tier T0
//! (`tests/golden/cases/edit/normals_m8/weighted_strength.py`). Port of
//! `edbm_mod_weighted_strength_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data), selected or not: the selection history is cleared
//! and the face attribute `__mod_weightednormals_faceweight` (INT32, the WEIGHTED_NORMAL
//! modifier's `use_face_influence` input) is created when missing, filled with 0 (`MEDIUM`).
//! With `set`, the selected faces get `face_strength` (`WEAK` -16384, `MEDIUM` 0, `STRONG`
//! 16384); without it, every visible face is selected when its strength equals
//! `face_strength` and deselected otherwise (`BM_face_select_set`). The operator always
//! finishes.

use std::sync::Arc;

use frap_bmesh::{BMesh, CdLayer, ElemFlag};
use frap_mesh::AttrData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, mod_weighted_strength::FaceStrength};

use super::flip_normals::edit_meshes_where;

/// `MOD_WEIGHTEDNORMALS_FACEWEIGHT_CDLAYER_ID`.
const FACEWEIGHT_LAYER: &str = "__mod_weightednormals_faceweight";

/// `FACE_STRENGTH_*` (`bmesh_operators.hh`).
fn strength_value(s: FaceStrength) -> i32 {
    match s {
        FaceStrength::Weak => -16384,
        FaceStrength::Medium => 0,
        FaceStrength::Strong => 16384,
    }
}

/// The position of the INT32 face layer, added (zeros) when missing; `None` when a layer of
/// another type has the name (`CustomData_get_named_layer_index(pdata, CD_PROP_INT32, …)`
/// would then add a second layer of the same name, which the attribute API cannot hold).
fn ensure_layer(bm: &mut BMesh) -> Option<usize> {
    match bm.pdata.find(FACEWEIGHT_LAYER) {
        Some(i) => matches!(bm.pdata.layers[i].data, AttrData::Int(_)).then_some(i),
        None => {
            let n = bm.faces.capacity();
            bm.pdata.add_layer(CdLayer {
                name: FACEWEIGHT_LAYER.into(),
                data: AttrData::Int(Arc::new(vec![0; n])),
            });
            bm.pdata.find(FACEWEIGHT_LAYER)
        }
    }
}

fn mod_weighted_strength(bm: &mut BMesh, set: bool, strength: i32) {
    bm.select_history_clear();
    let Some(layer) = ensure_layer(bm) else {
        return;
    };
    let faces = bm.face_ids();
    if set {
        let selected: Vec<usize> = faces
            .iter()
            .filter(|&&f| bm.face(f).flag.contains(ElemFlag::SELECT))
            .map(|f| f.slot())
            .collect();
        if let AttrData::Int(col) = &mut bm.pdata.layers[layer].data {
            let col = Arc::make_mut(col);
            for s in selected {
                if col.len() <= s {
                    col.resize(s + 1, 0);
                }
                col[s] = strength;
            }
        }
        return;
    }
    let values: Vec<i32> = match &bm.pdata.layers[layer].data {
        AttrData::Int(col) => faces
            .iter()
            .map(|f| col.get(f.slot()).copied().unwrap_or(0))
            .collect(),
        _ => return,
    };
    for (&f, v) in faces.iter().zip(values) {
        if bm.face(f).flag.contains(ElemFlag::HIDDEN) {
            continue;
        }
        bm.face_select_set(f, v == strength);
    }
}

#[operator(poll = poll::EDITMESH, tier = T0)]
fn exec(cx: &mut OpCtx, p: mesh::ModWeightedStrength) -> OpResult {
    let strength = strength_value(p.face_strength.value);
    let set = p.set.value;
    for me in edit_meshes_where(cx, |_| true) {
        mod_weighted_strength(cx.bmesh_mut(me)?, set, strength);
    }
    Ok(OpStatus::Finished)
}
