// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.face_split_by_edges()` (Weld Edges into Faces), tier T2
//! (`tests/golden/cases/edit/create/face_split_by_edges.py`). Port of
//! `edbm_face_split_by_edges_exec` and `bm_face_split_by_edges`
//! (`editors/mesh/editmesh_intersect.cc`, Blender 5.2.4, GPL-2.0-or-later) over
//! `BM_face_split_edgenet`.
//!
//! Selected wire edges connected to the boundary of a selected face (directly or through
//! other selected wire edges) split that face; each wire edge goes to the face whose corner
//! it leaves into. The pieces are selected. Wire nets that touch no face boundary
//! (`USE_NET_ISLAND_CONNECT`) are left unchanged (frap does not port the island connection;
//! `docs/deviations.md`).

use frap_bmesh::ops::contextual_create::{bli, split_edgenet};
use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use rustc_hash::FxHashMap;

use super::edge_face_add::edbm;

// Ported from blender/source/blender/editors/mesh/editmesh_intersect.cc
// (bm_face_split_by_edges).
fn split_face(bm: &mut BMesh, f: FaceId, f_index: i64, index: &mut FxHashMap<EdgeId, i64>) {
    let tagged = |bm: &BMesh, e: EdgeId| bm.edge(e).flag.contains(ElemFlag::TAG);
    let mut net: Vec<EdgeId> = Vec::new();
    let mut stack: Vec<VertId> = Vec::new();
    let mut stack_next: Vec<VertId> = Vec::new();
    for l in bm.face_loops(f) {
        let lv = bm.lp(l).v;
        for e in bm.vert_edges(lv) {
            if tagged(bm, e) && index.get(&e).copied().unwrap_or(-1) == f_index {
                let v = bm.edge_other_vert(e, lv);
                bm.vert_mut(v).e = Some(e);
                stack.push(v);
                net.push(e);
            }
        }
    }
    while let Some(v) = stack.pop() {
        for e_next in bm.vert_edges(v) {
            if tagged(bm, e_next) && index.get(&e_next).copied().unwrap_or(-1) == -1 {
                let v_next = bm.edge_other_vert(e_next, v);
                index.insert(e_next, f_index);
                stack_next.push(v_next);
                net.push(e_next);
            }
        }
        if stack.is_empty() {
            std::mem::swap(&mut stack, &mut stack_next);
        }
    }
    let faces = split_edgenet::face_split_edgenet(bm, f, &net);
    for nf in faces {
        bm.face_select_set(nf, true);
        bm.face_mut(nf).flag.remove(ElemFlag::TAG);
    }
}

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, _p: mesh::FaceSplitByEdges) -> OpResult {
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let Some((_, te, tf)) = edbm::with_bm(cx, me, |bm| bm.selected_counts()) else {
            continue;
        };
        if te == 0 || tf == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
        let mut index: FxHashMap<EdgeId, i64> = FxHashMap::default();
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::SELECT) && bm.edge_is_wire(e) {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                let [a, b] = bm.edge(e).v;
                bm.vert_mut(a).flag.insert(ElemFlag::TAG);
                bm.vert_mut(b).flag.insert(ElemFlag::TAG);
            } else {
                bm.edge_mut(e).flag.remove(ElemFlag::TAG);
            }
            index.insert(e, -1);
        }
        let mut face_index: FxHashMap<FaceId, i64> = FxHashMap::default();
        for (i, f) in bm.face_ids().into_iter().enumerate() {
            let sel = bm.face(f).flag.contains(ElemFlag::SELECT);
            bm.face_mut(f).flag.set(ElemFlag::TAG, sel);
            bm.face_mut(f).flag.remove(ElemFlag::INTERNAL_TAG);
            face_index.insert(f, i as i64);
        }
        for e in bm.edge_ids() {
            if !bm.edge(e).flag.contains(ElemFlag::TAG) {
                continue;
            }
            for v in bm.edge(e).v {
                let loops: Vec<_> = bm
                    .vert_loops(v)
                    .into_iter()
                    .filter(|&l| bm.face(bm.lp(l).f).flag.contains(ElemFlag::TAG))
                    .collect();
                let l_best = match loops.len() {
                    0 => None,
                    1 => Some(loops[0]),
                    _ => {
                        let v_other = bm.edge_other_vert(e, v);
                        let e_dir = bli::normalize_v3(bli::sub(bm.vert(v_other).co, bm.vert(v).co)).0;
                        let mut dot_best = f32::MAX;
                        let mut best = None;
                        // `BLI_SMALLSTACK_POP`: the last pushed loop first.
                        for &l in loops.iter().rev() {
                            let ll = bm.lp(l);
                            let fno = bm.face(ll.f).no;
                            let dot_test = bli::dot(e_dir, fno).abs();
                            if dot_test < dot_best {
                                let co = |x: VertId| bm.vert(x).co;
                                let pv = bm.lp(ll.prev).v;
                                let nv = bm.lp(ll.next).v;
                                if bli::angle_signed_on_axis_v3v3v3_v3(co(pv), co(ll.v), co(v_other), fno)
                                    < bli::angle_signed_on_axis_v3v3v3_v3(co(pv), co(ll.v), co(nv), fno)
                                {
                                    dot_best = dot_test;
                                    best = Some(l);
                                }
                            }
                        }
                        best
                    }
                };
                if let Some(lb) = l_best {
                    index.insert(e, face_index[&bm.lp(lb).f]);
                }
            }
        }
        for f in bm.face_ids() {
            if bm.faces.contains(f.0) && bm.face(f).flag.contains(ElemFlag::TAG) {
                let fi = face_index[&f];
                split_face(bm, f, fi, &mut index);
            }
        }
        bm.normals_update();
        bm.select_flush_mode();
    }
    Ok(OpStatus::Finished)
}
