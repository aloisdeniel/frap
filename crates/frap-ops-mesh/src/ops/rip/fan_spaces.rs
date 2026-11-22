// SPDX-License-Identifier: GPL-3.0-or-later
//! The smooth fans and corner normal spaces `BM_custom_loop_normals_from_vector_layer`
//! encodes in, from the face normals **as they are** (`bm_mesh_loops_custom_normals_set` runs
//! with no face normal array, so it reads `f->no` without recomputing it): a face made by the
//! rip fill copies the normal of its example face (`BM_elem_attrs_copy`), while recomputing it
//! would give the zero normal of a degenerate face (the ripped copies coincide).
//! [`frap_bmesh::clnor::rebuild`] recomputes every face normal first, so the rip computes its
//! spaces here.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
//! (`bm_mesh_edges_sharp_tag` with no split angle, `BM_loop_check_cyclic_smooth_fan`,
//! `bm_mesh_loops_calc_normals_for_loop` and `bm_mesh_loops_calc_normals__single_threaded`
//! with a space array and the custom normal layer), Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::{BMesh, EdgeId, ElemFlag, LoopId};
use frap_geom::normals::{
    CornerNormalSpace, V3, corner_space_custom_data_to_normal, dot, normalize_v3, safe_acos_approx,
    sub,
};

/// One smooth fan (`MLoopNorSpace`).
pub(crate) struct Fan {
    pub space: CornerNormalSpace,
    /// Its loops in the space's list order (`BLI_linklist_prepend`: last visited first).
    pub loops: Vec<LoopId>,
    /// `MLNOR_SPACE_IS_SINGLE`.
    pub single: bool,
}

/// The fans of the BMesh, and the current loop normals (`cur_lnors`: the custom normals
/// decoded, each mixed fan's values averaged) by loop slot.
pub(crate) struct Fans {
    pub fans: Vec<Fan>,
    pub cur_lnors: Vec<V3>,
}

fn etag(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::TAG)
}
fn ltag(bm: &BMesh, l: LoopId) -> bool {
    bm.lp(l).flag.contains(ElemFlag::TAG)
}

/// `bm_edge_tag_from_smooth` with no split angle.
fn edge_tag_from_smooth(bm: &mut BMesh, e: EdgeId) {
    let Some(l_a) = bm.edge(e).l else {
        return;
    };
    let l_b = bm.lp(l_a).radial_next;
    let smooth = bm.lp(l_b).radial_next == l_a
        && bm.lp(l_a).v != bm.lp(l_b).v
        && bm.edge(e).flag.contains(ElemFlag::SMOOTH)
        && bm.face(bm.lp(l_a).f).flag.contains(ElemFlag::SMOOTH)
        && bm.face(bm.lp(l_b).f).flag.contains(ElemFlag::SMOOTH);
    bm.edge_mut(e).flag.set(ElemFlag::TAG, smooth);
}

/// `BM_loop_check_cyclic_smooth_fan`.
fn check_cyclic_smooth_fan(bm: &mut BMesh, l_curr: LoopId) -> bool {
    let mut pivot = l_curr;
    let mut e_next = bm.lp(l_curr).e;
    bm.lp_mut(pivot).flag.insert(ElemFlag::TAG);
    loop {
        let Some(next) = bm.vert_step_fan_loop(pivot, &mut e_next) else {
            return false;
        };
        if !etag(bm, e_next) {
            return false;
        }
        if ltag(bm, next) {
            return next == l_curr;
        }
        bm.lp_mut(next).flag.insert(ElemFlag::TAG);
        pivot = next;
    }
}

fn clnor(bm: &BMesh, l: LoopId) -> [i16; 2] {
    frap_bmesh::clnor::get(bm, l)
}

/// `bm_mesh_loops_calc_normals_for_loop` (space array and custom normals, no split angle).
fn calc_for_loop(bm: &mut BMesh, out: &mut Fans, l_curr: LoopId) {
    let lc = bm.lp(l_curr).clone();
    if etag(bm, lc.e) && (ltag(bm, l_curr) || !check_cyclic_smooth_fan(bm, l_curr)) {
        return;
    }
    let co = |bm: &BMesh, l: LoopId| bm.vert(bm.lp(l).v).co;
    let prev_e = bm.lp(lc.prev).e;
    if !etag(bm, lc.e) && !etag(bm, prev_e) {
        // A single corner between two sharp edges.
        let no = bm.face(lc.f).no;
        let pivot = bm.vert(lc.v).co;
        let vec_curr = normalize_v3(sub(co(bm, lc.next), pivot)).0;
        let vec_prev = normalize_v3(sub(co(bm, lc.prev), pivot)).0;
        let space = frap_bmesh::clnor::fan_space_define(no, vec_curr, vec_prev, &[]);
        out.cur_lnors[l_curr.slot()] =
            corner_space_custom_data_to_normal(&space, clnor(bm, l_curr));
        out.fans.push(Fan {
            space,
            loops: vec![l_curr],
            single: true,
        });
        return;
    }
    let v_pivot = lc.v;
    let co_pivot = bm.vert(v_pivot).co;
    let e_org = lc.e;
    let mut lnor = [0.0f32; 3];
    let mut clnors_avg = [0i32; 2];
    let mut clnor_ref: Option<[i16; 2]> = None;
    let mut clnors_count = 0i32;
    let mut clnors_invalid = false;
    let mut fan: Vec<LoopId> = Vec::new();
    let mut edge_vectors: Vec<V3> = Vec::new();
    let mut pivot = l_curr;
    let mut e_next = e_org;
    let vec_org = normalize_v3(sub(co(bm, lc.next), co_pivot)).0;
    let mut vec_curr = vec_org;
    edge_vectors.push(vec_org);
    let mut vec_next;
    loop {
        frap_math::budget::check();
        let next = bm.vert_step_fan_loop(pivot, &mut e_next);
        if next.is_none() {
            let lp = bm.lp(pivot);
            e_next = if lp.e == e_next {
                bm.lp(lp.prev).e
            } else {
                lp.e
            };
        }
        let other = bm.edge_other_vert(e_next, v_pivot);
        vec_next = normalize_v3(sub(bm.vert(other).co, co_pivot)).0;
        let f = bm.lp(pivot).f;
        let fac = safe_acos_approx(dot(vec_next, vec_curr));
        let no = bm.face(f).no;
        lnor = [
            lnor[0] + no[0] * fac,
            lnor[1] + no[1] * fac,
            lnor[2] + no[2] * fac,
        ];
        let c = clnor(bm, pivot);
        match clnor_ref {
            Some(r) => clnors_invalid |= r != c,
            None => clnor_ref = Some(c),
        }
        clnors_avg[0] += i32::from(c[0]);
        clnors_avg[1] += i32::from(c[1]);
        clnors_count += 1;
        fan.push(pivot);
        if e_next != e_org {
            edge_vectors.push(vec_next);
        }
        if !etag(bm, e_next) || e_next == e_org {
            break;
        }
        vec_curr = vec_next;
        pivot = next.expect("a smooth edge is manifold");
    }
    let (mut lnor_n, mut lnor_len) = normalize_v3(lnor);
    if lnor_len == 0.0 {
        lnor_n = out.cur_lnors[pivot.slot()];
        lnor_len = 1.0;
    }
    let space = frap_bmesh::clnor::fan_space_define(lnor_n, vec_org, vec_next, &edge_vectors);
    let r = if clnors_invalid {
        [
            (clnors_avg[0] / clnors_count) as i16,
            (clnors_avg[1] / clnors_count) as i16,
        ]
    } else {
        clnor_ref.unwrap_or([0; 2])
    };
    if lnor_len != 0.0 {
        let n = corner_space_custom_data_to_normal(&space, r);
        for &l in &fan {
            out.cur_lnors[l.slot()] = n;
        }
    }
    fan.reverse();
    out.fans.push(Fan {
        space,
        loops: fan,
        single: false,
    });
    bm.vert_mut(v_pivot).flag.insert(ElemFlag::TAG);
}

/// The fans of `bm` from its current face normals (the custom normal layer must exist).
/// Uses and leaves the edge, loop and vertex `TAG` flags.
pub(crate) fn fans(bm: &mut BMesh) -> Fans {
    let n = bm.loops.capacity();
    let mut out = Fans {
        fans: Vec::new(),
        cur_lnors: vec![[0.0; 3]; n],
    };
    let mut loops = Vec::with_capacity(bm.loops_num());
    for f in bm.face_ids() {
        for l in bm.face_loops(f) {
            loops.push(l);
            bm.lp_mut(l).flag.remove(ElemFlag::TAG);
        }
    }
    for e in bm.edge_ids() {
        edge_tag_from_smooth(bm, e);
    }
    for l in loops {
        calc_for_loop(bm, &mut out, l);
    }
    out
}
