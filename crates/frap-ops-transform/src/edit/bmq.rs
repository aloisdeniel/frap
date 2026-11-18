// SPDX-License-Identifier: GPL-3.0-or-later
//! BMesh queries the edit-mode transforms need that `frap-bmesh` does not provide (WP 2.12
//! owns them here rather than in `frap-bmesh`, §22.1). Ported from
//! `blender/source/blender/bmesh/intern/{bmesh_marking.cc, bmesh_query.cc,
//! bmesh_polygon.cc}` (Blender 5.2.4, GPL-2.0-or-later): `BM_editselection_*`,
//! `BM_select_history_active_get`, `BM_mesh_active_face_get`, `BM_vert_calc_normal_ex`,
//! `BM_vert_calc_shell_factor(_ex)`, `BM_face_calc_tangent_auto`,
//! `BM_face_calc_tangent_pair_auto`, `BM_vert_tri_calc_tangent_from_edge`,
//! `BM_mesh_calc_edge_groups`, `BM_mesh_calc_face_groups` and `BM_loop_calc_face_direction`.

use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use glam::Vec3;

use super::mathx::{
    FLT_EPSILON, add_safe_acos, angle_v3v3v3, axis_sort_v3, cross, dot, is_zero, len_sq, mid,
    normalize, normalized, project,
};

/// The position of `v`.
#[inline]
pub fn co(bm: &BMesh, v: VertId) -> Vec3 {
    Vec3::from_array(bm.vert(v).co)
}

/// The normal of `v`.
#[inline]
pub fn vno(bm: &BMesh, v: VertId) -> Vec3 {
    Vec3::from_array(bm.vert(v).no)
}

/// The normal of `f`.
#[inline]
pub fn fno(bm: &BMesh, f: FaceId) -> Vec3 {
    Vec3::from_array(bm.face(f).no)
}

/// Whether `e` carries `flag`.
#[inline]
pub fn has(bm: &BMesh, e: impl Into<BMElem>, flag: ElemFlag) -> bool {
    bm.elem_flag(e.into()).contains(flag)
}

/// `BM_face_calc_center_median`.
pub fn face_center_median(bm: &BMesh, f: FaceId) -> Vec3 {
    Vec3::from_array(bm.face_calc_center_median(f))
}

/// `BM_editselection_center`.
pub fn editselection_center(bm: &BMesh, ese: BMElem) -> Vec3 {
    match ese {
        BMElem::Vert(v) => co(bm, v),
        BMElem::Edge(e) => {
            let ed = bm.edge(e);
            mid(co(bm, ed.v[0]), co(bm, ed.v[1]))
        }
        BMElem::Face(f) => face_center_median(bm, f),
    }
}

/// `BM_editselection_normal`.
pub fn editselection_normal(bm: &BMesh, ese: BMElem) -> Vec3 {
    match ese {
        BMElem::Vert(v) => vno(bm, v),
        BMElem::Edge(e) => {
            let ed = bm.edge(e);
            let n = vno(bm, ed.v[0]) + vno(bm, ed.v[1]);
            let plane = co(bm, ed.v[1]) - co(bm, ed.v[0]);
            let vec = cross(n, plane);
            normalized(cross(plane, vec))
        }
        BMElem::Face(f) => fno(bm, f),
    }
}

/// `BM_editselection_plane`; `prev` is `ese->prev` (the previous selection-history entry, or
/// `None` for a stand-alone selection).
pub fn editselection_plane(bm: &BMesh, ese: BMElem, prev: Option<BMElem>) -> Vec3 {
    match ese {
        BMElem::Vert(v) => {
            let no = vno(bm, v);
            let plane = if let Some(p) = prev {
                editselection_center(bm, p) - co(bm, v)
            } else {
                let mut vec = Vec3::ZERO;
                if no.x < 0.5 {
                    vec.x = 1.0;
                } else if no.y < 0.5 {
                    vec.y = 1.0;
                } else {
                    vec.z = 1.0;
                }
                cross(no, vec)
            };
            normalized(plane)
        }
        BMElem::Edge(e) => {
            let ed = bm.edge(e);
            let plane = if bm.edge_is_boundary(e) {
                let l = ed.l.expect("boundary edge has a loop");
                let ll = bm.lp(l);
                co(bm, ll.v) - co(bm, bm.lp(ll.next).v)
            } else {
                let (v1, v2) = (co(bm, ed.v[0]), co(bm, ed.v[1]));
                if v2.y > v1.y { v2 - v1 } else { v1 - v2 }
            };
            normalized(plane)
        }
        BMElem::Face(f) => face_calc_tangent_auto(bm, f),
    }
}

/// `BM_mesh_active_face_get(bm, false, true)`: the active face when it is selected.
pub fn active_face_selected(bm: &BMesh) -> Option<FaceId> {
    bm.act_face
        .filter(|f| bm.faces.contains(f.0) && has(bm, *f, ElemFlag::SELECT))
}

/// `BM_select_history_active_get`.
pub fn select_history_active_get(bm: &BMesh) -> Option<BMElem> {
    let efa = active_face_selected(bm);
    match bm.select_history.last() {
        Some(BMElem::Face(f)) => Some(BMElem::Face(efa.unwrap_or(*f))),
        Some(e) => Some(*e),
        None => efa.map(BMElem::Face),
    }
}

/// `BM_vert_calc_normal_ex(v, BM_ELEM_SELECT)`: the angle-weighted normal of the selected
/// faces around `v` (`None` without one).
pub fn vert_calc_normal_selected(bm: &BMesh, v: VertId) -> Option<Vec3> {
    let mut no = Vec3::ZERO;
    let mut n = 0;
    let first = bm.vert(v).e?;
    let mut e = first;
    loop {
        if let Some(l_first) = bm.edge(e).l {
            let mut l = l_first;
            loop {
                let ll = bm.lp(l);
                if ll.v == v && has(bm, ll.f, ElemFlag::SELECT) {
                    let vec1 = normalized(co(bm, v) - co(bm, bm.lp(ll.prev).v));
                    let vec2 = normalized(co(bm, bm.lp(ll.next).v) - co(bm, v));
                    no = add_safe_acos(no, fno(bm, ll.f), -dot(vec1, vec2));
                    n += 1;
                }
                l = ll.radial_next;
                if l == l_first {
                    break;
                }
            }
        }
        e = bm.disk_edge_next(e, v);
        if e == first {
            break;
        }
    }
    (n > 0).then(|| normalized(no))
}

/// `BM_loop_calc_face_angle`.
pub fn loop_calc_face_angle(bm: &BMesh, l: LoopId) -> f32 {
    let ll = bm.lp(l);
    angle_v3v3v3(
        co(bm, bm.lp(ll.prev).v),
        co(bm, ll.v),
        co(bm, bm.lp(ll.next).v),
    )
}

/// `shell_v3v3_normalized_to_dist`.
fn shell_dist(a: Vec3, b: Vec3) -> f32 {
    let c = dot(a, b).abs();
    if c < 1.0e-8 { 1.0 } else { 1.0 / c }
}

/// `BM_vert_calc_shell_factor`.
pub fn vert_calc_shell_factor(bm: &BMesh, v: VertId) -> f32 {
    let (mut shell, mut angle) = (0.0f32, 0.0f32);
    for l in bm.vert_loops(v) {
        let a = loop_calc_face_angle(bm, l);
        shell += shell_dist(vno(bm, v), fno(bm, bm.lp(l).f)) * a;
        angle += a;
    }
    if angle != 0.0 { shell / angle } else { 1.0 }
}

/// `BM_vert_calc_shell_factor_ex(v, no, BM_ELEM_SELECT)`.
pub fn vert_calc_shell_factor_selected(bm: &BMesh, v: VertId, no: Vec3) -> f32 {
    let (mut shell, mut angle) = (0.0f32, 0.0f32);
    let (mut tot_sel, mut tot) = (0, 0);
    for l in bm.vert_loops(v) {
        let f = bm.lp(l).f;
        if has(bm, f, ElemFlag::SELECT) {
            let a = loop_calc_face_angle(bm, l);
            shell += shell_dist(no, fno(bm, f)) * a;
            angle += a;
            tot_sel += 1;
        }
        tot += 1;
    }
    if angle != 0.0 {
        return shell / angle;
    }
    if tot != 0 && tot_sel == 0 {
        return vert_calc_shell_factor(bm, v);
    }
    1.0
}

/// `BM_loop_calc_face_direction`.
pub fn loop_calc_face_direction(bm: &BMesh, l: LoopId) -> Vec3 {
    let ll = bm.lp(l);
    let c = co(bm, ll.v);
    let v_prev = normalized(c - co(bm, bm.lp(ll.prev).v));
    let v_next = normalized(co(bm, bm.lp(ll.next).v) - c);
    normalized(v_prev + v_next)
}

/// `BM_vert_is_edge_pair`.
pub fn vert_is_edge_pair(bm: &BMesh, v: VertId) -> bool {
    vert_edge_pair(bm, v).is_some()
}

/// `BM_vert_edge_pair`.
pub fn vert_edge_pair(bm: &BMesh, v: VertId) -> Option<(EdgeId, EdgeId)> {
    let e = bm.vert(v).e?;
    let o = bm.disk_edge_next(e, v);
    (o != e && bm.disk_edge_next(o, v) == e).then_some((e, o))
}

/// `BM_vert_is_boundary`.
pub fn vert_is_boundary(bm: &BMesh, v: VertId) -> bool {
    bm.vert(v).e.is_some() && bm.vert_edges(v).iter().any(|e| bm.edge_is_boundary(*e))
}

/// `BM_edge_calc_length_squared`.
pub fn edge_len_sq(bm: &BMesh, e: EdgeId) -> f32 {
    let ed = bm.edge(e);
    len_sq(co(bm, ed.v[0]) - co(bm, ed.v[1]))
}

/// `BM_edge_ordered_verts`: the vertices in the winding of the edge's first loop.
pub fn edge_ordered_verts(bm: &BMesh, e: EdgeId) -> (VertId, VertId) {
    let l = bm.edge(e).l.expect("edge with a face");
    let ll = bm.lp(l);
    (ll.v, bm.lp(ll.next).v)
}

fn vert_tri_find_unique_edge(c: [Vec3; 3]) -> usize {
    let mut difs = [0.0f32; 3];
    let (mut i_prev, mut i_curr, mut i_next) = (1usize, 2usize, 0usize);
    while i_next < 3 {
        let co_ = c[i_curr];
        let other = [c[i_prev], c[i_next]];
        let proj_dir = mid(other[0], other[1]) - co_;
        let p0 = project(other[0], proj_dir);
        let p1 = project(other[1], proj_dir);
        difs[i_next] = len_sq(p0 - p1);
        i_prev = i_curr;
        i_curr = i_next;
        i_next += 1;
    }
    axis_sort_v3(difs)[0]
}

/// `BM_vert_tri_calc_tangent_from_edge`.
pub fn vert_tri_calc_tangent_from_edge(c: [Vec3; 3]) -> Vec3 {
    let i = vert_tri_find_unique_edge(c);
    normalized(c[i] - c[(i + 1) % 3])
}

/// `BM_vert_tri_calc_tangent_pair_from_edge`.
fn vert_tri_calc_tangent_pair_from_edge(c: [Vec3; 3]) -> (Vec3, Vec3) {
    let index = vert_tri_find_unique_edge(c);
    let index_next = (index + 1) % 3;
    let index_prev = (index_next + 1) % 3;
    let a = normalized(c[index] - c[index_next]);
    let vec_prev = c[index_prev] - c[index];
    let vec_next = c[index_next] - c[index_prev];
    let tmp_prev = cross(a, vec_prev);
    let tmp_next = cross(a, vec_next);
    let b = normalized(if len_sq(tmp_next) > len_sq(tmp_prev) {
        vec_next
    } else {
        vec_prev
    });
    (a, b)
}

fn face_cos(bm: &BMesh, f: FaceId) -> Vec<Vec3> {
    bm.face_verts(f).into_iter().map(|v| co(bm, v)).collect()
}

/// `BM_face_find_longest_loop`.
pub fn face_find_longest_loop(bm: &BMesh, f: FaceId) -> LoopId {
    let loops = bm.face_loops(f);
    let mut best = loops[0];
    let mut max = 0.0f32;
    for l in loops {
        let ll = bm.lp(l);
        let d = len_sq(co(bm, ll.v) - co(bm, bm.lp(ll.next).v));
        if d >= max {
            best = l;
            max = d;
        }
    }
    best
}

fn quad_edge_pair(c: &[Vec3]) -> (Vec3, Vec3) {
    let a = (c[3] - c[2]) + (c[0] - c[1]);
    let b = (c[0] - c[3]) + (c[1] - c[2]);
    (a, b)
}

/// `BM_face_calc_tangent_auto`.
pub fn face_calc_tangent_auto(bm: &BMesh, f: FaceId) -> Vec3 {
    let c = face_cos(bm, f);
    match c.len() {
        3 => vert_tri_calc_tangent_from_edge([c[0], c[1], c[2]]),
        4 => {
            let (mut t, vec) = quad_edge_pair(&c);
            if len_sq(t) < len_sq(vec) {
                t = vec;
            }
            normalized(t)
        }
        _ => {
            let l = face_find_longest_loop(bm, f);
            let ll = bm.lp(l);
            normalized(co(bm, ll.v) - co(bm, bm.lp(ll.next).v))
        }
    }
}

/// `BM_face_calc_tangent_pair_auto`.
pub fn face_calc_tangent_pair_auto(bm: &BMesh, f: FaceId) -> (Vec3, Vec3) {
    let c = face_cos(bm, f);
    match c.len() {
        3 => vert_tri_calc_tangent_pair_from_edge([c[0], c[1], c[2]]),
        4 => {
            let (a, b) = quad_edge_pair(&c);
            let (na, la) = normalize(a);
            let (nb, lb) = normalize(b);
            if la < lb { (nb, na) } else { (na, nb) }
        }
        _ => {
            let l_long = face_find_longest_loop(bm, f);
            let ll = bm.lp(l_long);
            let lc = co(bm, ll.v);
            let next = bm.lp(ll.next);
            let a = normalized(lc - co(bm, next.v));
            let vec_prev = co(bm, bm.lp(ll.prev).v) - lc;
            let vec_next = co(bm, next.v) - co(bm, bm.lp(next.next).v);
            let tmp_prev = cross(a, vec_prev);
            let tmp_next = cross(a, vec_next);
            let b = normalized(if len_sq(tmp_next) > len_sq(tmp_prev) {
                vec_next
            } else {
                vec_prev
            });
            (a, b)
        }
    }
}

/// `BM_mesh_calc_edge_groups(bm, .., nullptr, nullptr, BM_ELEM_SELECT)`: groups of selected
/// edges connected through their vertices, in Blender's discovery order.
pub fn edge_groups_selected(bm: &BMesh) -> Vec<Vec<EdgeId>> {
    let edges = bm.edge_ids();
    let mut tag = vec![false; bm.edges.slots.len()];
    let mut tot = 0usize;
    for e in &edges {
        if has(bm, *e, ElemFlag::SELECT) {
            tot += 1;
        } else {
            tag[e.slot()] = true;
        }
    }
    let mut groups = Vec::new();
    let mut touched = 0usize;
    let mut next = 0usize;
    while touched != tot {
        let mut stack = Vec::new();
        while next < edges.len() {
            let e = edges[next];
            next += 1;
            if !tag[e.slot()] {
                tag[e.slot()] = true;
                stack.push(e);
                break;
            }
        }
        let mut group = Vec::new();
        while let Some(e) = stack.pop() {
            group.push(e);
            touched += 1;
            for v in bm.edge(e).v {
                for o in bm.vert_edges(v) {
                    if !tag[o.slot()] {
                        tag[o.slot()] = true;
                        stack.push(o);
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// `BM_mesh_calc_face_groups(bm, .., nullptr, nullptr, nullptr, BM_ELEM_SELECT, BM_VERT)`.
/// Blender's `BM_VERT` step walks `BM_LOOPS_OF_LOOP`, i.e. the other loops of each corner's
/// **edge**, so the groups are those of faces sharing an edge.
pub fn face_groups_selected(bm: &BMesh) -> Vec<Vec<FaceId>> {
    let faces = bm.face_ids();
    let mut tag = vec![false; bm.faces.slots.len()];
    let mut tot = 0usize;
    for f in &faces {
        if has(bm, *f, ElemFlag::SELECT) {
            tot += 1;
        } else {
            tag[f.slot()] = true;
        }
    }
    let mut groups = Vec::new();
    let mut touched = 0usize;
    let mut next = 0usize;
    while touched != tot {
        let mut stack = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !tag[f.slot()] {
                tag[f.slot()] = true;
                stack.push(f);
                break;
            }
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            group.push(f);
            touched += 1;
            for l in bm.face_loops(f) {
                let mut o = bm.lp(l).radial_next;
                while o != l {
                    let of = bm.lp(o).f;
                    if !tag[of.slot()] {
                        tag[of.slot()] = true;
                        stack.push(of);
                    }
                    o = bm.lp(o).radial_next;
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// The live vertices in iteration order and each slot's dense index (`BM_mesh_elem_index_ensure`).
pub fn vert_index(bm: &BMesh) -> (Vec<VertId>, Vec<i32>) {
    let verts = bm.vert_ids();
    let mut idx = vec![-1i32; bm.verts.slots.len()];
    for (i, v) in verts.iter().enumerate() {
        idx[v.slot()] = i as i32;
    }
    (verts, idx)
}

/// `BM_loop_find_prev_nodouble`.
pub fn loop_find_prev_nodouble(bm: &BMesh, l: LoopId, l_stop: LoopId, eps_sq: f32) -> Option<LoopId> {
    let c = co(bm, bm.lp(l).v);
    let mut s = bm.lp(l).prev;
    while len_sq(c - co(bm, bm.lp(s).v)) < eps_sq {
        s = bm.lp(s).prev;
        if s == l_stop {
            return None;
        }
    }
    Some(s)
}

/// `BM_loop_find_next_nodouble`.
pub fn loop_find_next_nodouble(bm: &BMesh, l: LoopId, l_stop: LoopId, eps_sq: f32) -> Option<LoopId> {
    let c = co(bm, bm.lp(l).v);
    let mut s = bm.lp(l).next;
    while len_sq(c - co(bm, bm.lp(s).v)) < eps_sq {
        s = bm.lp(s).next;
        if s == l_stop {
            return None;
        }
    }
    Some(s)
}

/// Whether a vector is "zero" for Blender's `is_zero_v3`.
pub fn zero(v: Vec3) -> bool {
    is_zero(v)
}

/// `FLT_EPSILON` re-export for callers.
pub const EPS: f32 = FLT_EPSILON;
