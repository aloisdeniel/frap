// SPDX-License-Identifier: GPL-3.0-or-later
//! Edge slide and vertex slide, with the UV correction of `correct_uv`. Ported from Blender
//! 5.2.4 (GPL-2.0-or-later): `transform_mesh_edge_slide_data_create`,
//! `transform_mesh_vert_slide_data_create`, `isect_face_dst`, `bm_loop_calc_opposite_co`,
//! `mesh_vert_is_inner` and the custom-data correction (`mesh_customdatacorrect_*`,
//! `transform_convert_mesh.cc`); `createEdgeSlideVerts`, `calcEdgeSlide_mval_range`,
//! `edge_slide_apply_elem`, `doEdgeSlide`, `interp_line_v3_v3v3v3`
//! (`transform_mode_edge_slide.cc`); `VertSlideData::update_active_vert/edges`,
//! `vert_slide_apply_elem`, `doVertSlide` (`transform_mode_vert_slide.cc`);
//! `BMBVH_EdgeVisible` (`editors/mesh/editmesh_utils.cc`), `isect_ray_tri_watertight_v3`
//! (`blenlib/intern/math_geom.cc`) and the loop-group merging of `bmesh_interp.cc`.
//!
//! A non-interactive slide still depends on the 3D View, as in Blender: the mouse is at the
//! region's corner (0, 0), the loop whose slide segment projects closest to it (among those
//! not hidden behind the mesh) sets the screen direction every other loop is flipped to match,
//! and its vertex is the reference of `use_even`. frap uses the factory Layout viewport of a
//! background Blender (`VIEW_WINMAT`, `VIEW_WINX`/`VIEW_WINY`, the virtual view matrix).
//! Vertex slide without `direction` slides along the edge closest to world +X.

use frap_bmesh::{BMesh, CustomData, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use frap_mesh::AttrData;
use frap_op::prelude::*;
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::sync::Arc;

use super::bmq::{
    co, fno, has, loop_calc_face_direction, loop_find_next_nodouble, loop_find_prev_nodouble,
    vert_is_boundary, vert_is_edge_pair, vno,
};
use super::mathx::{
    FLT_EPSILON, angle_v3v3v3, cross, dist_signed_squared_to_corner, dot, interp, is_zero,
    isect_line_line_epsilon, isect_line_plane, len, len_sq, line_point_factor, madd, mid,
    normalize, normalized, project_plane_normalized,
};
use super::trans::{Mode, Tc, TransInfo, write_back};

/// Edge/vertex slide options.
#[derive(Clone, Debug, Default)]
pub struct SlideParams {
    /// `use_even`
    pub use_even: bool,
    /// `flipped`
    pub flipped: bool,
    /// `use_clamp`
    pub use_clamp: bool,
    /// `single_side`
    pub single_side: bool,
    /// `correct_uv` when set.
    pub correct_uv: Option<bool>,
    /// `direction` (vertex slide) when set.
    pub direction: Option<Vec3>,
}

/// The factory Layout viewport's `RegionView3D.winmat` in a background Blender 5.2.4 (rows).
const VIEW_WINMAT_ROWS: [[f32; 4]; 4] = [
    [1.388_888_8, 0.0, 0.0, 0.0],
    [0.0, 2.291_521, 0.0, 0.0],
    [0.0, 0.0, -1.000_02, -0.020_000_2],
    [0.0, 0.0, -1.0, 0.0],
];
/// The factory Layout viewport's region width.
const VIEW_WINX: f32 = 1574.0;
/// The factory Layout viewport's region height.
const VIEW_WINY: f32 = 954.0;
/// `View3D.clip_start` of the factory viewport.
const VIEW_CLIP_START: f32 = 0.01;

/// The view projection of the virtual 3D View.
struct View {
    persmat: Mat4,
    persinv: Mat4,
    viewinv: Mat4,
}

impl View {
    fn new(view_matrix: &Mat4) -> View {
        let winmat = Mat4::from_cols_array_2d(&VIEW_WINMAT_ROWS).transpose();
        let persmat = winmat * *view_matrix;
        View {
            persmat,
            persinv: persmat.inverse(),
            viewinv: view_matrix.inverse(),
        }
    }
    /// `ED_view3d_ob_project_mat_get` scaled by the half window (`update_proj_mat`).
    fn proj_mat(&self, obmat: &Mat4) -> Mat4 {
        let mut m = self.persmat * *obmat;
        for i in 0..4 {
            let c = m.col_mut(i);
            c.x *= VIEW_WINX / 2.0;
            c.y *= VIEW_WINY / 2.0;
        }
        m
    }
    /// The world-space origin of the ray through the window center
    /// (`ED_view3d_win_to_segment_clipped`'s start).
    fn center_ray_origin(&self) -> Vec3 {
        let mut out = Vec3::new(0.0, 0.0, -0.5);
        let p = self.persinv * Vec4::new(out.x, out.y, out.z, 1.0);
        out = p.truncate() / p.w;
        let cam = self.viewinv.w_axis.truncate();
        let dir = normalized(out - cam);
        madd(cam, dir, VIEW_CLIP_START)
    }
}

fn project(m: &Mat4, co: Vec3) -> Vec2 {
    let p = *m * Vec4::new(co.x, co.y, co.z, 1.0);
    Vec2::new(p.x / p.w, p.y / p.w) + Vec2::new(VIEW_WINX / 2.0, VIEW_WINY / 2.0)
}

/// `dist_squared_to_line_segment_v2`.
fn dist_sq_to_segment(p: Vec2, l1: Vec2, l2: Vec2) -> f32 {
    let u = l2 - l1;
    let h = p - l1;
    let d = u.x * u.x + u.y * u.y;
    let lambda = if d > 0.0 {
        (u.x * h.x + u.y * h.y) / d
    } else {
        0.0
    };
    let c = if lambda <= 0.0 {
        l1
    } else if lambda >= 1.0 {
        l2
    } else {
        Vec2::new(l1.x + u.x * lambda, l1.y + u.y * lambda)
    };
    let d = p - c;
    d.x * d.x + d.y * d.y
}

// ---------------------------------------------------------------- visibility (BMBVH)

fn axis_dominant_single(v: Vec3) -> usize {
    let (x, y, z) = (v.x.abs(), v.y.abs(), v.z.abs());
    if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    }
}

/// `isect_ray_tri_watertight_v3` (with its precalculation): the hit distance.
fn isect_ray_tri_watertight(origin: Vec3, dir: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
    let kz = axis_dominant_single(dir);
    let mut kx = if kz != 2 { kz + 1 } else { 0 };
    let mut ky = if kx != 2 { kx + 1 } else { 0 };
    if dir[kz] < 0.0 {
        std::mem::swap(&mut kx, &mut ky);
    }
    let inv_dir_z = 1.0 / dir[kz];
    let (sx, sy, sz) = (dir[kx] * inv_dir_z, dir[ky] * inv_dir_z, inv_dir_z);
    let a = v0 - origin;
    let b = v1 - origin;
    let c = v2 - origin;
    let ax = a[kx] - sx * a[kz];
    let ay = a[ky] - sy * a[kz];
    let bx = b[kx] - sx * b[kz];
    let by = b[ky] - sy * b[kz];
    let cx = c[kx] - sx * c[kz];
    let cy = c[ky] - sy * c[kz];
    let u = cx * by - cy * bx;
    let v = ax * cy - ay * cx;
    let w = bx * ay - by * ax;
    if (u < 0.0 || v < 0.0 || w < 0.0) && (u > 0.0 || v > 0.0 || w > 0.0) {
        return None;
    }
    let det = u + v + w;
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let t = (u * a[kz] + v * b[kz] + w * c[kz]) * sz;
    let sign_t = if det.is_sign_negative() { -t } else { t };
    if sign_t < 0.0 {
        return None;
    }
    Some(t * (1.0 / det))
}

/// The visible (not hidden) faces as fan triangles, the `BMBVHTree` of `BMBVH_RESPECT_HIDDEN`.
struct Tris {
    tris: Vec<(FaceId, [Vec3; 3])>,
}

impl Tris {
    fn new(bm: &BMesh) -> Tris {
        let mut tris = Vec::new();
        for f in bm.face_ids() {
            if has(bm, f, ElemFlag::HIDDEN) {
                continue;
            }
            let c: Vec<Vec3> = bm.face_verts(f).into_iter().map(|v| co(bm, v)).collect();
            for i in 1..c.len().saturating_sub(1) {
                tris.push((f, [c[0], c[i], c[i + 1]]));
            }
        }
        Tris { tris }
    }
    /// `BKE_bmbvh_ray_cast`: the nearest face hit closer than `dist`.
    fn ray_cast(&self, origin: Vec3, dir: Vec3, dist: f32) -> Option<FaceId> {
        let mut best = dist;
        let mut hit = None;
        for (f, t) in &self.tris {
            if let Some(d) = isect_ray_tri_watertight(origin, dir, t[0], t[1], t[2])
                && d < best
            {
                best = d;
                hit = Some(*f);
            }
        }
        hit
    }
}

/// `BMBVH_EdgeVisible` with the ray origin in object space.
fn edge_visible(bm: &BMesh, tris: &Tris, e: EdgeId, origin: Vec3) -> bool {
    let ed = bm.edge(e);
    let (v1, v2) = (co(bm, ed.v[0]), co(bm, ed.v[1]));
    let co2 = mid(v1, v2);
    let scale = |c: Vec3| (c - co2) * 0.99 + co2;
    let (co1, co3) = (scale(v1), scale(v2));
    let ray = |c: Vec3| {
        let d = origin - c;
        let dist = len(d);
        let (o, _) = super::mathx::normalize_length(d, 0.01);
        (c + o, normalized(d), dist)
    };
    let cast = |c: Vec3| {
        let (c, d, dist) = ray(c);
        tris.ray_cast(c, d, dist)
            .filter(|f| !bm.edge_in_face(e, *f))
    };
    let f = cast(co1);
    if f.is_some() && cast(co2).is_none() {
        return true;
    }
    if f.is_some() && cast(co3).is_none() {
        return true;
    }
    f.is_none()
}

// ---------------------------------------------------------------- edge slide data

/// `TransDataEdgeSlideVert`.
#[derive(Clone, Debug)]
struct EdgeSlideVert {
    td: usize,
    dir_side: [Vec3; 2],
    edge_len: f32,
    loop_nr: i32,
}

#[derive(Clone, Copy, Debug, Default)]
struct FData {
    f: Option<FaceId>,
    v_dst: Option<VertId>,
    dst: Vec3,
}

#[derive(Clone, Copy, Debug, Default)]
struct SlideTemp {
    i: i32,
    v: Option<VertId>,
    e: Option<EdgeId>,
    fdata: [FData; 2],
    vert_is_edge_pair: bool,
}

/// `mesh_vert_is_inner`.
fn vert_is_inner(bm: &BMesh, v: VertId) -> bool {
    vert_is_edge_pair(bm, v) && !vert_is_boundary(bm, v)
}

/// `bm_loop_calc_opposite_co`.
fn loop_calc_opposite_co(bm: &BMesh, l_tmp: LoopId, plane_no: Vec3) -> Option<Vec3> {
    let lt = bm.lp(l_tmp);
    let l_first = lt.next;
    let l_last = lt.prev;
    let tco = co(bm, lt.v);
    let mut best = f32::MAX;
    let mut out = None;
    let mut l = l_first;
    loop {
        let a = co(bm, bm.lp(l).v);
        let b = co(bm, bm.lp(bm.lp(l).next).v);
        if let Some(t) = isect_line_plane(a, b, tco, plane_no) {
            let fac = line_point_factor(t, a, b);
            if fac > -FLT_EPSILON && fac < 1.0 + FLT_EPSILON {
                let d = len_sq(tco - t);
                if d < best {
                    best = d;
                    out = Some(t);
                }
            }
        }
        l = bm.lp(l).next;
        if l == l_last {
            break;
        }
    }
    out
}

/// `isect_face_dst`.
fn isect_face_dst(bm: &BMesh, l: LoopId) -> Vec3 {
    let ll = bm.lp(l);
    let f = ll.f;
    let l_next = ll.next;
    if bm.face(f).len == 4 {
        return co(bm, bm.lp(bm.lp(l_next).next).v);
    }
    let plane_no = loop_calc_face_direction(bm, l);
    loop_calc_opposite_co(bm, l, plane_no)
        .unwrap_or_else(|| mid(co(bm, bm.lp(ll.prev).v), co(bm, bm.lp(l_next).v)))
}

impl SlideTemp {
    /// `SlideTempDataMesh::find_best_dir`.
    fn find_best_dir(
        &self,
        bm: &BMesh,
        other: &SlideTemp,
        f_curr: FaceId,
        l_src: LoopId,
        v_dst: VertId,
    ) -> (usize, bool) {
        if Some(f_curr) == other.fdata[0].f || Some(v_dst) == other.fdata[0].v_dst {
            return (0, false);
        }
        if Some(f_curr) == other.fdata[1].f || Some(v_dst) == other.fdata[1].v_dst {
            return (1, false);
        }
        if other.fdata[0].f.is_some() || other.fdata[1].f.is_some() {
            let ls = bm.lp(l_src);
            let l_edge = if bm.lp(ls.next).v == v_dst {
                l_src
            } else {
                ls.prev
            };
            let mut l_other = bm.lp(l_edge).radial_next;
            let mut best = None;
            while bm.lp(l_other).f != bm.lp(l_edge).f {
                let lo = bm.lp(l_other);
                if Some(lo.f) == other.fdata[0].f {
                    best = Some(0);
                    break;
                }
                if Some(lo.f) == other.fdata[1].f {
                    best = Some(1);
                    break;
                }
                let step = if Some(lo.v) == self.v {
                    lo.prev
                } else {
                    lo.next
                };
                l_other = bm.lp(step).radial_next;
            }
            if let Some(b) = best {
                return (b, true);
            }
        }
        if self.fdata[0].f.is_none() || self.fdata[1].f.is_none() {
            return (self.fdata[0].f.is_some() as usize, false);
        }
        let v = self.v.expect("slide vertex");
        let e0 = self.fdata[0].v_dst.and_then(|d| bm.edge_exists(v, d));
        let e1 = self.fdata[1].v_dst.and_then(|d| bm.edge_exists(v, d));
        let b0 = e0.is_some_and(|e| bm.edge_is_boundary(e));
        let b1 = e1.is_some_and(|e| bm.edge_is_boundary(e));
        if b0 && !b1 {
            return (0, true);
        }
        if b1 && !b0 {
            return (1, true);
        }
        let src = co(bm, v);
        let dir_curr = co(bm, v_dst) - src;
        let dir0 = normalized(self.fdata[0].dst - src);
        let dir1 = normalized(self.fdata[1].dst - src);
        ((dot(dir_curr, dir0) < dot(dir_curr, dir1)) as usize, true)
    }
}

/// `transform_mesh_edge_slide_data_create`: the slide vertices (in selected `TransData`
/// order) and the number of loops, or `None` for an invalid selection.
fn edge_slide_data_create(bm: &BMesh, tc: &Tc) -> Option<(Vec<EdgeSlideVert>, i32)> {
    let sel: Vec<usize> = (0..tc.data.len())
        .filter(|i| tc.data[*i].selected)
        .collect();
    for &i in &sel {
        let v = tc.data[i].v?;
        let numsel = bm
            .vert_edges(v)
            .iter()
            .filter(|e| has(bm, **e, ElemFlag::SELECT))
            .count();
        if numsel == 0 || numsel > 2 {
            return None;
        }
    }
    for e in bm.edge_ids() {
        if has(bm, e, ElemFlag::SELECT) && !bm.edge_is_manifold(e) && !bm.edge_is_boundary(e) {
            return None;
        }
    }
    let mut index = vec![-1i32; bm.verts.slots.len()];
    let mut sv: Vec<EdgeSlideVert> = Vec::with_capacity(sel.len());
    for (k, &i) in sel.iter().enumerate() {
        index[tc.data[i].v.expect("vertex data").slot()] = k as i32;
        sv.push(EdgeSlideVert {
            td: i,
            dir_side: [Vec3::ZERO; 2],
            edge_len: 0.0,
            loop_nr: -1,
        });
    }
    let svv = |k: i32| tc.data[sv_td(&sel, k)].v.expect("vertex data");
    fn sv_td(sel: &[usize], k: i32) -> usize {
        sel[k as usize]
    }
    let mut connected = vec![[-1i32; 2]; tc.data.len().max(sel.len())];
    for e in bm.edge_ids() {
        if !has(bm, e, ElemFlag::SELECT) {
            continue;
        }
        let ed = bm.edge(e);
        let (i1, i2) = (index[ed.v[0].slot()], index[ed.v[1].slot()]);
        if i1 == -1 || i2 == -1 {
            continue;
        }
        let s1 = (connected[i1 as usize][0] != -1) as usize;
        let s2 = (connected[i2 as usize][0] != -1) as usize;
        connected[i1 as usize][s1] = i2;
        connected[i2 as usize][s2] = i1;
    }
    let make = |i: i32| -> SlideTemp {
        let mut t = SlideTemp {
            i,
            ..SlideTemp::default()
        };
        if i != -1 {
            let v = svv(i);
            t.v = Some(v);
            t.vert_is_edge_pair = vert_is_inner(bm, v);
        }
        t
    };
    let mut loop_nr = 0;
    for i in 0..sv.len() {
        if sv[i].loop_nr != -1 {
            continue;
        }
        let mut i_curr = i as i32;
        let mut i_prev = connected[i][1];
        while i_prev != -1 && i_prev != i as i32 {
            let c = connected[i_prev as usize];
            let tmp = if c[0] != i_curr { c[0] } else { c[1] };
            i_curr = i_prev;
            i_prev = tmp;
        }
        let ci = connected[i_curr as usize];
        let next_i = if ci[0] != i_prev { ci[0] } else { ci[1] };
        let mut next = make(next_i);
        let mut curr = make(i_curr);
        if curr.i != -1 && next.i != -1 {
            curr.e = bm.edge_exists(curr.v.expect("v"), next.v.expect("v"));
        }
        let mut prev = SlideTemp {
            i: -1,
            ..SlideTemp::default()
        };
        while curr.i != -1 {
            let mut next_next = SlideTemp {
                i: -1,
                ..SlideTemp::default()
            };
            if next.i != -1 {
                let cn = connected[next.i as usize];
                let nn_i = if cn[0] != curr.i { cn[0] } else { cn[1] };
                if nn_i != -1 {
                    next_next = make(nn_i);
                    next.e = bm.edge_exists(next.v.expect("v"), next_next.v.expect("v"));
                } else {
                    next_next.i = -1;
                }
                let tmp = curr;
                let e = curr.e.expect("slide edge");
                let cv = curr.v.expect("v");
                for l in bm.edge_loops(e) {
                    let ll = bm.lp(l);
                    let f_curr = ll.f;
                    let (l1, l2, l_edge_next, v1_dst, v2_dst);
                    if ll.v == cv {
                        l1 = l;
                        l2 = ll.next;
                        l_edge_next = bm.lp(l2).e;
                        v1_dst = bm.lp(bm.lp(l1).prev).v;
                        v2_dst = bm.lp(bm.lp(l2).next).v;
                    } else {
                        l1 = ll.next;
                        l2 = l;
                        l_edge_next = bm.lp(bm.lp(l2).prev).e;
                        v1_dst = bm.lp(bm.lp(l1).next).v;
                        v2_dst = bm.lp(bm.lp(l2).prev).v;
                    }
                    let dst = co(bm, v1_dst);
                    let (best_dir, isect_curr_dirs) =
                        curr.find_best_dir(bm, &tmp, f_curr, l1, v1_dst);
                    if curr.fdata[best_dir].f.is_none() {
                        curr.fdata[best_dir].f = Some(f_curr);
                        if curr.vert_is_edge_pair {
                            curr.fdata[best_dir].dst = isect_face_dst(bm, l1);
                        } else {
                            curr.fdata[best_dir].v_dst = Some(v1_dst);
                            curr.fdata[best_dir].dst = dst;
                        }
                    }
                    next.fdata[best_dir].f = Some(f_curr);
                    if Some(l_edge_next) == next.e || next.vert_is_edge_pair {
                        next.fdata[best_dir].v_dst = None;
                        next.fdata[best_dir].dst = isect_face_dst(bm, l2);
                    } else {
                        next.fdata[best_dir].v_dst = Some(v2_dst);
                        next.fdata[best_dir].dst = co(bm, v2_dst);
                    }
                    if isect_curr_dirs {
                        let curr_orig = tc.data[sv[curr.i as usize].td].iloc;
                        let dst0 = prev.fdata[best_dir].dst;
                        let dst1 = curr.fdata[best_dir].dst;
                        let dst2 = dst;
                        let dst3 = next.fdata[best_dir].dst;
                        let eps = FLT_EPSILON;
                        let (mut n_isect, i0, i1) =
                            isect_line_line_epsilon(dst0, dst1, dst2, dst3, eps);
                        if n_isect != 0 {
                            let dir1 = normalized(dst1 - curr_orig);
                            let dir2 = normalized(dst2 - curr_orig);
                            let (n, len_n) = normalize(cross(dir1, dir2));
                            if len_n < eps {
                                n_isect = 0;
                            } else {
                                let (p1, l1n) = normalize(cross(n, dir1));
                                let (p2, l2n) = normalize(cross(dir2, n));
                                if l1n < eps || l2n < eps {
                                    n_isect = 0;
                                } else {
                                    for pass in 0..n_isect {
                                        let ic = if pass == 0 { i0 } else { i1 } - curr_orig;
                                        if dot(ic, p1) <= 0.0 || dot(ic, p2) <= 0.0 {
                                            n_isect = 0;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        curr.fdata[best_dir].dst = if n_isect != 0 {
                            mid(i0, i1)
                        } else {
                            mid(dst1, dst2)
                        };
                    }
                }
            }
            let s = &mut sv[curr.i as usize];
            let iloc = tc.data[s.td].iloc;
            if curr.fdata[0].f.is_some() {
                s.dir_side[0] = curr.fdata[0].dst - iloc;
            }
            if curr.fdata[1].f.is_some() {
                s.dir_side[1] = curr.fdata[1].dst - iloc;
            }
            s.edge_len = len(s.dir_side[0] - s.dir_side[1]);
            s.loop_nr = loop_nr;
            if i_prev != -1 && prev.i == i_prev {
                break;
            }
            prev = curr;
            curr = next;
            next = next_next;
        }
        loop_nr += 1;
    }
    Some((sv, loop_nr))
}

/// `EdgeSlideData`: the slide vertices of one container and the reference vertex.
struct EdgeSlideData {
    sv: Vec<EdgeSlideVert>,
    curr_sv_index: usize,
}

/// `createEdgeSlideVerts` (single side and the view-dependent direction).
fn create_edge_slide(
    cx: &OpCtx,
    bm: &BMesh,
    tc: &Tc,
    view: &View,
    use_double_side: bool,
) -> Option<EdgeSlideData> {
    let (mut sv, group_len) = edge_slide_data_create(bm, tc)?;
    if sv.is_empty() {
        return None;
    }
    if !use_double_side {
        let mut acc = vec![(Vec2::ZERO, 0usize); group_len as usize];
        for s in &sv {
            let a = &mut acc[s.loop_nr as usize];
            a.0 += Vec2::new(len(s.dir_side[0]), len(s.dir_side[1]));
            a.1 += 1;
        }
        for a in &mut acc {
            a.0 /= a.1 as f32;
        }
        for s in &mut sv {
            let a = acc[s.loop_nr as usize].0;
            if a.y > a.x {
                s.dir_side[0] = s.dir_side[1];
            }
            s.dir_side[1] = Vec3::ZERO;
            s.edge_len = len(s.dir_side[0]);
        }
    }
    let mut data = EdgeSlideData {
        sv,
        curr_sv_index: 0,
    };
    // `calcEdgeSlide_mval_range` with the mouse at the region origin.
    let c = cx.ctx();
    let occlude = c
        .main
        .objects
        .get(tc.ob)
        .is_some_and(|o| matches!(o.display_type.as_str(), "SOLID" | "TEXTURED"))
        && {
            let s = &c.win.view3d.shading;
            !(if s.r#type == "WIREFRAME" {
                s.show_xray_wireframe
            } else {
                s.show_xray
            })
        };
    let tris = occlude.then(|| Tris::new(bm));
    let origin = tc.imat.transform_point3(view.center_ray_origin());
    let proj = view.proj_mat(&tc.mat);
    let mval = Vec2::ZERO;
    let mut mval_dir = Vec2::ZERO;
    let mut dist_best = f32::MAX;
    let mut loop_dir = vec![Vec2::ZERO; group_len as usize];
    let mut loop_maxdist = vec![f32::MAX; group_len as usize];
    for (i, s) in data.sv.iter().enumerate() {
        let td = &tc.data[s.td];
        let visible = match &tris {
            None => true,
            Some(t) => {
                let v = td.v.expect("vertex");
                bm.vert_edges(v).into_iter().any(|e| {
                    !has(bm, e, ElemFlag::SELECT)
                        && !has(bm, e, ElemFlag::HIDDEN)
                        && edge_visible(bm, t, e, origin)
                })
            }
        };
        let sco_a = project(&proj, td.iloc + s.dir_side[0]);
        let sco_b = project(&proj, td.iloc + s.dir_side[1]);
        let dist_sq = dist_sq_to_segment(mval, sco_b, sco_a);
        if visible {
            let d = sco_b - sco_a;
            if dist_sq < dist_best && (d.x * d.x + d.y * d.y) > 0.1 {
                dist_best = dist_sq;
                mval_dir = d;
                data.curr_sv_index = i;
            }
        }
        if use_double_side {
            let l = s.loop_nr as usize;
            if dist_sq < loop_maxdist[l] {
                loop_maxdist[l] = dist_sq;
                loop_dir[l] = sco_b - sco_a;
            }
        }
    }
    if use_double_side {
        for s in &mut data.sv {
            let l = s.loop_nr as usize;
            if loop_dir[l].dot(mval_dir) < 0.0 {
                s.dir_side.swap(0, 1);
            }
        }
    }
    Some(data)
}

/// `interp_line_v3_v3v3v3`.
fn interp_line(v1: Vec3, v2: Vec3, v3: Vec3, t: f32) -> Vec3 {
    let t_mid = line_point_factor(v2, v1, v3);
    let t_delta = t - t_mid;
    if t_delta < 0.0 {
        if t_mid.abs() < FLT_EPSILON {
            v2
        } else {
            interp(v1, v2, t / t_mid)
        }
    } else {
        let t = t - t_mid;
        let t_mid = 1.0 - t_mid;
        if t_mid.abs() < FLT_EPSILON {
            v3
        } else {
            interp(v2, v3, t / t_mid)
        }
    }
}

/// `edge_slide_apply_elem`.
#[allow(clippy::too_many_arguments)]
fn edge_slide_apply(
    sv: &EdgeSlideVert,
    orig: Vec3,
    fac: f32,
    curr_length_fac: f32,
    curr_side_unclamp: usize,
    use_clamp: bool,
    use_even: bool,
    use_flip: bool,
) -> Vec3 {
    let mut r = orig;
    if !use_even {
        if use_clamp {
            let side = (fac < 0.0) as usize;
            r = madd(r, sv.dir_side[side], fac.abs());
        } else {
            let mut side = curr_side_unclamp;
            if is_zero(sv.dir_side[side]) {
                side = 1 - side;
            }
            let fac_final = if side == (fac < 0.0) as usize {
                fac.abs()
            } else {
                -fac.abs()
            };
            r = madd(r, sv.dir_side[side], fac_final);
        }
    } else if sv.edge_len > FLT_EPSILON {
        let fac_final = sv.edge_len.min(curr_length_fac) / sv.edge_len;
        let co_a = r + sv.dir_side[0];
        let co_b = r + sv.dir_side[1];
        r = if use_flip {
            interp_line(co_b, r, co_a, fac_final)
        } else {
            interp_line(co_a, r, co_b, fac_final)
        };
    }
    r
}

// ---------------------------------------------------------------- vertex slide

/// `TransDataVertSlideVert`.
struct VertSlideVert {
    td: usize,
    targets: Vec<Vec3>,
    curr: usize,
}

/// `transform_mesh_vert_slide_data_create` + `update_active_vert/edges`.
fn create_vert_slide(
    bm: &BMesh,
    tc: &Tc,
    view: &View,
    dir: Vec3,
) -> Option<(Vec<VertSlideVert>, usize)> {
    let mut out = Vec::new();
    for (i, td) in tc.data.iter().enumerate() {
        if !td.selected {
            continue;
        }
        let v = td.v?;
        let mut targets = Vec::new();
        if bm.vert(v).e.is_some() {
            for e in bm.vert_edges(v) {
                if has(bm, e, ElemFlag::HIDDEN) {
                    continue;
                }
                targets.push(co(bm, bm.edge_other_vert(e, v)));
            }
        }
        if targets.is_empty() {
            targets.push(td.iloc);
        }
        out.push(VertSlideVert {
            td: i,
            targets,
            curr: 0,
        });
    }
    if out.is_empty() {
        return None;
    }
    let proj = view.proj_mat(&tc.mat);
    let mut best = f32::MAX;
    let mut curr_sv_index = 0;
    for (i, s) in out.iter().enumerate() {
        let p = project(&proj, tc.data[s.td].iloc);
        let d = p.x * p.x + p.y * p.y;
        if d < best {
            best = d;
            curr_sv_index = i;
        }
    }
    let obmat3 = glam::Mat3::from_mat4(tc.mat);
    for s in &mut out {
        if s.targets.len() <= 1 {
            continue;
        }
        let orig = tc.data[s.td].iloc;
        let mut best_dot = -f32::MAX;
        let mut best_j = None;
        for (j, t) in s.targets.iter().enumerate() {
            // `math::normalize`: a zero-length edge gives a zero direction (dot 0).
            let tdir = normalized(obmat3 * (*t - orig));
            let d = dot(dir, tdir);
            if d > best_dot {
                best_dot = d;
                best_j = Some(j);
            }
        }
        if let Some(j) = best_j {
            s.curr = j;
        }
    }
    Some((out, curr_sv_index))
}

/// `vert_slide_apply_elem`.
fn vert_slide_apply(orig: Vec3, dest: Vec3, perc: f32, use_even: bool, use_flip: bool) -> Vec3 {
    if !use_even {
        return interp(orig, dest, perc);
    }
    let (dir, l) = normalize(dest - orig);
    if l > FLT_EPSILON {
        if use_flip {
            madd(dest, dir, -perc)
        } else {
            madd(orig, dir, perc)
        }
    } else {
        orig
    }
}

// ---------------------------------------------------------------- UV correction

/// Copies the values of slot `slot` of every layer from `src` to `dst` (same layout).
fn copy_slot(src: &CustomData, dst: &mut CustomData, slot: usize) {
    macro_rules! cp {
        ($a:expr, $b:expr) => {{
            let v = $a[slot].clone();
            Arc::make_mut($b)[slot] = v;
        }};
    }
    for (s, d) in src.layers.iter().zip(dst.layers.iter_mut()) {
        match (&s.data, &mut d.data) {
            (AttrData::Float(a), AttrData::Float(b)) => cp!(a, b),
            (AttrData::Int(a), AttrData::Int(b)) => cp!(a, b),
            (AttrData::Bool(a), AttrData::Bool(b)) => cp!(a, b),
            (AttrData::Float2(a), AttrData::Float2(b)) => cp!(a, b),
            (AttrData::Float3(a), AttrData::Float3(b)) => cp!(a, b),
            (AttrData::Float4(a), AttrData::Float4(b)) => cp!(a, b),
            (AttrData::FloatColor(a), AttrData::FloatColor(b)) => cp!(a, b),
            (AttrData::ByteColor(a), AttrData::ByteColor(b)) => cp!(a, b),
            (AttrData::Quaternion(a), AttrData::Quaternion(b)) => cp!(a, b),
            // Types without interpolation keep the destination's value.
            _ => {}
        }
    }
}

/// A "math" loop layer (`CustomData_layer_has_math`): its values and equality test.
#[derive(Clone, Copy)]
enum MathCol {
    Float2,
    FloatColor,
    ByteColor,
}

fn math_layers(ld: &CustomData) -> Vec<(usize, MathCol)> {
    ld.layers
        .iter()
        .enumerate()
        .filter_map(|(i, l)| match &l.data {
            AttrData::Float2(_) => Some((i, MathCol::Float2)),
            AttrData::FloatColor(_) => Some((i, MathCol::FloatColor)),
            AttrData::ByteColor(_) => Some((i, MathCol::ByteColor)),
            _ => None,
        })
        .collect()
}

fn loop_value(ld: &CustomData, layer: usize, slot: usize) -> [f32; 4] {
    match &ld.layers[layer].data {
        AttrData::Float2(c) => [c[slot][0], c[slot][1], 0.0, 0.0],
        AttrData::FloatColor(c) => c[slot],
        AttrData::ByteColor(c) => c[slot].map(f32::from),
        _ => [0.0; 4],
    }
}

/// `CustomData_data_equals` for the math types.
fn values_equal(kind: MathCol, a: [f32; 4], b: [f32; 4]) -> bool {
    match kind {
        MathCol::Float2 => {
            let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
            dx * dx + dy * dy < 0.00001
        }
        MathCol::FloatColor | MathCol::ByteColor => {
            let mut tot = 0.0f32;
            for i in 0..4 {
                let c = a[i] - b[i];
                tot += c * c;
            }
            tot < 0.001
        }
    }
}

/// `round_fl_to_uchar_clamp`.
fn round_uchar(f: f32) -> u8 {
    if f <= 0.0 {
        0
    } else if f > 255.0 - 0.5 {
        255
    } else {
        (f + 0.5) as u8
    }
}

/// `CustomData_bmesh_interp_n` of one math layer, then the copy to every loop of the group.
fn merge_group(ld: &mut CustomData, layer: usize, kind: MathCol, slots: &[usize], w: &[f32]) {
    let mut acc = [0.0f32; 4];
    for (&s, &wt) in slots.iter().zip(w) {
        let v = loop_value(ld, layer, s);
        for k in 0..4 {
            acc[k] += v[k] * wt;
        }
    }
    match (&mut ld.layers[layer].data, kind) {
        (AttrData::Float2(c), MathCol::Float2) => {
            let c = Arc::make_mut(c);
            for &s in slots {
                c[s] = [acc[0], acc[1]];
            }
        }
        (AttrData::FloatColor(c), MathCol::FloatColor) => {
            let c = Arc::make_mut(c);
            for &s in slots {
                c[s] = acc;
            }
        }
        (AttrData::ByteColor(c), MathCol::ByteColor) => {
            let c = Arc::make_mut(c);
            let v = acc.map(round_uchar);
            for &s in slots {
                c[s] = v;
            }
        }
        _ => {}
    }
}

/// One loop fan of equal data around a vertex (`LoopGroupCD`).
struct LoopGroup {
    /// Loop slots.
    slots: Vec<usize>,
    /// Indices in the vertex's `BM_LOOPS_OF_VERT` order.
    index: Vec<usize>,
    /// Normalized angle weights.
    weights: Vec<f32>,
}

/// `BM_vert_loop_groups_data_layer_create`: groups of the loops of `v` (`loops`) connected
/// through edges with equal data, discovered as Blender does (recursive walk, groups
/// prepended).
fn loop_groups(
    bm: &BMesh,
    loops: &[LoopId],
    layer: usize,
    kind: MathCol,
    loop_weights: &[f32],
) -> Vec<LoopGroup> {
    let mut tag: Vec<bool> = vec![true; loops.len()];
    let pos = |l: LoopId| loops.iter().position(|x| *x == l);
    let mut groups = Vec::new();
    for (k, &l) in loops.iter().enumerate() {
        if !tag[k] {
            continue;
        }
        let data_ref = loop_value(&bm.ldata, layer, l.slot());
        let mut g = LoopGroup {
            slots: Vec::new(),
            index: Vec::new(),
            weights: Vec::new(),
        };
        let mut accum = 0.0f32;
        // Recursive `bm_loop_walk_data`, written with an explicit stack of pending branches.
        #[allow(clippy::too_many_arguments)]
        fn walk(
            bm: &BMesh,
            l_walk: LoopId,
            k: usize,
            ctx: &mut (Vec<bool>, &mut LoopGroup, &mut f32),
            data_ref: [f32; 4],
            layer: usize,
            kind: MathCol,
            loop_weights: &[f32],
            pos: &dyn Fn(LoopId) -> Option<usize>,
        ) {
            let w = loop_weights[k];
            ctx.0[k] = false;
            ctx.1.slots.push(l_walk.slot());
            ctx.1.index.push(k);
            ctx.1.weights.push(w);
            *ctx.2 += w;
            for i in 0..2 {
                let lw = bm.lp(l_walk);
                let base = if i == 0 { l_walk } else { lw.prev };
                let mut l_other = bm.lp(base).radial_next;
                if bm.lp(l_other).radial_next != l_other {
                    if bm.lp(l_other).v != lw.v {
                        l_other = bm.lp(l_other).next;
                    }
                    if let Some(ko) = pos(l_other)
                        && ctx.0[ko]
                        && values_equal(
                            kind,
                            data_ref,
                            loop_value(&bm.ldata, layer, l_other.slot()),
                        )
                    {
                        walk(
                            bm,
                            l_other,
                            ko,
                            ctx,
                            data_ref,
                            layer,
                            kind,
                            loop_weights,
                            pos,
                        );
                    }
                }
            }
        }
        {
            let mut ctx = (std::mem::take(&mut tag), &mut g, &mut accum);
            walk(
                bm,
                l,
                k,
                &mut ctx,
                data_ref,
                layer,
                kind,
                loop_weights,
                &pos,
            );
            tag = ctx.0;
        }
        let n = g.weights.len();
        if accum != 0.0 {
            let f = 1.0 / accum;
            for w in &mut g.weights {
                *w *= f;
            }
        } else {
            for w in &mut g.weights {
                *w = 1.0 / n as f32;
            }
        }
        groups.insert(0, g);
    }
    groups
}

/// Per corrected vertex: its loops (`BM_LOOPS_OF_VERT`) and the merge groups of every math layer.
struct CorrectVert {
    v: VertId,
    iloc: Vec3,
    loops: Vec<LoopId>,
    groups: Vec<(usize, MathCol, Vec<LoopGroup>)>,
}

/// `mesh_customdatacorrect_create(tc, use_merge_group = true)`: run on the original mesh.
fn correct_init(bm: &BMesh, tc: &Tc, moved: &[usize]) -> Vec<CorrectVert> {
    let layers = math_layers(&bm.ldata);
    let mut out = Vec::new();
    for &i in moved {
        let td = &tc.data[i];
        let v = td.v.expect("vertex data");
        let loops = bm.vert_loops(v);
        let mut loop_weights = vec![0.0f32; loops.len()];
        for (j, &l) in loops.iter().enumerate() {
            let ll = bm.lp(l);
            if let Some(lp) = loop_find_prev_nodouble(bm, l, ll.next, FLT_EPSILON)
                && let Some(ln) = loop_find_next_nodouble(bm, l, lp, FLT_EPSILON)
            {
                loop_weights[j] =
                    angle_v3v3v3(co(bm, bm.lp(lp).v), co(bm, ll.v), co(bm, bm.lp(ln).v));
            }
        }
        let groups = if loops.is_empty() {
            Vec::new()
        } else {
            layers
                .iter()
                .map(|&(li, kind)| (li, kind, loop_groups(bm, &loops, li, kind, &loop_weights)))
                .collect()
        };
        out.push(CorrectVert {
            v,
            iloc: td.iloc,
            loops,
            groups,
        });
    }
    out
}

/// `mesh_customdatacorrect_apply`: interpolates the loop data of every corner of the moved
/// vertices from the original faces, then merges the originally connected loops.
fn correct_apply(bm: &mut BMesh, orig: &BMesh, verts: &[CorrectVert]) {
    let mut scratch = orig.ldata.clone();
    for cv in verts {
        let vco = co(bm, cv.v);
        let is_moved = len_sq(vco - cv.iloc) > FLT_EPSILON;
        let has_math = cv.groups.iter().any(|g| !g.2.is_empty());
        let do_loop_weight = is_moved && !cv.groups.is_empty();
        let axis = vno(bm, cv.v);
        let mut loop_weights = vec![0.0f32; cv.loops.len()];
        let v_proj1 = project_plane_normalized(cv.iloc, axis);
        // `mesh_vert_orig_co_get`: transformed vertices at their original position, the others
        // where they are (unchanged), i.e. the original mesh's positions.
        let orig_get = |v: VertId| co(orig, v);
        for (j, &l) in cv.loops.iter().enumerate() {
            let f = bm.lp(l).f;
            // `BM_loop_interp_from_face(bm, l, f_copy, false, false)` against the original face.
            let fn_ = fno(orig, f);
            let axis_f = if !is_zero(fn_) {
                fn_
            } else {
                let t = super::bmq::face_calc_tangent_auto(orig, f);
                normalized(super::mathx::ortho(t))
            };
            let rows = frap_bmesh::query::geom::axis_dominant_v3_rows(axis_f.to_array());
            let mut cos = Vec::new();
            let mut src = Vec::new();
            for lo in orig.face_loops(f) {
                cos.push(frap_bmesh::query::geom::project_v2(
                    &rows,
                    orig.vert(orig.lp(lo).v).co,
                ));
                src.push(lo.0);
            }
            let p = frap_bmesh::query::geom::project_v2(&rows, vco.to_array());
            let w = frap_bmesh::query::geom::interp_weights_poly_v2(&cos, p);
            scratch.interp(&src, &w, l.0);
            copy_slot(&scratch, &mut bm.ldata, l.slot());
            copy_slot(&orig.ldata, &mut scratch, l.slot());

            if do_loop_weight {
                let eps = 1.0e-8f32;
                let ll = bm.lp(l);
                let (mut l_prev, mut l_next) = (ll.prev, ll.next);
                let mut v0 = project_plane_normalized(orig_get(bm.lp(l_prev).v), axis);
                let mut prev_ok;
                loop {
                    prev_ok = len_sq(v_proj1 - v0) > eps;
                    if prev_ok {
                        break;
                    }
                    l_prev = bm.lp(l_prev).prev;
                    if l_prev == ll.next {
                        break;
                    }
                    v0 = project_plane_normalized(orig_get(bm.lp(l_prev).v), axis);
                }
                let mut v2 = project_plane_normalized(orig_get(bm.lp(l_next).v), axis);
                let mut next_ok;
                loop {
                    next_ok = len_sq(v_proj1 - v2) > eps;
                    if next_ok {
                        break;
                    }
                    l_next = bm.lp(l_next).next;
                    if l_next == ll.prev {
                        break;
                    }
                    v2 = project_plane_normalized(orig_get(bm.lp(l_next).v), axis);
                }
                loop_weights[j] = if prev_ok && next_ok {
                    let dist = dist_signed_squared_to_corner(vco, v0, v_proj1, v2, axis);
                    let w = if dist >= 0.0 {
                        1.0
                    } else if dist <= -eps {
                        0.0
                    } else {
                        1.0 + dist / eps
                    };
                    if w.is_finite() { w } else { 0.0 }
                } else {
                    0.0
                };
            }
        }
        if !has_math {
            continue;
        }
        for (layer, kind, groups) in &cv.groups {
            for g in groups {
                let w: Vec<f32> = if do_loop_weight {
                    let mut tw: Vec<f32> = g
                        .index
                        .iter()
                        .zip(&g.weights)
                        .map(|(&i, &dw)| loop_weights[i] * dw)
                        .collect();
                    let acc: f32 = tw.iter().fold(0.0f32, |a, b| a + b);
                    if acc != 0.0 {
                        let f = 1.0 / acc;
                        for x in &mut tw {
                            *x *= f;
                        }
                        tw
                    } else {
                        g.weights.clone()
                    }
                } else {
                    g.weights.clone()
                };
                merge_group(&mut bm.ldata, *layer, *kind, &g.slots, &w);
            }
        }
    }
}

// ---------------------------------------------------------------- driver

/// Runs an edge or vertex slide on the prepared transform data.
pub fn run(cx: &mut OpCtx, mut t: TransInfo) -> OpResult {
    let p = t.req.slide.clone();
    let view = View::new(&cx.ctx().win.view3d.view_matrix);
    let is_edge = t.req.mode == Mode::EdgeSlide;
    // Per container: the slide data, or `None` when its selection cannot slide.
    enum Data {
        Edge(EdgeSlideData),
        Vert(Vec<VertSlideVert>, usize),
    }
    let mut datas: Vec<Option<Data>> = Vec::new();
    let origs: Vec<BMesh>;
    {
        let dir = p.direction.map_or(Vec3::X, normalized);
        let (_, edit) = cx.split_edit();
        let bms: Vec<BMesh> = t
            .tcs
            .iter()
            .map(|tc| (*edit.meshes[&tc.me.key()].bm).clone())
            .collect();
        for (tc, bm) in t.tcs.iter().zip(&bms) {
            datas.push(if is_edge {
                create_edge_slide(cx, bm, tc, &view, !p.single_side).map(Data::Edge)
            } else {
                create_vert_slide(bm, tc, &view, dir).map(|(s, c)| Data::Vert(s, c))
            });
        }
        origs = bms;
    }
    if datas.iter().all(Option::is_none) {
        cx.report(
            ReportType::Warning,
            if is_edge {
                "frap: edge slide needs a selection of edge loops or chains (each selected vertex \
                 on one or two selected edges, each edge with one or two faces)"
            } else {
                "frap: nothing to slide"
            },
        );
        return Ok(OpStatus::Cancelled);
    }
    let use_clamp = p.use_clamp;
    let mut perc = t.req.values[0];
    if is_edge {
        let flipped = if p.single_side { !p.flipped } else { p.flipped };
        if use_clamp {
            perc = perc.clamp(-1.0, 1.0);
        }
        // `edge_slide_container_first_ok`: the first container with slide data.
        let curr_length_fac = if p.use_even {
            datas
                .iter()
                .zip(&t.tcs)
                .find_map(|(d, _)| match d {
                    Some(Data::Edge(sd)) => Some(sd),
                    _ => None,
                })
                .map_or(0.0, |sd| {
                    let s = &sd.sv[sd.curr_sv_index];
                    s.edge_len * (((if flipped { perc } else { -perc }) + 1.0) / 2.0)
                })
        } else {
            0.0
        };
        // `slp->curr_side_unclamp` is only written by clamped slides: 0 in a single call.
        let side_unclamp = 0;
        for (tc, d) in t.tcs.iter_mut().zip(&datas) {
            let Some(Data::Edge(sd)) = d else { continue };
            for s in &sd.sv {
                let orig = tc.data[s.td].iloc;
                tc.data[s.td].loc = edge_slide_apply(
                    s,
                    orig,
                    perc,
                    curr_length_fac,
                    side_unclamp,
                    use_clamp,
                    p.use_even,
                    flipped,
                );
            }
        }
    } else {
        if use_clamp {
            perc = perc.clamp(0.0, 1.0);
        }
        for (tc, d) in t.tcs.iter_mut().zip(&datas) {
            let Some(Data::Vert(svs, curr)) = d else {
                continue;
            };
            let mut tperc = perc;
            if p.use_even {
                let s = &svs[*curr];
                tperc *= len(s.targets[s.curr] - tc.data[s.td].iloc);
            }
            for s in svs {
                let orig = tc.data[s.td].iloc;
                tc.data[s.td].loc =
                    vert_slide_apply(orig, s.targets[s.curr], tperc, p.use_even, p.flipped);
            }
        }
    }
    // Containers without slide data keep their vertices.
    let correct = p.correct_uv.unwrap_or(true);
    let mut corrections = Vec::new();
    for ((tc, d), orig) in t.tcs.iter().zip(&datas).zip(&origs) {
        let moved: Vec<usize> = match d {
            Some(Data::Edge(sd)) => sd.sv.iter().map(|s| s.td).collect(),
            Some(Data::Vert(svs, _)) => svs.iter().map(|s| s.td).collect(),
            None => Vec::new(),
        };
        corrections.push(if correct && !math_layers(&orig.ldata).is_empty() {
            Some(correct_init(orig, tc, &moved))
        } else {
            None
        });
    }
    // Positions and normals, then the loop data against the original faces: Blender corrects
    // once in `recalcData_mesh` and again, with the updated normals, in
    // `special_aftertrans_update__mesh`; the second pass decides the result.
    write_back(cx, &t)?;
    for ((tc, corr), orig) in t.tcs.iter().zip(&corrections).zip(&origs) {
        if let Some(c) = corr {
            let bm = cx.bmesh_mut(tc.me)?;
            correct_apply(bm, orig, c);
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watertight_ray() {
        let (a, b, c) = (
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let d = isect_ray_tri_watertight(Vec3::new(0.0, 0.0, 2.0), -Vec3::Z, a, b, c);
        assert!((d.expect("hit") - 2.0).abs() < 1e-6);
        assert!(isect_ray_tri_watertight(Vec3::new(3.0, 0.0, 2.0), -Vec3::Z, a, b, c).is_none());
        assert!(isect_ray_tri_watertight(Vec3::new(0.0, 0.0, 2.0), Vec3::Z, a, b, c).is_none());
    }

    #[test]
    fn slide_interpolation() {
        let sv = EdgeSlideVert {
            td: 0,
            dir_side: [Vec3::X, -Vec3::Y],
            edge_len: 2f32.sqrt(),
            loop_nr: 0,
        };
        let o = Vec3::ZERO;
        let at = |fac: f32, clamp: bool| edge_slide_apply(&sv, o, fac, 0.0, 0, clamp, false, false);
        assert_eq!(at(0.5, true), Vec3::new(0.5, 0.0, 0.0));
        assert_eq!(at(-0.5, true), Vec3::new(0.0, -0.5, 0.0));
        // Unclamped: the side stays the first one.
        assert_eq!(at(-0.5, false), Vec3::new(-0.5, 0.0, 0.0));
        let p = interp_line(Vec3::X, Vec3::ZERO, -Vec3::Y, 0.25);
        assert!(p.abs_diff_eq(Vec3::new(0.5, 0.0, 0.0), 1e-6), "{p:?}");
        assert_eq!(
            vert_slide_apply(o, Vec3::X * 2.0, 0.25, false, false),
            Vec3::new(0.5, 0.0, 0.0)
        );
        assert_eq!(
            vert_slide_apply(o, Vec3::X * 2.0, 0.25, true, true),
            Vec3::new(1.75, 0.0, 0.0)
        );
        assert_eq!(round_uchar(254.6), 255);
        let a = [0.5, 0.5, 0.0, 0.0];
        assert!(values_equal(MathCol::Float2, a, [0.5, 0.501, 0.0, 0.0]));
        assert!(!values_equal(MathCol::Float2, a, [0.5, 0.51, 0.0, 0.0]));
    }

    #[test]
    fn factory_view_projects_the_origin_inside_the_region() {
        // The factory Layout view matrix of a background Blender 5.2.4 (columns).
        let vm = Mat4::from_cols_array_2d(&[
            [0.410_028_4, -0.401_742_6, 0.818_828_3, 0.0],
            [0.911_976_5, 0.193_643_4, -0.361_664_95, 0.0],
            [-0.013_264_662, 0.895_044_9, 0.445_778_9, 0.0],
            [0.050_467_845, 0.0, -18.388_048, 1.0],
        ]);
        let v = View::new(&vm);
        let p = project(&v.proj_mat(&Mat4::IDENTITY), Vec3::ZERO);
        assert!(
            p.x > 0.0 && p.x < VIEW_WINX && p.y > 0.0 && p.y < VIEW_WINY,
            "{p:?}"
        );
        let o = v.center_ray_origin();
        assert!((len(o) - 18.388).abs() < 0.1, "{o:?}");
    }
}
