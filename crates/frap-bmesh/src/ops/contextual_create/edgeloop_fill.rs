// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.edgeloop_fill` (`operators/bmo_fill_edgeloop.cc`, Blender 5.2.4): one face per
//! isolated closed loop of the input edges.
//!
//! **Create order**: loops are walked from the first unused vertex of the input (vertices in
//! the order the edges list them); each loop becomes one face with
//! `BM_face_create_ngon_verts(calc_winding, no edge creation)`, unless a face over the same
//! vertices exists. Nothing is killed.

use rustc_hash::FxHashSet;

use super::construct;
use crate::types::CreateFlag;
use crate::{BMesh, EdgeId, ElemFlag, FaceId, VertId};

// Ported from blender/source/blender/bmesh/operators/bmo_fill_edgeloop.cc
// (bmo_edgeloop_fill_exec).
/// `bmesh.ops.edgeloop_fill(edges, mat_nr, use_smooth)`; returns `faces.out` (mesh order).
pub fn edgeloop_fill(bm: &mut BMesh, edges: &[EdgeId], mat_nr: i32, use_smooth: bool) -> Vec<FaceId> {
    let tote = edges.len();
    let totv = tote;
    let mut verts: Vec<VertId> = Vec::with_capacity(totv);
    let mut used: FxHashSet<VertId> = FxHashSet::default();
    let marked: FxHashSet<EdgeId> = edges.iter().copied().collect();
    for &e in edges {
        for v in bm.edge(e).v {
            if !used.contains(&v) {
                if verts.len() == tote {
                    return Vec::new();
                }
                used.insert(v);
                verts.push(v);
            }
        }
    }
    if verts.len() != tote {
        return Vec::new();
    }
    for &v in &verts {
        let c = bm
            .vert_edges(v)
            .into_iter()
            .filter(|e| marked.contains(e))
            .count();
        if c != 2 {
            return Vec::new();
        }
    }
    let mut out: FxHashSet<FaceId> = FxHashSet::default();
    let mut e_prev: Option<EdgeId> = None;
    let mut e_next: Option<EdgeId> = None;
    let mut totv_used = 0;
    while totv_used < totv {
        let Some(&v0) = verts.iter().find(|v| used.contains(v)) else {
            break;
        };
        let mut v = v0;
        let mut f_verts: Vec<VertId> = Vec::new();
        loop {
            for e in bm.vert_edges(v) {
                if marked.contains(&e) && Some(e) != e_prev {
                    e_next = Some(e);
                    break;
                }
            }
            f_verts.push(v);
            used.remove(&v);
            totv_used += 1;
            let en = e_next.expect("vertex has two loop edges");
            v = bm.edge_other_vert(en, v);
            e_prev = e_next;
            if v == f_verts[0] {
                break;
            }
        }
        if bm.face_exists(&f_verts).is_none() {
            let f = construct::face_create_ngon_verts(
                bm,
                &f_verts,
                None,
                CreateFlag::empty(),
                true,
                false,
            )
            .expect("edge loop face");
            out.insert(f);
            bm.face_mut(f).mat_nr = mat_nr as i16;
            if use_smooth {
                bm.face_mut(f).flag.insert(ElemFlag::SMOOTH);
            }
        }
    }
    bm.face_ids().into_iter().filter(|f| out.contains(f)).collect()
}
