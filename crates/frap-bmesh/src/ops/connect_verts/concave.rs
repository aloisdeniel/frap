// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.connect_verts_concave` (Blender 5.2.4 `bmo_connect_verts_concave_exec`,
//! `operators/bmo_connect_concave.cc`), implemented by WP 2.8.
//!
//! Python signature: `bmesh.ops.connect_verts_concave(bmesh, faces=[])`.
//!
//! **Create/kill order**: each input face with a concave corner (vertices tagged, input
//! order) is triangulated (`BM_face_triangulate`, BEAUTY), then its new interior edges
//! (creation order, sorted with libc++'s `std::sort`, `crate::cxx_sort`) are dissolved again
//! (`BM_faces_join`, the joined face is new) as long as both sides stay convex: edges between
//! two convex corners first, longest first.
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH`.

use crate::ops::contextual_create::bli;
use crate::ops::triangulate::{NgonMethod, QuadMethod, face_triangulate};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId};

/// Input slots of `connect_verts_concave`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectVertsConcaveIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
}

/// Output slots of `connect_verts_concave`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConnectVertsConcaveOut {
    /// `edges.out` then `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out` (mesh order).
    pub edges: Vec<EdgeId>,
    /// `faces.out` (mesh order).
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_connect_verts_concave_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const EDGE_OUT: u16 = 1;
const FACE_OUT: u16 = 2;

/// `BM_loop_is_convex`.
fn loop_is_convex(bm: &BMesh, l: LoopId) -> bool {
    let ll = bm.lp(l);
    let co = bm.vert(ll.v).co;
    let prev = bli::sub(bm.vert(bm.lp(ll.prev).v).co, co);
    let next = bli::sub(bm.vert(bm.lp(ll.next).v).co, co);
    let l_no = bli::cross(next, prev);
    bli::dot(l_no, bm.face(ll.f).no) > 0.0
}

/// `bm_face_convex_tag_verts`.
fn face_convex_tag_verts(bm: &mut BMesh, f: FaceId) -> bool {
    let mut is_concave = false;
    if bm.face(f).len > 3 {
        for l in bm.face_loops(f) {
            let convex = loop_is_convex(bm, l);
            let v = bm.lp(l).v;
            if !convex {
                is_concave = true;
            }
            bm.vert_mut(v).flag.set(ElemFlag::TAG, !convex);
        }
    }
    is_concave
}

// Ported from blender/source/blender/bmesh/operators/bmo_connect_concave.cc
// (bm_face_split_by_concave).
fn face_split_by_concave(bm: &mut BMesh, f_base: FaceId, eps: f32) {
    let normal = bm.face(f_base).no;
    // `BM_face_triangulate(…, quad_method = 0, ngon_method = 0 (beauty), use_tag = false)`.
    let tri = face_triangulate(bm, f_base, QuadMethod::Beauty, NgonMethod::Beauty, false);
    for &f in &tri.faces_new {
        bm.oflag_enable(f, FACE_OUT);
    }
    bm.oflag_enable(f_base, FACE_OUT);
    // `faces_double`: a linked list the triangulation prepends to (most recent first), then
    // the joins below.
    let mut faces_double: Vec<FaceId> = tri.faces_double;
    // The new edges in creation order, sorted as `std::sort` does (M8 stabilization round 2:
    // equal lengths keep libc++'s order, not the slot order).
    let mut edges = tri.edges_new;
    let concave = |bm: &BMesh, e: EdgeId| {
        let [a, b] = bm.edge(e).v;
        bm.vert(a).flag.contains(ElemFlag::TAG) && bm.vert(b).flag.contains(ElemFlag::TAG)
    };
    let len_sq = |bm: &BMesh, e: EdgeId| {
        let [a, b] = bm.edge(e).v;
        bli::len_squared_v3v3(bm.vert(a).co, bm.vert(b).co)
    };
    {
        let bm_ref: &BMesh = bm;
        crate::cxx_sort::sort(&mut edges, |&a, &b| {
            let (ca, cb) = (concave(bm_ref, a), concave(bm_ref, b));
            if ca != cb {
                // Edges between concave corners last.
                return !ca & cb;
            }
            // Otherwise shortest edges last.
            len_sq(bm_ref, a) > len_sq(bm_ref, b)
        });
    }
    for e in edges {
        if !bm.edges.contains(e.0) {
            continue;
        }
        bm.oflag_enable(e, EDGE_OUT);
        if !bm.edge_is_contiguous(e) {
            continue;
        }
        let pair = bm.edge(e).l.and_then(|la| {
            let lb = bm.lp(la).radial_next;
            (la != lb && bm.lp(lb).radial_next == la).then_some([la, lb])
        });
        let Some(l_pair) = pair else {
            continue;
        };
        let mut ok = true;
        for &l in &l_pair {
            let ll = bm.lp(l);
            let rn = bm.lp(ll.radial_next);
            let rnn = bm.lp(rn.next).next;
            let cr = bli::cross_tri_v3(
                bm.vert(ll.v).co,
                bm.vert(bm.lp(rnn).v).co,
                bm.vert(bm.lp(ll.prev).v).co,
            );
            if bli::dot(cr, normal) <= eps {
                ok = false;
                break;
            }
        }
        if ok {
            let f_pair = [bm.lp(l_pair[0]).f, bm.lp(l_pair[1]).f];
            let (f_new, f_double) = bm.faces_join(&f_pair, true, true);
            match (f_new, f_double) {
                // `BLI_linklist_prepend`.
                (Some(fnew), Some(_)) => faces_double.insert(0, fnew),
                (Some(fnew), None) => bm.oflag_enable(fnew, FACE_OUT),
                _ => {}
            }
        }
    }
    for f in faces_double {
        if bm.faces.contains(f.0) {
            bm.face_kill(f);
        }
    }
}

/// Runs `connect_verts_concave` on `bm`.
pub fn connect_verts_concave(
    bm: &mut BMesh,
    i: &ConnectVertsConcaveIn,
) -> Result<ConnectVertsConcaveOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|&&f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "connect_verts_concave: dead face {dead:?}"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        let mut changed = false;
        for &f in &i.faces {
            if bm.faces.contains(f.0) && bm.face(f).len > 3 && face_convex_tag_verts(bm, f) {
                face_split_by_concave(bm, f, f32::EPSILON);
                changed = true;
            }
        }
        let mut out = ConnectVertsConcaveOut::default();
        if changed {
            out.edges = bm
                .edge_ids()
                .into_iter()
                .filter(|&e| bm.oflag_test(e, EDGE_OUT))
                .collect();
            out.faces = bm
                .face_ids()
                .into_iter()
                .filter(|&f| bm.oflag_test(f, FACE_OUT))
                .collect();
            out.geom = out.edges.iter().map(|&e| BMElem::Edge(e)).collect();
            out.geom.extend(out.faces.iter().map(|&f| BMElem::Face(f)));
        }
        out
    }))
}
