// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.duplicate` (Blender 5.2.4 `bmo_duplicate_exec` / `bmo_mesh_copy`,
//! `operators/bmo_dupe.cc`), implemented by WP 2.1 (BMesh core).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.duplicate(bmesh, geom=[], dest=None, use_select_history=False, use_edge_flip_from_face=False)`.
//!
//! **Create/kill order**: nothing is killed. Copies are created in three passes over the
//! source mesh in iteration order: (1) every input vertex; (2) every input edge, creating its
//! not-yet-copied vertices first (`v1` then `v2`); (3) every input face, creating its
//! not-yet-copied vertices (loop order from `l_first`), then its not-yet-copied edges (loop
//! order), then the face (with its loops). Each element is copied once (`DUPE_DONE`). New
//! elements take freed slots first (LIFO), then append. Attributes are copied with
//! `BM_elem_attrs_copy` (the copy keeps its own, cleared, selection flag).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH` (the history survives the flush).
//! `dest` (copying into another BMesh) is not supported: the copy goes into `bm`.

use smallvec::SmallVec;

use crate::types::{CreateFlag, EdgeId, FaceId, LoopId, OpTypeFlag, VertId};
use crate::{BMElem, BMesh, BmError};

/// Input slots of `duplicate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DuplicateIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `dest` (another BMesh (unsupported: always this one)).
    pub dest: Option<()>,
    /// `use_select_history`.
    pub use_select_history: bool,
    /// `use_edge_flip_from_face`.
    pub use_edge_flip_from_face: bool,
}

/// Output slots of `duplicate`. Blender's `*_map.out` slots map both ways (source → copy and
/// copy → source); here each map lists `(source, copy)` pairs in creation order.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DuplicateOut {
    /// `geom.out`: the new vertices, edges and faces, in iteration order (vertices, then
    /// edges, then faces).
    pub geom: Vec<BMElem>,
    /// `geom_orig.out`: the input `geom`.
    pub geom_orig: Vec<BMElem>,
    /// `vert_map.out`.
    pub vert_map: Vec<(VertId, VertId)>,
    /// `edge_map.out`.
    pub edge_map: Vec<(EdgeId, EdgeId)>,
    /// `face_map.out`.
    pub face_map: Vec<(FaceId, FaceId)>,
    /// `boundary_map.out`: source edges used by fewer than two input faces → their copies.
    pub boundary_map: Vec<(EdgeId, EdgeId)>,
    /// `isovert_map.out`: input vertices with no input edge or face → their copies.
    pub isovert_map: Vec<(VertId, VertId)>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_duplicate_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const DUPE_INPUT: u16 = 1;
const DUPE_NEW: u16 = 2;
const DUPE_DONE: u16 = 4;

struct Maps {
    v: rustc_hash::FxHashMap<VertId, VertId>,
    e: rustc_hash::FxHashMap<EdgeId, EdgeId>,
    out: DuplicateOut,
}

// Ported from blender/source/blender/bmesh/operators/bmo_dupe.cc (bmo_vert_copy).
fn vert_copy(bm: &mut BMesh, m: &mut Maps, v_src: VertId) -> VertId {
    let co = bm.vert(v_src).co;
    let v_dst = bm.vert_create(co, None, CreateFlag::SKIP_CD);
    m.out.vert_map.push((v_src, v_dst));
    m.v.insert(v_src, v_dst);
    bm.vert_attrs_copy(v_src, v_dst);
    bm.shape_copy(Some(v_src), v_dst);
    bm.oflag_enable(v_dst, DUPE_NEW);
    v_dst
}

// Ported from blender/source/blender/bmesh/operators/bmo_dupe.cc (bmo_edge_copy).
fn edge_copy(bm: &mut BMesh, m: &mut Maps, e_src: EdgeId, use_edge_flip_from_face: bool) -> EdgeId {
    let rlen = bm
        .edge_loops_iter(e_src)
        .filter(|&l| bm.oflag_test(bm.lp(l).f, DUPE_INPUT))
        .count();
    let [v1, v2] = bm.edge(e_src).v;
    let (d1, d2) = (m.v[&v1], m.v[&v2]);
    let e_dst = bm.edge_create(d1, d2, None, CreateFlag::SKIP_CD);
    m.out.edge_map.push((e_src, e_dst));
    if rlen < 2 {
        m.out.boundary_map.push((e_src, e_dst));
    }
    m.e.insert(e_src, e_dst);
    bm.edge_attrs_copy(e_src, e_dst);
    bm.oflag_enable(e_dst, DUPE_NEW);
    if use_edge_flip_from_face && bm.edge_is_boundary(e_src) {
        let l = bm.edge(e_src).l.expect("boundary edge");
        if bm.lp(l).v == v1 {
            // BM_edge_verts_swap.
            let ed = bm.edge_mut(e_dst);
            ed.v.swap(0, 1);
            ed.disk.swap(0, 1);
        }
    }
    e_dst
}

// Ported from blender/source/blender/bmesh/operators/bmo_dupe.cc (bmo_face_copy).
fn face_copy(bm: &mut BMesh, m: &mut Maps, f_src: FaceId) -> FaceId {
    // Small faces stay on the stack (M8 WP 8.8: three heap lists per face were a fifth of a
    // whole-mesh extrude on 100k faces).
    let loops: SmallVec<[LoopId; 8]> = bm.face_loops_iter(f_src).collect();
    let vtar: SmallVec<[VertId; 8]> = loops.iter().map(|&l| m.v[&bm.lp(l).v]).collect();
    let edar: SmallVec<[EdgeId; 8]> = loops.iter().map(|&l| m.e[&bm.lp(l).e]).collect();
    let f_dst = bm
        .face_create(&vtar, &edar, None, CreateFlag::SKIP_CD)
        .expect("source face has corners");
    m.out.face_map.push((f_src, f_dst));
    bm.face_attrs_copy(f_src, f_dst);
    let mut ld = bm.face(f_dst).l_first;
    for ls in loops {
        bm.loop_attrs_copy(ls, ld);
        ld = bm.lp(ld).next;
    }
    bm.oflag_enable(f_dst, DUPE_NEW);
    f_dst
}

/// Runs `duplicate` on `bm`.
pub fn duplicate(bm: &mut BMesh, i: &DuplicateIn) -> Result<DuplicateOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "duplicate: dead element {dead:?} in geom"
        )));
    }
    let out = bm.bmo_exec(TYPE_FLAG, |bm| {
        for &g in &i.geom {
            bm.oflag_enable(g, DUPE_INPUT);
        }
        let mut m = Maps {
            v: Default::default(),
            e: Default::default(),
            out: DuplicateOut::default(),
        };
        // Duplicate flagged vertices.
        for v in bm.vert_ids() {
            if bm.oflag_test(v, DUPE_INPUT) && !bm.oflag_test(v, DUPE_DONE) {
                frap_math::budget::check();
                let v2 = vert_copy(bm, &mut m, v);
                let mut isolated = !bm
                    .vert_faces_any_order(v)
                    .any(|f| bm.oflag_test(f, DUPE_INPUT));
                if isolated {
                    isolated = !bm.vert_edges_iter(v).any(|e| bm.oflag_test(e, DUPE_INPUT));
                }
                if isolated {
                    m.out.isovert_map.push((v, v2));
                }
                bm.oflag_enable(v, DUPE_DONE);
            }
        }
        // Now dupe all the edges.
        for e in bm.edge_ids() {
            if bm.oflag_test(e, DUPE_INPUT) && !bm.oflag_test(e, DUPE_DONE) {
                frap_math::budget::check();
                let [v1, v2] = bm.edge(e).v;
                for v in [v1, v2] {
                    if !bm.oflag_test(v, DUPE_DONE) {
                        vert_copy(bm, &mut m, v);
                        bm.oflag_enable(v, DUPE_DONE);
                    }
                }
                edge_copy(bm, &mut m, e, i.use_edge_flip_from_face);
                bm.oflag_enable(e, DUPE_DONE);
            }
        }
        // Dupe all flagged faces and their elements.
        for f in bm.face_ids() {
            if bm.oflag_test(f, DUPE_INPUT) {
                frap_math::budget::check();
                let loops: SmallVec<[LoopId; 8]> = bm.face_loops_iter(f).collect();
                for &l in &loops {
                    let v = bm.lp(l).v;
                    if !bm.oflag_test(v, DUPE_DONE) {
                        vert_copy(bm, &mut m, v);
                        bm.oflag_enable(v, DUPE_DONE);
                    }
                }
                for &l in &loops {
                    let e = bm.lp(l).e;
                    if !bm.oflag_test(e, DUPE_DONE) {
                        edge_copy(bm, &mut m, e, i.use_edge_flip_from_face);
                        bm.oflag_enable(e, DUPE_DONE);
                    }
                }
                face_copy(bm, &mut m, f);
                bm.oflag_enable(f, DUPE_DONE);
            }
        }
        if i.use_select_history {
            // BMO_mesh_selected_remap(check_select = false).
            let (vm, em) = (&m.v, &m.e);
            let fm: rustc_hash::FxHashMap<FaceId, FaceId> =
                m.out.face_map.iter().copied().collect();
            bm.select_history_remap(&|x| match x {
                BMElem::Vert(v) => vm.get(&v).map(|&n| BMElem::Vert(n)),
                BMElem::Edge(e) => em.get(&e).map(|&n| BMElem::Edge(n)),
                BMElem::Face(f) => fm.get(&f).map(|&n| BMElem::Face(n)),
            });
        }
        let mut out = m.out;
        out.geom_orig = i.geom.clone();
        // BMO_slot_buffer_from_enabled_flag(..., BM_ALL_NOLOOP, DUPE_NEW).
        for v in bm.vert_ids() {
            if bm.oflag_test(v, DUPE_NEW) {
                out.geom.push(BMElem::Vert(v));
            }
        }
        for e in bm.edge_ids() {
            if bm.oflag_test(e, DUPE_NEW) {
                out.geom.push(BMElem::Edge(e));
            }
        }
        for f in bm.face_ids() {
            if bm.oflag_test(f, DUPE_NEW) {
                out.geom.push(BMElem::Face(f));
            }
        }
        out
    });
    Ok(out)
}
