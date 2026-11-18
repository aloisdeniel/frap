// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.unsubdivide` (Blender 5.2.4 `bmo_unsubdivide_exec` and
//! `BM_mesh_decimate_unsubdivide_ex`), implemented by WP 2.6 (Subdivide and loops).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_unsubdivide.cc and
//! blender/source/blender/bmesh/tools/bmesh_decimate_unsubdivide.cc (Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.unsubdivide(bmesh, verts=[], iterations=0)`.
//!
//! **Create/kill order**: per iteration, the tagged (input) vertices that can be dissolved
//! (surrounded by 3 or 4 manifold edges, by 1 manifold and 2 boundary edges, or by 2 manifold
//! or 2 wire edges) are classified in a diamond pattern by alternately expanding "collapse"
//! and "keep" fronts from each untouched candidate, in vertex iteration order; then every
//! vertex marked for collapse is dissolved in vertex iteration order. A fan vertex first has
//! the corner of each n-gon around it split off (`BM_face_split` from `l->prev` to `l->next`:
//! one edge and one face each), then `BM_vert_dissolve` joins the faces and kills the vertex;
//! a chain vertex is collapsed onto its edge (`BM_vert_collapse_edge`). Freed slots are reused
//! LIFO by the next allocations. Vertex indices are scratch storage and left dirty.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE` (plus
//! `UNTAN_MULTIRES`).

pub(crate) mod mods;

use crate::types::{ElemFlag, ElemTypes, LoopId, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, VertId};

/// Input slots of `unsubdivide` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnsubdivideIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `iterations`.
    pub iterations: i32,
}

/// Output slots of `unsubdivide` (Blender's op has none; `geom` stays empty).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct UnsubdivideOut {
    /// Every element the op created or kept as its result (`geom.out`-like): empty.
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_unsubdivide_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// Vertices with more than this number of connected edges/faces are not dissolved.
const VERT_DISSOLVE_MAX: usize = 4;

/// The kind of dissolve a vertex may use.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum VertDissolveMethod {
    /// Surrounded by 3/4 manifold edges.
    InteriorFan,
    /// Surrounded by 2 manifold edges.
    InteriorChain,
    /// 1 manifold edge and 2 boundary edges.
    BoundaryFan,
    /// 2 wire edges.
    WireChain,
}

const VERT_INDEX_DO_COLLAPSE: i32 = -1;
const VERT_INDEX_INIT: i32 = 0;
const VERT_INDEX_IGNORE: i32 = 1;

/// `bm_vert_loops_from_vert_edges`: the corners of `v` in the faces of its edges.
fn vert_loops_from_vert_edges(bm: &BMesh, v: VertId, v_edges: &[EdgeId]) -> Vec<LoopId> {
    let mut out = Vec::new();
    for &e in v_edges {
        for l in bm.edge_loops(e) {
            if bm.lp(l).v == v {
                if out.len() == VERT_DISSOLVE_MAX {
                    return out;
                }
                out.push(l);
            }
        }
    }
    out
}

/// `bm_vert_dissolve_fan_makes_double_whole_face`.
fn fan_makes_double_whole_face(bm: &mut BMesh, v_loops: &[LoopId]) -> bool {
    let mut ears: Vec<EdgeId> = Vec::new();
    for &l in v_loops {
        let ll = bm.lp(l).clone();
        let e_opposite = if bm.face(ll.f).len > 3 {
            let Some(e) = bm.edge_exists(bm.lp(ll.prev).v, bm.lp(ll.next).v) else {
                return false;
            };
            if ears.len() == VERT_DISSOLVE_MAX {
                return false;
            }
            e
        } else {
            bm.lp(ll.next).e
        };
        ears.push(e_opposite);
    }
    ears.len() >= 3 && mods::face_exists_multi_edge(bm, &ears)
}

/// `bm_vert_dissolve_fan_makes_double_with_existing`.
fn fan_makes_double_with_existing(bm: &BMesh, v_loops: &[LoopId]) -> bool {
    v_loops.iter().any(|&l| {
        let ll = bm.lp(l);
        bm.face(ll.f).len > 3 && mods::face_split_check_double_face(bm, ll.prev, ll.next, 3)
    })
}

/// `bm_vert_dissolve_fan_makes_double_with_self`: two n-gons around the vertex collapse to
/// the same face.
fn fan_makes_double_with_self(bm: &BMesh, v_loops_all: &[LoopId]) -> bool {
    if v_loops_all.len() < 2 {
        return false;
    }
    let mut v_loops: Vec<LoopId> = v_loops_all
        .iter()
        .copied()
        .filter(|&l| bm.face(bm.lp(l).f).len > 3)
        .collect();
    if v_loops.len() < 2 {
        return false;
    }
    v_loops.sort_by_key(|&l| bm.face(bm.lp(l).f).len);
    // The vertices left when the central vertex is collapsed out, canonicalised (rotated to
    // the minimum, in the direction of its smaller neighbour).
    let canonical = |l_vert: LoopId| -> Vec<VertId> {
        let mut verts = Vec::new();
        let mut l = bm.lp(l_vert).next;
        while l != l_vert {
            verts.push(bm.lp(l).v);
            l = bm.lp(l).next;
        }
        let n = verts.len();
        let i_min = (0..n).min_by_key(|&i| verts[i]).unwrap_or(0);
        let i_next = if i_min + 1 == n { 0 } else { i_min + 1 };
        let i_prev = if i_min == 0 { n - 1 } else { i_min - 1 };
        let step = if verts[i_next] < verts[i_prev] { 1 } else { n - 1 };
        let mut out = Vec::with_capacity(n);
        let mut j = i_min;
        for _ in 0..n {
            out.push(verts[j]);
            j += step;
            if j >= n {
                j -= n;
            }
        }
        out
    };
    let mut beg = 0;
    while beg < v_loops.len() {
        let len = bm.face(bm.lp(v_loops[beg]).f).len;
        let mut end = beg + 1;
        while end < v_loops.len() && bm.face(bm.lp(v_loops[end]).f).len == len {
            end += 1;
        }
        if end - beg >= 2 {
            let collapsed: Vec<Vec<VertId>> = (beg..end).map(|i| canonical(v_loops[i])).collect();
            for i in 0..collapsed.len() {
                for j in i + 1..collapsed.len() {
                    if collapsed[i] == collapsed[j] {
                        return true;
                    }
                }
            }
        }
        beg = end;
    }
    false
}

/// `bm_vert_dissolve_fan_or_chain_test`: how `v` can be dissolved (with `check_for_duplicates`
/// the surrounding corners are returned and dissolves that would create duplicate faces are
/// rejected).
fn dissolve_test(
    bm: &mut BMesh,
    v: VertId,
    check_for_duplicates: bool,
) -> Option<(VertDissolveMethod, Vec<LoopId>)> {
    let mut v_edges: Vec<EdgeId> = Vec::new();
    let mut e_boundary: Vec<EdgeId> = Vec::new();
    let (mut tot_boundary, mut tot_manifold, mut tot_wire) = (0usize, 0usize, 0usize);
    for e in bm.vert_edges(v) {
        if bm.edge_is_boundary(e) {
            if e_boundary.len() < 2 {
                e_boundary.push(e);
            }
            tot_boundary += 1;
        } else if bm.edge_is_manifold(e) {
            tot_manifold += 1;
        } else if bm.edge_is_wire(e) {
            tot_wire += 1;
        } else {
            return None;
        }
        if v_edges.len() == VERT_DISSOLVE_MAX {
            return None;
        }
        v_edges.push(e);
    }
    let tot_edge = v_edges.len();
    if (tot_edge == 4 && tot_boundary == 0 && tot_manifold == 4)
        || (tot_edge == 3 && tot_boundary == 0 && tot_manifold == 3)
    {
        let mut v_loops = Vec::new();
        if check_for_duplicates {
            v_loops = vert_loops_from_vert_edges(bm, v, &v_edges);
            if fan_makes_double_whole_face(bm, &v_loops)
                || fan_makes_double_with_existing(bm, &v_loops)
                || fan_makes_double_with_self(bm, &v_loops)
            {
                return None;
            }
        }
        return Some((VertDissolveMethod::InteriorFan, v_loops));
    }
    if tot_edge == 3 && tot_boundary == 2 && tot_manifold == 1 {
        let mut v_loops = Vec::new();
        if check_for_duplicates {
            let a = bm.edge_other_vert(e_boundary[0], v);
            let b = bm.edge_other_vert(e_boundary[1], v);
            if let Some(e_span) = bm.edge_exists(a, b)
                && bm.edge(e_span).l.is_some()
            {
                // The dissolve could join across `e_span` and remove other vertices.
                return None;
            }
            v_loops = vert_loops_from_vert_edges(bm, v, &v_edges);
            if fan_makes_double_with_existing(bm, &v_loops) || fan_makes_double_with_self(bm, &v_loops) {
                return None;
            }
        }
        return Some((VertDissolveMethod::BoundaryFan, v_loops));
    }
    if tot_edge == 2 && tot_wire == 2 {
        return Some((VertDissolveMethod::WireChain, Vec::new()));
    }
    if tot_edge == 2 && tot_manifold == 2 {
        return Some((VertDissolveMethod::InteriorChain, Vec::new()));
    }
    None
}

/// `bm_vert_dissolve_fan_or_chain`: dissolves `v` (and only `v`).
fn dissolve_fan_or_chain(bm: &mut BMesh, v: VertId) -> bool {
    let Some((method, v_loops)) = dissolve_test(bm, v, true) else {
        return false;
    };
    match method {
        VertDissolveMethod::WireChain | VertDissolveMethod::InteriorChain => {
            let e = bm.vert(v).e.expect("chain vertex has edges");
            mods::vert_collapse_edge(bm, e, v, true, true, true).is_some()
        }
        VertDissolveMethod::BoundaryFan | VertDissolveMethod::InteriorFan => {
            for l in v_loops {
                let ll = bm.lp(l).clone();
                if bm.face(ll.f).len > 3
                    && let Some((_, l_new)) = bm.face_split(ll.f, ll.prev, ll.next, None, true)
                {
                    // `BM_elem_flag_merge_into(l_new->e, l->e, l->prev->e)`.
                    let fa = bm.edge(bm.lp(l).e).flag;
                    let fb = bm.edge(bm.lp(bm.lp(l).prev).e).flag;
                    let e_new = bm.lp(l_new).e;
                    bm.edge_mut(e_new).flag = fa | fb;
                }
            }
            mods::vert_dissolve(bm, v)
        }
    }
}

/// `bm_tag_untagged_neighbors`: tags the untagged neighbours of `verts_start` with
/// `desired_tag` and returns them.
fn tag_untagged_neighbors(bm: &mut BMesh, verts_start: &[VertId], desired_tag: i32) -> Vec<VertId> {
    let mut out = Vec::new();
    for &v in verts_start {
        for e in bm.vert_edges(v) {
            let v_other = bm.edge_other_vert(e, v);
            if bm.index_get(BMElem::Vert(v_other)) == VERT_INDEX_INIT {
                bm.index_set(BMElem::Vert(v_other), desired_tag);
                out.push(v_other);
            }
        }
    }
    out
}

/// `BM_mesh_decimate_unsubdivide_ex(bm, iterations, tag_only)`: with `tag_only` only the
/// vertices tagged `BM_ELEM_TAG` are candidates.
pub fn decimate_unsubdivide_ex(bm: &mut BMesh, iterations: i32, tag_only: bool) {
    if !tag_only {
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
    }
    for _ in 0..iterations {
        let mut marked = false;
        // Tag every vertex eligible to be dissolved in this iteration.
        for v in bm.vert_ids() {
            frap_math::budget::check();
            let ok = bm.vert(v).flag.contains(ElemFlag::TAG) && dissolve_test(bm, v, false).is_some();
            bm.index_set(
                BMElem::Vert(v),
                if ok {
                    VERT_INDEX_INIT
                } else {
                    VERT_INDEX_IGNORE
                },
            );
        }
        // Keep tagging until no island is left.
        for v in bm.vert_ids() {
            if bm.index_get(BMElem::Vert(v)) != VERT_INDEX_INIT {
                continue;
            }
            bm.index_set(BMElem::Vert(v), VERT_INDEX_IGNORE);
            let mut verts_ignore = vec![v];
            loop {
                frap_math::budget::check();
                let verts_collapse = tag_untagged_neighbors(bm, &verts_ignore, VERT_INDEX_DO_COLLAPSE);
                if verts_collapse.is_empty() {
                    break;
                }
                marked = true;
                verts_ignore = tag_untagged_neighbors(bm, &verts_collapse, VERT_INDEX_IGNORE);
                if verts_ignore.is_empty() {
                    break;
                }
            }
        }
        // Later steps can run out of eligible vertices.
        if !marked {
            break;
        }
        // Remove every vertex tagged for removal (`BM_ITER_MESH_MUTABLE`).
        for v in bm.vert_ids() {
            frap_math::budget::check();
            if bm.verts.contains(v.0) && bm.index_get(BMElem::Vert(v)) == VERT_INDEX_DO_COLLAPSE {
                dissolve_fan_or_chain(bm, v);
            }
        }
    }
    bm.index.dirty |= ElemTypes::VERT;
}

/// Runs `unsubdivide` on `bm`.
pub fn unsubdivide(bm: &mut BMesh, i: &UnsubdivideIn) -> Result<UnsubdivideOut, BmError> {
    for &v in &i.verts {
        if !bm.verts.contains(v.0) {
            return Err(BmError::Invalid(format!("verts: no vertex {}", v.0)));
        }
    }
    let iterations = i.iterations.max(1);
    bm.bmo_exec(TYPE_FLAG, |bm| {
        // `BMVert.flag & BM_ELEM_TAG` marks the candidates.
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
        for &v in &i.verts {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
        decimate_unsubdivide_ex(bm, iterations, true);
    });
    Ok(UnsubdivideOut::default())
}
