// SPDX-License-Identifier: GPL-3.0-or-later
//! The two rip paths of `edbm_rip_invoke` (one selected vertex, or edges), the side
//! selection (`ripsel`) and the optional face fill.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_rip.cc (Blender 5.2.4,
//! GPL-2.0-or-later), with `BM_vert_is_manifold_region`, `BM_vert_other_disk_edge`,
//! `BM_loop_other_vert_loop`, `BM_loop_calc_face_tangent`, `BM_edge_loop_pair`
//! (`bmesh/intern/bmesh_query.cc`) and `BM_mesh_select_mode_clean_ex`
//! (`bmesh/intern/bmesh_marking.cc`).

use std::collections::HashSet;

use frap_bmesh::ops::contextual_create::construct::vert_is_wire;
use frap_bmesh::ops::split_edges::mesh_edgesplit;
use frap_bmesh::{BMElem, BMesh, CreateFlag, EdgeId, ElemFlag, LoopId, VertId};

use super::super::rip_edge::math::{
    V2, V3, add3, cross3, dist_squared_to_line_segment_v2, dot3, interp_v2_v2v2, len_v2v2,
    len_v3v3, mid_v3_v3v3, midpoint3, normalize_v2_length, normalize_v3, sub2, sub3,
};
use super::kernel::{face_loop_separate_multi, face_loop_separate_multi_isolated};
use super::view::Projection;

/// `INSET_DEFAULT`.
const INSET_DEFAULT: f32 = 0.00001;
/// `INVALID_UID`.
const INVALID_UID: i32 = i32::MIN;

/// The outcome of one rip path (`OPERATOR_FINISHED` / `OPERATOR_CANCELLED`).
pub(crate) type Finished = bool;

/// The region data both paths measure with.
pub(crate) struct RipView {
    pub proj: Projection,
    pub fmval: V2,
}

fn hidden(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::HIDDEN)
}
fn selected(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::SELECT)
}
fn tagged(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::TAG)
}
fn set_tag(bm: &mut BMesh, e: EdgeId) {
    bm.edge_mut(e).flag.insert(ElemFlag::TAG);
}

/// `edbm_rip_edgedist_squared`: the squared region distance of the mouse to the segment, the
/// segment shortened by `inset` pixels at each end.
fn edgedist_squared(rv: &RipView, co1: V3, co2: V3, inset: f32) -> f32 {
    let mut vec1 = rv.proj.project(co1);
    let mut vec2 = rv.proj.project(co2);
    if inset != 0.0 {
        let dist_2d = len_v2v2(vec1, vec2);
        if dist_2d > f32::EPSILON {
            let dist = inset / dist_2d;
            vec1 = interp_v2_v2v2(vec1, vec2, dist);
            vec2 = interp_v2_v2v2(vec2, vec1, dist);
        }
    }
    dist_squared_to_line_segment_v2(rv.fmval, vec1, vec2)
}

/// `BM_loop_calc_face_tangent`.
fn loop_calc_face_tangent(bm: &BMesh, l: LoopId) -> V3 {
    let ll = bm.lp(l);
    let co = bm.vert(ll.v).co;
    let v_prev = normalize_v3(sub3(bm.vert(bm.lp(ll.prev).v).co, co)).0;
    let v_next = normalize_v3(sub3(co, bm.vert(bm.lp(ll.next).v).co)).0;
    let dir = add3(v_prev, v_next);
    let fno = bm.face(ll.f).no;
    // `compare_v3v3(v_prev, v_next, FLT_EPSILON * 10)`.
    let limit = f32::EPSILON * 10.0;
    let same = (0..3).all(|k| (v_prev[k] - v_next[k]).abs() <= limit);
    let t = if !same {
        let mut nor = cross3(v_prev, v_next);
        // Concave face check.
        if dot3(nor, fno) < 0.0 {
            nor = [-nor[0], -nor[1], -nor[2]];
        }
        cross3(dir, nor)
    } else {
        cross3(dir, fno)
    };
    normalize_v3(t).0
}

fn edge_calc_length(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    len_v3v3(bm.vert(a).co, bm.vert(b).co)
}

/// `edbm_calc_loop_co`: a point along the loop tangent, about one edge length away.
fn calc_loop_co(bm: &BMesh, l: LoopId) -> V3 {
    let t = loop_calc_face_tangent(bm, l);
    let ll = bm.lp(l);
    let f = (edge_calc_length(bm, ll.e) + edge_calc_length(bm, bm.lp(ll.prev).e)) / 2.0;
    add3([t[0] * f, t[1] * f, t[2] * f], bm.vert(ll.v).co)
}

/// `BM_face_other_vert_loop(f, v_prev, v)->v`.
fn face_other_vert(bm: &BMesh, l_in_face: LoopId, v_prev: VertId, v: VertId) -> VertId {
    let f = bm.lp(l_in_face).f;
    let l = bm
        .face_other_vert_loop(f, v_prev, v)
        .expect("rip: edge vertices are adjacent in the face");
    bm.lp(l).v
}

/// `edbm_rip_edge_side_measure`: the edge's projected length, negative when the mouse is not
/// on the side of the face of `e_l`.
fn edge_side_measure(bm: &BMesh, e: EdgeId, e_l: LoopId, rv: &RipView) -> f32 {
    let [ev1, ev2] = bm.edge(e).v;
    // Rather than the face centre, the middle of both edge verts connected to this one.
    let v1_other = face_other_vert(bm, e_l, ev2, ev1);
    let v2_other = face_other_vert(bm, e_l, ev1, ev2);
    let cent = rv
        .proj
        .project(midpoint3(bm.vert(v1_other).co, bm.vert(v2_other).co));
    let mid = rv.proj.project(midpoint3(bm.vert(ev1).co, bm.vert(ev2).co));
    let e_v1_co = rv.proj.project(bm.vert(ev1).co);
    let e_v2_co = rv.proj.project(bm.vert(ev2).co);
    let vec = normalize_v2_length(sub2(cent, mid), 0.01);
    // Rather than adding to both verts, subtract from the mouse.
    let fmval_tweak = sub2(rv.fmval, vec);
    let score = len_v2v2(e_v1_co, e_v2_co);
    if dist_squared_to_line_segment_v2(fmval_tweak, e_v1_co, e_v2_co)
        > dist_squared_to_line_segment_v2(rv.fmval, e_v1_co, e_v2_co)
    {
        score
    } else {
        -score
    }
}

/// `BM_edge_loop_pair`.
fn edge_loop_pair(bm: &BMesh, e: EdgeId) -> Option<(LoopId, LoopId)> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    (la != lb && bm.lp(lb).radial_next == la).then_some((la, lb))
}

/// `BM_edge_vert_share_loop(l, v)`.
fn edge_vert_share_loop(bm: &BMesh, l: LoopId, v: VertId) -> LoopId {
    if bm.lp(l).v == v { l } else { bm.lp(l).next }
}

/// `BM_edge_in_loop(e, l)`.
fn edge_in_loop(bm: &BMesh, e: EdgeId, l: LoopId) -> bool {
    bm.lp(l).e == e || bm.lp(bm.lp(l).prev).e == e
}

/// `BM_loop_other_vert_loop`.
fn loop_other_vert_loop(bm: &BMesh, l: LoopId, v: VertId) -> LoopId {
    let ll = bm.lp(l);
    let v_prev = bm.edge_other_vert(ll.e, v);
    if ll.v == v {
        if bm.lp(ll.prev).v == v_prev {
            ll.next
        } else {
            ll.prev
        }
    } else if bm.lp(ll.prev).v == v {
        bm.lp(ll.prev).prev
    } else {
        bm.lp(ll.next).next
    }
}

/// `BM_vert_other_disk_edge`: the edge across the fan of a manifold vertex.
fn vert_other_disk_edge(bm: &BMesh, v: VertId, e_first: EdgeId) -> Option<EdgeId> {
    let step = |l_a: LoopId| -> (LoopId, bool) {
        let mut l = loop_other_vert_loop(bm, l_a, v);
        if !bm.edge(bm.lp(l).e).v.contains(&v) {
            l = bm.lp(l).prev;
        }
        if bm.edge_is_manifold(bm.lp(l).e) {
            (bm.lp(l).radial_next, true)
        } else {
            (l, false)
        }
    };
    let first = bm.edge(e_first).l?;
    let mut tot = 0;
    let mut l_a = first;
    loop {
        let (n, ok) = step(l_a);
        if !ok {
            return None;
        }
        l_a = n;
        tot += 1;
        if l_a == first {
            break;
        }
    }
    // Loop half way.
    tot /= 2;
    let mut i = 0;
    let mut l_a = first;
    loop {
        if i == tot {
            let l = if bm.edge(bm.lp(l_a).e).v.contains(&v) {
                l_a
            } else {
                bm.lp(l_a).prev
            };
            return Some(bm.lp(l).e);
        }
        l_a = step(l_a).0;
        i += 1;
        if l_a == first {
            break;
        }
    }
    None
}

/// `BM_vert_find_first_loop`.
fn vert_find_first_loop(bm: &BMesh, v: VertId) -> Option<LoopId> {
    bm.vert_loops(v).first().copied()
}

/// `BM_vert_is_manifold_region`: the loops reachable from the first loop of `v` through its
/// edges are all of its loops.
fn vert_is_manifold_region(bm: &BMesh, v: VertId) -> bool {
    let Some(l_first) = vert_find_first_loop(bm, v) else {
        return true;
    };
    // `bm_loop_region_count__recursive`.
    fn count(
        bm: &BMesh,
        e: EdgeId,
        v: VertId,
        ev: &mut HashSet<EdgeId>,
        lv: &mut HashSet<LoopId>,
    ) -> usize {
        ev.insert(e);
        let mut n = 0;
        let Some(first) = bm.edge(e).l else {
            return 0;
        };
        let mut l = first;
        loop {
            let ll = bm.lp(l);
            if ll.v == v {
                let e_other = bm.lp(ll.prev).e;
                if lv.insert(l) {
                    n += 1;
                }
                if !ev.contains(&e_other) {
                    n += count(bm, e_other, v, ev, lv);
                }
            } else if bm.lp(ll.next).v == v {
                let e_other = bm.lp(ll.next).e;
                if lv.insert(ll.next) {
                    n += 1;
                }
                if !ev.contains(&e_other) {
                    n += count(bm, e_other, v, ev, lv);
                }
            }
            l = bm.lp(l).radial_next;
            if l == first {
                break;
            }
        }
        n
    }
    let (mut ev, mut lv) = (HashSet::new(), HashSet::new());
    let c = count(bm, bm.lp(l_first).e, v, &mut ev, &mut lv);
    // `bm_loop_region_count__clear`: every loop of `v` on its edges.
    let total = bm.vert_loops(v).len();
    c == total
}

// --------------------------------------------------------------------------- face fill

/// `UnorderedLoopPair`.
#[derive(Copy, Clone)]
struct LoopPair {
    l_pair: [Option<LoopId>; 2],
    flip0: bool,
    flip1: bool,
}

/// `edbm_tagged_loop_pairs_to_fill`.
fn tagged_loop_pairs_to_fill(bm: &BMesh) -> Option<Vec<LoopPair>> {
    let mut out = Vec::new();
    for (i, e) in bm.edges.iter() {
        if !e.flag.contains(ElemFlag::TAG) {
            continue;
        }
        let e = EdgeId(i);
        out.push(match edge_loop_pair(bm, e) {
            Some((l1, l2)) => {
                let v_cmp = bm.edge(bm.lp(l1).e).v[0];
                LoopPair {
                    l_pair: [Some(l1), Some(l2)],
                    flip0: bm.lp(l1).v != v_cmp,
                    flip1: bm.lp(l2).v == v_cmp,
                }
            }
            None => LoopPair {
                l_pair: [None, None],
                flip0: false,
                flip1: false,
            },
        });
    }
    (!out.is_empty()).then_some(out)
}

/// `BM_edge_share_vert`.
fn edge_share_vert(bm: &BMesh, e1: EdgeId, e2: EdgeId) -> Option<VertId> {
    let a = bm.edge(e1).v;
    let b = bm.edge(e2).v;
    if b.contains(&a[0]) {
        Some(a[0])
    } else if b.contains(&a[1]) {
        Some(a[1])
    } else {
        None
    }
}

/// `edbm_tagged_loop_pairs_do_fill_faces`.
fn tagged_loop_pairs_do_fill_faces(bm: &mut BMesh, pairs: &[LoopPair], normals: &mut ClnorKeep) {
    for ulp in pairs {
        let (Some(l0), Some(l1)) = (ulp.l_pair[0], ulp.l_pair[1]) else {
            continue;
        };
        let (e0, e1) = (bm.lp(l0).e, bm.lp(l1).e);
        if e0 == e1 {
            continue;
        }
        // Time has come to make a face!
        let v_shared = edge_share_vert(bm, e0, e1);
        let f_example = bm.lp(l0).f;
        let verts: Vec<VertId> = match v_shared {
            None => {
                // Quad.
                let mut fv = [
                    bm.edge(e0).v[0],
                    bm.edge(e1).v[0],
                    bm.edge(e1).v[1],
                    bm.edge(e0).v[1],
                ];
                if ulp.flip0 {
                    fv.swap(0, 3);
                }
                if ulp.flip1 {
                    fv.swap(1, 2);
                }
                fv.to_vec()
            }
            Some(vs) => {
                // Tri: the flip flags are not used.
                let mut fv = [vs, bm.edge_other_vert(e0, vs), bm.edge_other_vert(e1, vs)];
                if vs == bm.lp(l0).v {
                    fv.swap(0, 1);
                }
                fv.to_vec()
            }
        };
        let Some(f) = bm.face_create_verts(&verts, Some(f_example), CreateFlag::empty(), true)
        else {
            continue;
        };
        let loops = bm.face_loops(f);
        let srcs: Vec<EdgeId> = if verts.len() == 4 {
            vec![e0, e1, e1, e0]
        } else {
            vec![e0, e0, e1]
        };
        for (&l, &e) in loops.iter().zip(&srcs) {
            let src = bm.edge_other_loop(e, l);
            bm.loop_attrs_copy(src, l);
            normals.copy(src, l);
        }
    }
}

// --------------------------------------------------------------------- custom normals

/// `BM_custom_loop_normals_to_vector_layer` / `…_from_vector_layer`: the custom normals as
/// vectors by loop slot while the topology changes, re-encoded in the new corner spaces
/// afterwards.
#[derive(Default)]
pub(crate) struct ClnorKeep {
    vecs: Option<Vec<Option<V3>>>,
}

impl ClnorKeep {
    pub(crate) fn to_vector_layer(bm: &mut BMesh) -> ClnorKeep {
        if !frap_bmesh::clnor::has_layer(bm) {
            return ClnorKeep::default();
        }
        let sp = frap_bmesh::clnor::update(bm);
        let mut vecs = vec![None; bm.loops.capacity()];
        for f in bm.face_ids() {
            for l in bm.face_loops(f) {
                vecs[l.slot()] = Some(sp.decode(bm, l));
            }
        }
        ClnorKeep { vecs: Some(vecs) }
    }
    fn copy(&mut self, src: LoopId, dst: LoopId) {
        if let Some(v) = &mut self.vecs {
            if v.len() <= dst.slot() {
                v.resize(dst.slot() + 1, None);
            }
            v[dst.slot()] = v.get(src.slot()).copied().flatten();
        }
    }
    pub(crate) fn restore(self, bm: &mut BMesh) {
        let Some(vecs) = self.vecs else {
            return;
        };
        bm.lnor_spacearr.invalidate_all();
        let sp = frap_bmesh::clnor::rebuild(bm);
        for f in bm.face_ids() {
            for l in bm.face_loops(f) {
                if let Some(Some(n)) = vecs.get(l.slot()) {
                    let data = sp.encode(l, *n);
                    frap_bmesh::clnor::set(bm, l, data);
                }
            }
        }
    }
}

// ------------------------------------------------------------------------- vertex rip

/// `edbm_rip_invoke__vert`: rip when one vertex is selected.
pub(crate) fn rip_vert(
    bm: &mut BMesh,
    rv: &RipView,
    do_fill: bool,
    normals: &mut ClnorKeep,
) -> Finished {
    let totvert_orig = bm.verts_num();
    // Find the selected vertex: the active history entry first.
    let ese = match bm.select_history_active() {
        Some(BMElem::Vert(v)) => Some(v),
        _ => None,
    };
    let v = match ese {
        Some(v) => Some(v),
        None => bm
            .verts
            .iter()
            .find(|(_, v)| v.flag.contains(ElemFlag::SELECT))
            .map(|(i, _)| VertId(i)),
    };
    let Some(v) = v else {
        return false;
    };
    if bm.vert(v).e.is_none() {
        return false;
    }
    let is_wire = vert_is_wire(bm, v);
    let is_manifold_region = vert_is_manifold_region(bm, v);
    let mut e_best: Option<EdgeId> = None;
    let mut dist_sq = f32::MAX;
    let mut totboundary_edge = 0;
    // Find the closest edge to the mouse.
    for e in bm.vert_edges(v) {
        // Wire counts as boundary here, else a face cannot be ripped away from a wire edge.
        totboundary_edge += usize::from(bm.edge_is_boundary(e) || bm.edge_is_wire(e));
        if !hidden(bm, e) && (!is_manifold_region || bm.edge_is_manifold(e)) {
            let [a, b] = bm.edge(e).v;
            let d = edgedist_squared(rv, bm.vert(a).co, bm.vert(b).co, INSET_DEFAULT);
            if e_best.is_none() || d < dist_sq {
                dist_sq = d;
                e_best = Some(e);
            }
        }
    }

    if let Some(eb) = e_best
        && let Some(el) = bm.edge(eb).l
        && !is_manifold_region
    {
        // Split off a non-manifold fan (several disconnected fans).
        let l_sep = if bm.lp(el).v == v { el } else { bm.lp(el).next };
        let v_new = face_loop_separate_multi_isolated(bm, l_sep);
        bm.vert_select_set(v, false);
        bm.select_history_remove(BMElem::Vert(v));
        bm.vert_select_set(v_new, true);
        if ese.is_some() {
            bm.select_history_store(BMElem::Vert(v_new));
        }
        if do_fill {
            bm.edge_create(v, v_new, None, CreateFlag::empty());
        }
        return true;
    }

    // Ripping a single vertex from 3 faces: measure the distance to the corners too.
    if bm.vert_face_count(v) == 3 && bm.vert_edge_count(v) == 3 {
        let e_all = bm.vert_edges(v);
        let l_all = bm.vert_loops(v);
        for &l in l_all.iter().take(3) {
            let l_mid_co = calc_loop_co(bm, l);
            let d = edgedist_squared(rv, bm.vert(bm.lp(l).v).co, l_mid_co, INSET_DEFAULT);
            if e_best.is_none() || d < dist_sq {
                dist_sq = d;
                // The edge that is not in this loop.
                e_best = e_all.iter().copied().find(|&e| !edge_in_loop(bm, e, l));
            }
        }
    }

    // Split off the vertex when no edge can be split: it joins faces only at itself, more
    // than two boundary edges (nicer than an edge split), or a wire vertex with two edges.
    if (!is_wire && totboundary_edge > 2) || (is_wire && totboundary_edge > 1) {
        bm.vert_select_set(v, false);
        let mut vout = bm.vert_separate(v, &[], true);
        if vout.len() < 2 {
            // Set the selection back to avoid an active unselected vertex.
            bm.vert_select_set(v, true);
            return false;
        }
        if let Some(a) = ese {
            bm.select_history_remove(BMElem::Vert(a));
        }
        let mut vi_best = 0;
        let mut dist_sq = f32::MAX;
        let vco = bm.vert(v).co;
        // The best vertex to drag: by its face corner, or its edge without faces.
        for (i, &vo) in vout.iter().enumerate() {
            if !vert_is_wire(bm, vo) {
                for l in bm.vert_loops(vo) {
                    if hidden(bm, bm.lp(l).f) {
                        continue;
                    }
                    let l_mid_co = calc_loop_co(bm, l);
                    let d = edgedist_squared(rv, vco, l_mid_co, INSET_DEFAULT);
                    if d < dist_sq {
                        dist_sq = d;
                        vi_best = i;
                    }
                }
            } else {
                for e in bm.vert_edges(vo) {
                    if hidden(bm, e) {
                        continue;
                    }
                    let [a, b] = bm.edge(e).v;
                    let e_mid_co = mid_v3_v3v3(bm.vert(a).co, bm.vert(b).co);
                    let d = edgedist_squared(rv, vco, e_mid_co, INSET_DEFAULT);
                    if d < dist_sq {
                        dist_sq = d;
                        vi_best = i;
                    }
                }
            }
        }
        // vout[0] = best, vout[1] = glue, vout[2..] = spliced with the glue.
        vout.swap(0, vi_best);
        let v = vout[0];
        bm.vert_select_set(v, true);
        if ese.is_some() {
            bm.select_history_store(BMElem::Vert(v));
        }
        for i in 2..vout.len() {
            bm.vert_splice(vout[1], vout[i]);
        }
        if do_fill {
            // Match the extrude vertex order.
            bm.edge_create(vout[1], vout[0], None, CreateFlag::empty());
        }
        return true;
    }

    let Some(e_best) = e_best else {
        return false;
    };

    // Execute the split.
    let mut larr: Vec<LoopId> = Vec::new();
    if bm.edge_is_boundary(e_best) || bm.vert_face_count(v) == 2 {
        // Rip two adjacent edges (no edge split).
        let l = edge_vert_share_loop(bm, bm.edge(e_best).l.expect("face edge"), v);
        larr.push(l);
        // Only tag for face fill.
        if bm.edge_is_boundary(e_best) {
            set_tag(bm, e_best);
        } else {
            let (le, lpe) = (bm.lp(l).e, bm.lp(bm.lp(l).prev).e);
            set_tag(bm, le);
            set_tag(bm, lpe);
        }
    } else if bm.edge_is_manifold(e_best) {
        let l_first = bm.edge(e_best).l.expect("manifold");
        let mut l_iter = l_first;
        loop {
            let l = edge_vert_share_loop(bm, l_iter, v);
            if do_fill {
                // Never tag the best edge, it does not change during the split (#44618).
                let t = if bm.lp(l).e == e_best {
                    bm.lp(bm.lp(l).prev).e
                } else {
                    bm.lp(l).e
                };
                set_tag(bm, t);
            }
            larr.push(l);
            l_iter = bm.lp(l_iter).radial_next;
            if l_iter == l_first {
                break;
            }
        }
    }
    // Keep directly before the split.
    let fill_pairs = if do_fill {
        tagged_loop_pairs_to_fill(bm)
    } else {
        None
    };
    let v_rip = (!larr.is_empty()).then(|| face_loop_separate_multi(bm, &larr));
    match v_rip {
        Some(vr) => bm.vert_select_set(vr, true),
        None => return false,
    }

    // Select which vertex.
    let mut v_best = None;
    let mut dist_sq = f32::MAX;
    for v in bm.vert_ids() {
        if !selected(bm, v) {
            continue;
        }
        // Deselect by default, the winner is selected again at the end.
        bm.vert_select_set(v, false);
        bm.select_history_remove(BMElem::Vert(v));
        for l in bm.vert_loops(v) {
            let l_corner_co = calc_loop_co(bm, l);
            let d = edgedist_squared(rv, bm.vert(bm.lp(l).v).co, l_corner_co, INSET_DEFAULT);
            if v_best.is_none() || d < dist_sq {
                v_best = Some(v);
                dist_sq = d;
            }
        }
    }
    if let Some(vb) = v_best {
        bm.vert_select_set(vb, true);
        if ese.is_some() {
            bm.select_history_store(BMElem::Vert(vb));
        }
    }
    if let Some(p) = fill_pairs {
        tagged_loop_pairs_do_fill_faces(bm, &p, normals);
    }
    totvert_orig != bm.verts_num()
}

// --------------------------------------------------------------------------- edge rip

/// The loop uids of `edbm_ripsel_looptag_helper` (`BM_elem_index_set` on loops), by loop
/// slot.
struct LoopUid(Vec<i32>);

impl LoopUid {
    fn get(&self, l: LoopId) -> i32 {
        self.0.get(l.slot()).copied().unwrap_or(INVALID_UID)
    }
    fn set(&mut self, l: LoopId, uid: i32) {
        if self.0.len() <= l.slot() {
            self.0.resize(l.slot() + 1, INVALID_UID);
        }
        self.0[l.slot()] = uid;
    }
    /// `BM_elem_index_get(e->l)`; a wire edge has no loop (Blender would read through a null
    /// pointer; it never matches here).
    fn of_edge(&self, bm: &BMesh, e: EdgeId) -> Option<i32> {
        bm.edge(e).l.map(|l| self.get(l))
    }
}

/// `IS_VISIT_POSSIBLE(e)`.
fn visit_possible(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge_is_manifold(e) && tagged(bm, e)
}
/// `IS_VISIT_DONE(e)`.
fn visit_done(bm: &BMesh, uid: &LoopUid, e: EdgeId) -> bool {
    uid.of_edge(bm, e).is_some_and(|u| u != INVALID_UID)
}

/// `edbm_ripsel_edge_mark_step`: marks and returns the next walkable edge of `v`.
fn edge_mark_step(bm: &BMesh, uid: &mut LoopUid, v: VertId, id: i32) -> Option<EdgeId> {
    for e in bm.vert_edges(v) {
        if visit_possible(bm, e) && !visit_done(bm, uid, e) {
            let (la, lb) = edge_loop_pair(bm, e).expect("manifold");
            uid.set(la, id);
            uid.set(lb, id);
            return Some(e);
        }
    }
    None
}

/// `edbm_ripsel_looptag_helper`: numbers the loops along each contiguous run of selected
/// manifold edges (descending towards one end) and returns one loop pair per run (the
/// highest uid end).
fn ripsel_looptag_helper(bm: &BMesh) -> (LoopUid, Vec<(LoopId, LoopId)>) {
    let totedge = bm.edges_num() as i32;
    let mut uid = LoopUid(vec![INVALID_UID; bm.loops.capacity()]);
    let mut id = totedge;
    let mut pairs = Vec::new();
    while let Some(e_first) = bm
        .edge_ids()
        .into_iter()
        .find(|&e| visit_possible(bm, e) && !visit_done(bm, &uid, e))
    {
        let mut v_step = bm.edge(e_first).v[0];
        let mut e_step = None;
        let uid_start = id;
        while let Some(e) = edge_mark_step(bm, &mut uid, v_step, id) {
            e_step = Some(e);
            v_step = bm.edge_other_vert(e, v_step);
            id += 1;
        }
        // This edge's loops have the highest uids: walk down from it later.
        let e_last = e_step.expect("the first edge is walkable");
        let uid_end = id - 1;
        id = uid_start - 1;
        v_step = bm.edge(e_first).v[0];
        while let Some(e) = edge_mark_step(bm, &mut uid, v_step, id) {
            v_step = bm.edge_other_vert(e, v_step);
            id -= 1;
        }
        // Stride far enough to never overlap a range.
        id = uid_end + totedge;
        pairs.push(edge_loop_pair(bm, e_last).expect("manifold"));
    }
    (uid, pairs)
}

/// `edbm_ripsel_edge_uid_step`.
fn edge_uid_step(bm: &BMesh, uid: &LoopUid, e_orig: EdgeId, v_prev: &mut VertId) -> Option<EdgeId> {
    let v = bm.edge_other_vert(e_orig, *v_prev);
    let uid_cmp = uid.of_edge(bm, e_orig)? - 1;
    for e in bm.vert_edges(v) {
        if uid.of_edge(bm, e) == Some(uid_cmp) {
            *v_prev = v;
            return Some(e);
        }
    }
    None
}

/// `edbm_ripsel_edloop_pair_start_vert`.
fn edloop_pair_start_vert(bm: &BMesh, uid: &LoopUid, e: EdgeId) -> VertId {
    let [v1, v2] = bm.edge(e).v;
    let mut v_test = v1;
    if edge_uid_step(bm, uid, e, &mut v_test).is_some() {
        v1
    } else {
        v2
    }
}

/// `edbm_ripsel_deselect_helper`: per run, deselects the side facing away from the mouse.
fn ripsel_deselect_helper(bm: &mut BMesh, uid: &LoopUid, pairs: &[(LoopId, LoopId)], rv: &RipView) {
    for &(la, lb) in pairs {
        let score = |bm: &BMesh, l: LoopId| {
            let mut s = 0.0f32;
            let mut e = Some(bm.lp(l).e);
            let mut v_prev = edloop_pair_start_vert(bm, uid, bm.lp(l).e);
            while let Some(ee) = e {
                s += edge_side_measure(bm, ee, bm.edge(ee).l.expect("loop"), rv);
                e = edge_uid_step(bm, uid, ee, &mut v_prev);
            }
            s
        };
        let score_a = score(bm, la);
        let score_b = score(bm, lb);
        let start = if score_a > score_b {
            bm.lp(la).e
        } else {
            bm.lp(lb).e
        };
        let mut v_prev = edloop_pair_start_vert(bm, uid, start);
        let mut e = Some(start);
        while let Some(ee) = e {
            bm.edge_select_set(ee, false);
            e = edge_uid_step(bm, uid, ee, &mut v_prev);
        }
    }
}

/// `BM_mesh_select_mode_clean_ex(bm, SCE_SELECT_EDGE)`: the vertex selection is rebuilt
/// from the selected edges.
fn select_mode_clean_edge(bm: &mut BMesh) {
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::SELECT);
    }
    let sel: Vec<[VertId; 2]> = bm
        .edges
        .iter()
        .filter(|(_, e)| e.flag.contains(ElemFlag::SELECT))
        .map(|(_, e)| e.v)
        .collect();
    for [a, b] in sel {
        bm.vert_select_set(a, true);
        bm.vert_select_set(b, true);
    }
}

/// `edbm_rip_invoke__edge`: the main edge ripping function.
pub(crate) fn rip_edge(
    bm: &mut BMesh,
    rv: &RipView,
    do_fill: bool,
    normals: &mut ClnorKeep,
) -> Finished {
    let (totvert_orig, totedge_orig) = (bm.verts_num(), bm.edges_num());
    // Runs on the original selection, before the tags are changed.
    let (uid, eloop_pairs) = ripsel_looptag_helper(bm);

    // Expand the edge selection.
    for v in bm.vert_ids() {
        let mut e_best = None;
        let mut i = 0;
        let mut totedge_manifold = 0;
        let mut all_manifold = true;
        for e in bm.vert_edges(v) {
            if !bm.edge_is_wire(e) && !hidden(bm, e) {
                // Check the selection rather than the tag, else feedback.
                if selected(bm, e) {
                    e_best = Some(e);
                    i += 1;
                    // Tag the edge vertices so we know which to rip.
                    let [a, b] = bm.edge(e).v;
                    bm.vert_mut(a).flag.insert(ElemFlag::TAG);
                    bm.vert_mut(b).flag.insert(ElemFlag::TAG);
                }
                totedge_manifold += 1;
            }
            // `BM_vert_other_disk_edge` has no hidden checks.
            if all_manifold && !bm.edge_is_manifold(e) {
                all_manifold = false;
            }
        }
        // A single edge: extend.
        let Some(eb) = e_best else {
            continue;
        };
        if i != 1 || bm.edge(eb).l.is_none() {
            continue;
        }
        if totedge_manifold == 4 || totedge_manifold == 3 || !all_manifold {
            let l_a = bm.edge(eb).l.expect("face edge");
            let l_b = bm.lp(l_a).radial_next;
            // The face to follow: the shortest fan, so the edge does not point away from the
            // mouse with more than 4 edges.
            let mut l = if edge_side_measure(bm, eb, l_a, rv) < edge_side_measure(bm, eb, l_b, rv) {
                l_a
            } else {
                l_b
            };
            l = bm.loop_other_edge_loop(l, v);
            // The edge must be manifold, else a fan that does not budge would get a
            // duplicate edge.
            if bm.edge_is_manifold(bm.lp(l).e) {
                l = bm.lp(l).radial_next;
                if totedge_manifold != 3 {
                    l = bm.loop_other_edge_loop(l, v);
                }
                let le = bm.lp(l).e;
                set_tag(bm, le);
            }
        } else if let Some(e) = vert_other_disk_edge(bm, v, eb) {
            set_tag(bm, e);
        }
    }

    // Keep directly before the split.
    let fill_pairs = if do_fill {
        tagged_loop_pairs_to_fill(bm)
    } else {
        None
    };
    mesh_edgesplit(bm, true, true, true);
    // The operator output is ignored: the contiguous loop pairs were built already.
    ripsel_deselect_helper(bm, &uid, &eloop_pairs, rv);
    // Deselect loose verts.
    select_mode_clean_edge(bm);
    if let Some(p) = fill_pairs {
        tagged_loop_pairs_do_fill_faces(bm, &p, normals);
    }
    if totvert_orig == bm.verts_num() && totedge_orig == bm.edges_num() {
        return false;
    }
    bm.select_history_validate();
    true
}
