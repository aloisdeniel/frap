// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_by_pole_count(pole_count=4, type='NOTEQUAL', extend=False,
//! exclude_nonmanifold=True)`, tier T1 (`tests/golden/cases/edit/select_m8/pole_count.py`).
//! Port of `edbm_select_by_pole_count_exec` and `is_count_a_match`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode (after deselecting all unless `extend`): every visible vertex whose
//! edge count (counted up to `pole_count + 1`) compares `LESS`, `EQUAL`, `GREATER` or
//! `NOTEQUAL` to `pole_count`, and with `exclude_nonmanifold` is manifold with only manifold
//! edges, is selected in vertex mode; in edge mode its edges, in face mode its faces. The
//! selection is then flushed.

use frap_bmesh::{BMesh, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, select_by_pole_count::Type};

use super::select_more::common;

/// `is_count_a_match`.
fn is_count_a_match(ty: Type, value_test: i64, value_reference: i64) -> bool {
    match ty {
        Type::Less => value_test < value_reference,
        Type::Equal => value_test == value_reference,
        Type::Greater => value_test > value_reference,
        Type::Notequal => value_test != value_reference,
    }
}

/// `BM_vert_edge_count_at_most`.
fn edge_count_at_most(bm: &BMesh, v: VertId, n: i64) -> i64 {
    let mut count = 0i64;
    for _ in bm.vert_edges(v) {
        count += 1;
        if count == n {
            break;
        }
    }
    count
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectByPoleCount) -> OpResult {
    let pole_count = i64::from(p.pole_count.value);
    let ty = p.r#type.value;
    for (_, me) in common::edit_meshes(cx) {
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        if !p.extend.value {
            common::deselect_all(bm);
            changed = true;
        }
        for v in bm.vert_ids() {
            if common::hidden(bm, v) {
                continue;
            }
            let count = edge_count_at_most(bm, v, pole_count + 1);
            if !is_count_a_match(ty, count, pole_count) {
                continue;
            }
            if p.exclude_nonmanifold.value
                && (!bm.vert_is_manifold(v)
                    || !bm.vert_edges(v).into_iter().all(|e| bm.edge_is_manifold(e)))
            {
                continue;
            }
            changed = true;
            let mode = bm.select_mode;
            if mode.contains(SelectMode::VERT) {
                bm.vert_select_set(v, true);
            } else if mode.contains(SelectMode::EDGE) {
                for e in bm.vert_edges(v) {
                    bm.edge_select_set(e, true);
                }
            } else {
                for f in bm.vert_faces(v) {
                    bm.face_select_set(f, true);
                }
            }
        }
        if changed {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_stop_past_the_pole_count() {
        let mut bm = BMesh::new();
        let c = bm.vert_create([0.0; 3], None, Default::default());
        for i in 0..6 {
            let o = bm.vert_create([i as f32, 1.0, 0.0], None, Default::default());
            bm.edge_create(c, o, None, Default::default());
        }
        assert_eq!(edge_count_at_most(&bm, c, 5), 5);
        assert_eq!(edge_count_at_most(&bm, c, 8), 6);
        assert!(is_count_a_match(Type::Notequal, 5, 4));
        assert!(!is_count_a_match(Type::Equal, 5, 4));
    }
}
