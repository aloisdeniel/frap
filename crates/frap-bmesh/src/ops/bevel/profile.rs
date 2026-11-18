// SPDX-License-Identifier: GPL-3.0-or-later
//! Edge profiles: parameters, superellipse sampling with even chords, profile spacing and
//! fullness. Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

use super::boundary::nearly_parallel;
use super::math::{self, M4, V3};
use super::types::{
    Affect, BEVEL_EPSILON, BEVEL_EPSILON_BIG, Bev, Bevel, Bnd, PRO_CIRCLE_R, PRO_LINE_R,
    PRO_SQUARE_IN_R, PRO_SQUARE_R, ProfileSpacing, Vm, VmeshMethod, bevel_epsilon_ang,
};
use super::util::co;
use crate::types::{BMesh, EdgeId, VertId};

impl Bevel {
    /// `set_profile_params`: the profile of `bndv` (mid control point, plane, projection).
    pub(crate) fn set_profile_params(&mut self, bm: &BMesh, bv: Bev, bndv: Bnd) {
        let mut do_linear_interp = true;
        let e = self.bnd(bndv).ebev;
        let start = self.bnd(bndv).nv.co;
        let end = self.bnd(self.bnext(bndv)).nv.co;
        let bvv = self.bev(bv).v;
        let bvco = co(bm, bvv);
        let selcount = self.bev(bv).selcount;
        let is_arc_start = self.bnd(bndv).is_arc_start;
        let pro_super_r = self.pro_super_r;
        let vertices = self.affect_type == Affect::Vertices;
        if let Some(e) = e {
            do_linear_interp = false;
            let ee = self.ehe(e);
            let [ev1, ev2] = bm.edge(ee).v;
            let is_rev = self.eh(e).is_rev;
            let (eprev, enext) = (self.eprev(e), self.enext(e));
            let prev_bev = self.eh(eprev).is_bev;
            let next_bev = self.eh(enext).is_bev;
            let (pe, ne) = (self.ehe(eprev), self.ehe(enext));
            let pro = &mut self.bnds[bndv.0 as usize].profile;
            pro.super_r = pro_super_r;
            // Projection direction is the direction of the edge.
            let mut pd = math::sub(co(bm, ev1), co(bm, ev2));
            if is_rev {
                pd = math::neg(pd);
            }
            pro.proj_dir = math::norm(pd);
            pro.middle = project_to_edge(bm, ee, start, end);
            pro.start = start;
            pro.end = end;
            // Default plane to project onto is the one with triangle start - middle - end.
            let d1 = math::norm(math::sub(pro.middle, start));
            let d2 = math::norm(math::sub(pro.middle, end));
            pro.plane_no = math::norm(math::cross(d1, d2));
            if nearly_parallel(d1, d2) {
                // Start - middle - end are collinear: move the profile to the common plane of
                // the beveled edge and the two boundary verts if possible.
                pro.middle = bvco;
                if prev_bev && next_bev && selcount >= 3 {
                    // Want mid at the meet point of next and prev offset edges.
                    let [pv1, pv2] = bm.edge(pe).v;
                    let [nv1, nv2] = bm.edge(ne).v;
                    let d3 = math::norm(math::sub(co(bm, pv1), co(bm, pv2)));
                    let d4 = math::norm(math::sub(co(bm, nv1), co(bm, nv2)));
                    if nearly_parallel(d3, d4) {
                        // Offset lines are collinear - want linear interpolation.
                        pro.middle = math::mid(start, end);
                        do_linear_interp = true;
                    } else {
                        let co3 = math::add(start, d3);
                        let co4 = math::add(end, d4);
                        let (isect_kind, meetco, _) =
                            math::isect_line_line_v3(start, co3, end, co4);
                        if isect_kind != 0 {
                            pro.middle = meetco;
                        } else {
                            // Offset lines don't intersect - want linear interpolation.
                            pro.middle = math::mid(start, end);
                            do_linear_interp = true;
                        }
                    }
                }
                pro.end = end;
                let d1 = math::norm(math::sub(pro.middle, start));
                let d2 = math::norm(math::sub(pro.middle, end));
                pro.plane_no = math::norm(math::cross(d1, d2));
                if nearly_parallel(d1, d2) {
                    // Whole profile is collinear with edge: just interpolate.
                    do_linear_interp = true;
                } else {
                    pro.plane_co = bvco;
                    pro.proj_dir = pro.plane_no;
                }
            }
            pro.plane_co = start;
        } else if is_arc_start {
            // Assume pro->middle was already set.
            let pro = &mut self.bnds[bndv.0 as usize].profile;
            pro.start = start;
            pro.end = end;
            pro.super_r = PRO_CIRCLE_R;
            pro.plane_co = [0.0; 3];
            pro.plane_no = [0.0; 3];
            pro.proj_dir = [0.0; 3];
            do_linear_interp = false;
        } else if vertices {
            let pro = &mut self.bnds[bndv.0 as usize].profile;
            pro.start = start;
            pro.middle = bvco;
            pro.end = end;
            pro.super_r = pro_super_r;
            pro.plane_co = [0.0; 3];
            pro.plane_no = [0.0; 3];
            pro.proj_dir = [0.0; 3];
            do_linear_interp = false;
        }

        if do_linear_interp {
            let pro = &mut self.bnds[bndv.0 as usize].profile;
            pro.super_r = PRO_LINE_R;
            pro.start = start;
            pro.end = end;
            pro.middle = math::mid(start, end);
            // Won't use projection for this line profile.
            pro.plane_co = [0.0; 3];
            pro.plane_no = [0.0; 3];
            pro.proj_dir = [0.0; 3];
        }
    }

    /// `move_profile_plane`: maybe move the profile plane of `bndv` to the plane of its
    /// profile's start and end and the original vertex `bmvert`.
    pub(crate) fn move_profile_plane(&mut self, bm: &BMesh, bndv: Bnd, bmvert: VertId) {
        let vco = co(bm, bmvert);
        let pro = &mut self.bnds[bndv.0 as usize].profile;
        // Only do this if projecting, and start, end and proj_dir are not coplanar.
        if math::is_zero_v3(pro.proj_dir) {
            return;
        }
        let d1 = math::norm(math::sub(vco, pro.start));
        let d2 = math::norm(math::sub(vco, pro.end));
        let (no, l1) = math::normalize_v3(math::cross(d1, d2));
        let (no2, l2) = math::normalize_v3(math::cross(d1, pro.proj_dir));
        let (no3, l3) = math::normalize_v3(math::cross(d2, pro.proj_dir));
        if l1 > BEVEL_EPSILON_BIG && l2 > BEVEL_EPSILON_BIG && l3 > BEVEL_EPSILON_BIG {
            let dot2 = math::dot(no, no2);
            let dot3 = math::dot(no, no3);
            if dot2.abs() < (1.0 - BEVEL_EPSILON_BIG) && dot3.abs() < (1.0 - BEVEL_EPSILON_BIG) {
                pro.plane_no = no;
            }
        }
        // The parameters changed from their defaults: don't recalculate them later.
        pro.special_params = true;
    }

    /// `move_weld_profile_planes`: the profile planes of the two BoundVerts of a weld.
    pub(crate) fn move_weld_profile_planes(&mut self, bm: &BMesh, bv: Bev, b1: Bnd, b2: Bnd) {
        let vco = co(bm, self.bev(bv).v);
        // Only do this if projecting, and d1, d2 and proj_dir are not coplanar.
        if math::is_zero_v3(self.bnd(b1).profile.proj_dir)
            || math::is_zero_v3(self.bnd(b2).profile.proj_dir)
        {
            return;
        }
        let d1 = math::sub(vco, self.bnd(b1).nv.co);
        let d2 = math::sub(vco, self.bnd(b2).nv.co);
        let (no, l1) = math::normalize_v3(math::cross(d1, d2));
        // "no" is the new normal of the projection plane, but don't move it if it is coplanar
        // with both of the projection directions.
        let (no2, l2) = math::normalize_v3(math::cross(d1, self.bnd(b1).profile.proj_dir));
        let (no3, l3) = math::normalize_v3(math::cross(d2, self.bnd(b2).profile.proj_dir));
        if l1 != 0.0 && (l2 != 0.0 || l3 != 0.0) {
            let dot1 = math::dot(no, no2).abs();
            let dot2 = math::dot(no, no3).abs();
            if (dot1 - 1.0).abs() > BEVEL_EPSILON {
                self.bnd_mut(b1).profile.plane_no = no;
            }
            if (dot2 - 1.0).abs() > BEVEL_EPSILON {
                self.bnd_mut(b2).profile.plane_no = no;
            }
        }
        // The parameters changed from their defaults: don't recalculate them later.
        self.bnd_mut(b1).profile.special_params = true;
        self.bnd_mut(b2).profile.special_params = true;
    }

    /// `get_profile_point`: the point at parameter `i` (of `nseg`) on the profile of `bndv`.
    pub(crate) fn get_profile_point(&self, bndv: Bnd, i: i32, nseg: i32) -> V3 {
        let pro = &self.bnd(bndv).profile;
        if self.seg == 1 {
            return if i == 0 { pro.start } else { pro.end };
        }
        if nseg == self.seg {
            return pro.prof_co[i as usize];
        }
        debug_assert!(math::is_power_of_2_i(nseg) && nseg <= self.pro_spacing.seg_2);
        // Spacing between sub-samples in prof_co_2.
        let subsample_spacing = self.pro_spacing.seg_2 / nseg;
        pro.co2()[(i * subsample_spacing) as usize]
    }

    /// `calculate_profile`: the coordinates of the profile of `bndv` (only for `seg > 1`).
    pub(crate) fn calculate_profile(&mut self, bndv: Bnd, reversed: bool, miter: bool) {
        if self.seg == 1 {
            return;
        }
        let seg = self.seg;
        let seg_2 = self.pro_spacing.seg_2;
        let need_2 = seg != seg_2;
        let vmesh_cutoff = self.vmesh_method == VmeshMethod::Cutoff;
        let spacing = if miter {
            &self.pro_spacing_miter
        } else {
            &self.pro_spacing
        };
        // Blender takes the seg_2 samples from the chosen spacing struct as well.
        let (xs, ys) = (spacing.xvals.clone(), spacing.yvals.clone());
        let (xs2, ys2) = (spacing.x2().to_vec(), spacing.y2().to_vec());
        let pro = &mut self.bnds[bndv.0 as usize].profile;
        if pro.prof_co.is_empty() {
            pro.prof_co = vec![[0.0; 3]; (seg + 1) as usize];
            if need_2 {
                pro.prof_co_2 = vec![[0.0; 3]; (seg_2 + 1) as usize];
            } else {
                pro.prof_co_2 = Vec::new();
            }
        }
        // The profile type is always SUPERELLIPSE (custom profiles are not supported).
        let map = if pro.super_r == PRO_LINE_R {
            None
        } else {
            make_unit_square_map(pro.start, pro.middle, pro.end)
        };
        if vmesh_cutoff && let Some(m) = &map {
            // The "height" of the profile: the distance between the mapped (0,0) and (1,1)
            // corners of the un-transformed profile.
            let bottom_corner = math::mul_v3_m4v3(m, [0.0, 0.0, 0.0]);
            let top_corner = math::mul_v3_m4v3(m, [1.0, 1.0, 0.0]);
            pro.height = math::len_v3v3(bottom_corner, top_corner);
        }
        let mut out = std::mem::take(&mut pro.prof_co);
        calculate_profile_segments(pro, map.as_ref(), reversed, seg, &xs, &ys, &mut out);
        pro.prof_co = out;
        if need_2 {
            let mut out2 = std::mem::take(&mut pro.prof_co_2);
            calculate_profile_segments(pro, map.as_ref(), reversed, seg_2, &xs2, &ys2, &mut out2);
            pro.prof_co_2 = out2;
        }
    }

    /// `calculate_vm_profiles`: the profiles of every BoundVert of `vm` (after all changes to
    /// their parameters).
    pub(crate) fn calculate_vm_profiles(&mut self, bm: &BMesh, bv: Bev, vm: Vm) {
        for bndv in self.bounds(vm) {
            // In special cases the params will have already been set.
            if !self.bnd(bndv).profile.special_params {
                self.set_profile_params(bm, bv, bndv);
            }
            self.calculate_profile(bndv, false, false);
        }
    }

    /// `find_profile_fullness`.
    pub(crate) fn find_profile_fullness(&self) -> f32 {
        const CIRCLE_FULLNESS: [f32; 11] = [
            0.0, 0.559, 0.642, 0.551, 0.646, 0.624, 0.646, 0.619, 0.647, 0.639, 0.647,
        ];
        let nseg = self.seg;
        if self.pro_super_r == PRO_LINE_R {
            0.0
        } else if self.pro_super_r == PRO_CIRCLE_R && nseg > 0 && nseg <= 11 {
            CIRCLE_FULLNESS[(nseg - 1) as usize]
        } else if nseg % 2 == 0 {
            2.4506 * self.profile - 0.00000300 * nseg as f32 - 0.6266
        } else {
            2.3635 * self.profile + 0.000152 * nseg as f32 - 0.6060
        }
    }

    /// `set_profile_spacing` for the superellipse profile.
    pub(crate) fn set_profile_spacing(&mut self) {
        let seg = self.seg;
        let mut ps = ProfileSpacing::default();
        if seg <= 1 {
            self.pro_spacing = ps;
            return;
        }
        let seg_2 = math::power_of_2_max_i(seg).max(4);
        ps.seg_2 = seg_2;
        if seg_2 != seg {
            ps.xvals_2 = vec![0.0; (seg_2 + 1) as usize];
            ps.yvals_2 = vec![0.0; (seg_2 + 1) as usize];
            find_even_superellipse_chords(
                seg_2,
                self.pro_super_r,
                &mut ps.xvals_2,
                &mut ps.yvals_2,
            );
        }
        ps.xvals = vec![0.0; (seg + 1) as usize];
        ps.yvals = vec![0.0; (seg + 1) as usize];
        find_even_superellipse_chords(seg, self.pro_super_r, &mut ps.xvals, &mut ps.yvals);
        self.pro_spacing = ps;
    }
}

/// `project_to_edge`: the point on `e` where the line `(co_a, co_b)` comes closest.
fn project_to_edge(bm: &BMesh, e: EdgeId, co_a: V3, co_b: V3) -> V3 {
    let [v1, v2] = bm.edge(e).v;
    let (k, p, _) = math::isect_line_line_v3(co(bm, v1), co(bm, v2), co_a, co_b);
    if k == 0 { co(bm, v1) } else { p }
}

/// `make_unit_square_map`: the matrix mapping the unit square onto the parallelogram with
/// corners `va`, `vmid`, `vb` (`None` when degenerate).
pub(crate) fn make_unit_square_map(va: V3, vmid: V3, vb: V3) -> Option<M4> {
    let va_vmid = math::sub(vmid, va);
    let vb_vmid = math::sub(vmid, vb);
    if math::is_zero_v3(va_vmid) || math::is_zero_v3(vb_vmid) {
        return None;
    }
    if (math::angle_v3v3(va_vmid, vb_vmid) - math::PI_F).abs() <= bevel_epsilon_ang() {
        return None;
    }
    let vo = math::sub(va, vb_vmid);
    let vddir = math::norm(math::cross(vb_vmid, va_vmid));
    let vd = math::add(vo, vddir);
    // The columns are vmid - va, vmid - vb, vmid + vd - va - vb, va + vb - vmid.
    let c0 = math::sub(vmid, va);
    let c1 = math::sub(vmid, vb);
    let c2 = math::sub(math::sub(math::add(vmid, vd), va), vb);
    let c3 = math::sub(math::add(va, vb), vmid);
    Some([
        [c0[0], c0[1], c0[2], 0.0],
        [c1[0], c1[1], c1[2], 0.0],
        [c2[0], c2[1], c2[2], 0.0],
        [c3[0], c3[1], c3[2], 1.0],
    ])
}

/// `make_unit_cube_map`: maps `(1,0,0)`, `(0,1,0)`, `(0,0,1)`, `(1,1,1)` to `va`, `vb`,
/// `vc`, `vd`.
pub(crate) fn make_unit_cube_map(va: V3, vb: V3, vc: V3, vd: V3) -> M4 {
    let c0 = math::scale(math::add(math::sub(math::sub(va, vb), vc), vd), 0.5);
    let c1 = math::scale(math::add(math::sub(math::sub(vb, va), vc), vd), 0.5);
    let c2 = math::scale(math::add(math::sub(math::sub(vc, va), vb), vd), 0.5);
    let c3 = math::scale(math::sub(math::add(math::add(va, vb), vc), vd), 0.5);
    [
        [c0[0], c0[1], c0[2], 0.0],
        [c1[0], c1[1], c1[2], 0.0],
        [c2[0], c2[1], c2[2], 0.0],
        [c3[0], c3[1], c3[2], 1.0],
    ]
}

/// `calculate_profile_segments`: the 3D profile points for `ns` segments.
fn calculate_profile_segments(
    pro: &super::types::Profile,
    map: Option<&M4>,
    reversed: bool,
    ns: i32,
    xvals: &[f64],
    yvals: &[f64],
    out: &mut [V3],
) {
    for k in 0..=ns {
        let p = if k == 0 {
            pro.start
        } else if k == ns {
            pro.end
        } else if let Some(m) = map {
            let (kk, nk) = (k as usize, (ns - k) as usize);
            let p = [
                if reversed {
                    yvals[nk] as f32
                } else {
                    xvals[kk] as f32
                },
                if reversed {
                    xvals[nk] as f32
                } else {
                    yvals[kk] as f32
                },
                0.0,
            ];
            // The 2D -> 3D transformation of the profile coordinates.
            math::mul_v3_m4v3(m, p)
        } else {
            math::interp(pro.start, pro.end, k as f32 / ns as f32)
        };
        // Finish the 2D -> 3D transformation by projecting onto the final profile plane.
        out[k as usize] = if !math::is_zero_v3(pro.proj_dir) {
            let co2 = math::add(p, pro.proj_dir);
            math::isect_line_plane_v3(p, co2, pro.plane_co, pro.plane_no).unwrap_or(p)
        } else {
            p
        };
    }
}

/// `snap_to_superellipsoid`: snap a direction to the superellipsoid with parameter
/// `super_r` (`midline`: for square profiles, snap to both planes).
pub(crate) fn snap_to_superellipsoid(p: V3, super_r: f32, midline: bool) -> V3 {
    let r = super_r;
    if r == PRO_CIRCLE_R {
        return math::norm(p);
    }
    let a = p[0].max(0.0);
    let b = p[1].max(0.0);
    let c = p[2].max(0.0);
    let (mut x, mut y, z);
    if r == PRO_SQUARE_R || r == PRO_SQUARE_IN_R {
        // Only called for a 2d profile.
        z = 0.0;
        x = a.min(1.0);
        y = b.min(1.0);
        if r == PRO_SQUARE_R {
            // Snap to the closer of the x==1 and y==1 lines, or maybe both.
            let dx = 1.0 - x;
            let dy = 1.0 - y;
            if dx < dy {
                x = 1.0;
                y = if midline { 1.0 } else { y };
            } else {
                y = 1.0;
                x = if midline { 1.0 } else { x };
            }
        } else if x < y {
            // Snap to the closer of the x==0 and y==0 lines, or maybe both.
            x = 0.0;
            y = if midline { 0.0 } else { y };
        } else {
            y = 0.0;
            x = if midline { 0.0 } else { x };
        }
    } else {
        let rinv = 1.0 / r;
        if a == 0.0 {
            if b == 0.0 {
                x = 0.0;
                y = 0.0;
                z = libm::powf(c, rinv);
            } else {
                x = 0.0;
                y = libm::powf(1.0 / (1.0 + libm::powf(c / b, r)), rinv);
                z = c * y / b;
            }
        } else {
            x = libm::powf(
                1.0 / (1.0 + libm::powf(b / a, r) + libm::powf(c / a, r)),
                rinv,
            );
            y = b * x / a;
            z = c * x / a;
        }
    }
    [x, y, z]
}

/// `superellipse_co`: the superellipse `x^r + y^r = 1` at `x` (mirrored by `y = x` when
/// `!rbig`).
fn superellipse_co(x: f64, r: f32, rbig: bool) -> f64 {
    let r = f64::from(r);
    if rbig {
        return libm::pow(1.0 - libm::pow(x, r), 1.0 / r);
    }
    1.0 - libm::pow(1.0 - libm::pow(1.0 - x, r), 1.0 / r)
}

/// `find_superellipse_chord_endpoint`: `xnew > x0` with chord length `dtarget` (Illinois
/// false position).
fn find_superellipse_chord_endpoint(x0: f64, dtarget: f64, r: f32, rbig: bool) -> f64 {
    let y0 = superellipse_co(x0, r, rbig);
    let tol = 1e-13;
    let maxiter = 10;
    let mut xmin = (x0 + std::f64::consts::SQRT_2 / 2.0 * dtarget).min(1.0);
    let mut xmax = (x0 + dtarget).min(1.0);
    let ymin = superellipse_co(xmin, r, rbig);
    let ymax = superellipse_co(xmax, r, rbig);
    let mut dmaxerr = (libm::pow(xmax - x0, 2.0) + libm::pow(ymax - y0, 2.0)).sqrt() - dtarget;
    let mut dminerr = (libm::pow(xmin - x0, 2.0) + libm::pow(ymin - y0, 2.0)).sqrt() - dtarget;
    let mut xnew = xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr);
    let mut lastupdated_upper = true;
    for _ in 0..maxiter {
        let ynew = superellipse_co(xnew, r, rbig);
        let dnewerr = (libm::pow(xnew - x0, 2.0) + libm::pow(ynew - y0, 2.0)).sqrt() - dtarget;
        if dnewerr.abs() < tol {
            break;
        }
        if dnewerr < 0.0 {
            xmin = xnew;
            dminerr = dnewerr;
            if !lastupdated_upper {
                xnew = (dmaxerr / 2.0 * xmin - dminerr * xmax) / (dmaxerr / 2.0 - dminerr);
            } else {
                xnew = xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr);
            }
            lastupdated_upper = false;
        } else {
            xmax = xnew;
            dmaxerr = dnewerr;
            if lastupdated_upper {
                xnew = (dmaxerr * xmin - dminerr / 2.0 * xmax) / (dmaxerr - dminerr / 2.0);
            } else {
                xnew = xmax - dmaxerr * (xmax - xmin) / (dmaxerr - dminerr);
            }
            lastupdated_upper = true;
        }
    }
    xnew
}

/// `find_even_superellipse_chords_general`.
fn find_even_superellipse_chords_general(seg: i32, r: f32, xvals: &mut [f64], yvals: &mut [f64]) {
    let smoothitermax = 10;
    let error_tol = 1e-7;
    let imax = ((seg + 1) / 2 - 1) as usize; // Ceiling division - 1.
    let seg_odd = seg % 2 == 1;
    let (rbig, mx) = if r > 1.0 {
        (true, libm::pow(0.5, 1.0 / f64::from(r)))
    } else {
        (false, 1.0 - libm::pow(0.5, 1.0 / f64::from(r)))
    };
    // Initial positions, linear spacing along the x axis.
    for i in 0..=imax {
        xvals[i] = i as f64 * mx / f64::from(seg) * 2.0;
        yvals[i] = superellipse_co(xvals[i], r, rbig);
    }
    yvals[0] = 1.0;
    // Smooth distance loop.
    for _ in 0..smoothitermax {
        let mut sum = 0.0;
        let mut dmin = 2.0f64;
        let mut dmax = 0.0f64;
        for i in 0..imax {
            let d = (libm::pow(xvals[i + 1] - xvals[i], 2.0)
                + libm::pow(yvals[i + 1] - yvals[i], 2.0))
            .sqrt();
            sum += d;
            dmax = d.max(dmax);
            dmin = d.min(dmin);
        }
        // For the last distance, weight with 1/2 if seg_odd.
        let davg = if seg_odd {
            sum += std::f64::consts::SQRT_2 / 2.0 * (yvals[imax] - xvals[imax]);
            sum / (imax as f64 + 0.5)
        } else {
            sum += (libm::pow(xvals[imax] - mx, 2.0) + libm::pow(yvals[imax] - mx, 2.0)).sqrt();
            sum / (imax as f64 + 1.0)
        };
        // Max error in tolerance? -> Quit.
        let mut precision_reached = true;
        if dmax - davg > error_tol {
            precision_reached = false;
        }
        if dmin - davg < error_tol {
            precision_reached = false;
        }
        if precision_reached {
            break;
        }
        // Update new coordinates.
        for i in 1..=imax {
            xvals[i] = find_superellipse_chord_endpoint(xvals[i - 1], davg, r, rbig);
            yvals[i] = superellipse_co(xvals[i], r, rbig);
        }
    }
    // Fill remaining.
    if !seg_odd {
        xvals[imax + 1] = mx;
        yvals[imax + 1] = mx;
    }
    for i in imax + 1..=seg as usize {
        yvals[i] = xvals[seg as usize - i];
        xvals[i] = yvals[seg as usize - i];
    }
    if !rbig {
        for i in 0..=seg as usize {
            let temp = xvals[i];
            xvals[i] = 1.0 - yvals[i];
            yvals[i] = 1.0 - temp;
        }
    }
}

/// `find_even_superellipse_chords`: equidistant points on the superellipse quadrant.
pub(crate) fn find_even_superellipse_chords(n: i32, r: f32, xvals: &mut [f64], yvals: &mut [f64]) {
    let seg_odd = n % 2 == 1;
    let n2 = n / 2;
    let nu = n as usize;
    if r == PRO_LINE_R {
        for i in 0..=nu {
            xvals[i] = i as f64 / f64::from(n);
            yvals[i] = 1.0 - i as f64 / f64::from(n);
        }
        return;
    }
    if r == PRO_CIRCLE_R {
        let temp = std::f64::consts::FRAC_PI_2 / f64::from(n);
        for i in 0..=nu {
            xvals[i] = libm::sin(i as f64 * temp);
            yvals[i] = libm::cos(i as f64 * temp);
        }
        return;
    }
    if r == PRO_SQUARE_IN_R {
        if !seg_odd {
            for i in 0..=n2 as usize {
                xvals[i] = 0.0;
                yvals[i] = 1.0 - i as f64 / f64::from(n2);
                xvals[nu - i] = yvals[i];
                yvals[nu - i] = xvals[i];
            }
        } else {
            let temp = 1.0 / (f64::from(n2) + std::f64::consts::SQRT_2 / 2.0);
            for i in 0..=n2 as usize {
                xvals[i] = 0.0;
                yvals[i] = 1.0 - i as f64 * temp;
                xvals[nu - i] = yvals[i];
                yvals[nu - i] = xvals[i];
            }
        }
        return;
    }
    if r == PRO_SQUARE_R {
        if !seg_odd {
            for i in 0..=n2 as usize {
                xvals[i] = i as f64 / f64::from(n2);
                yvals[i] = 1.0;
                xvals[nu - i] = yvals[i];
                yvals[nu - i] = xvals[i];
            }
        } else {
            let temp = 1.0 / (f64::from(n2) + std::f64::consts::SQRT_2 / 2.0);
            for i in 0..=n2 as usize {
                xvals[i] = i as f64 * temp;
                yvals[i] = 1.0;
                xvals[nu - i] = yvals[i];
                yvals[nu - i] = xvals[i];
            }
        }
        return;
    }
    find_even_superellipse_chords_general(n, r, xvals, yvals);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(x: &[f64], y: &[f64], i: usize) -> f64 {
        let (dx, dy) = (x[i + 1] - x[i], y[i + 1] - y[i]);
        (dx * dx + dy * dy).sqrt()
    }

    #[test]
    fn circle_chords_are_even() {
        let mut x = vec![0.0; 5];
        let mut y = vec![0.0; 5];
        find_even_superellipse_chords(4, PRO_CIRCLE_R, &mut x, &mut y);
        let d: Vec<f64> = (0..4).map(|i| chord(&x, &y, i)).collect();
        for w in d.windows(2) {
            assert!((w[0] - w[1]).abs() < 1e-12);
        }
    }

    #[test]
    fn general_chords_are_nearly_even() {
        for (n, r) in [(3, 3.0f32), (4, 0.7), (5, 1.5), (8, 4.0)] {
            let mut x = vec![0.0; n + 1];
            let mut y = vec![0.0; n + 1];
            find_even_superellipse_chords(n as i32, r, &mut x, &mut y);
            assert!((x[0] - 0.0).abs() < 1e-12 && (y[0] - 1.0).abs() < 1e-12);
            assert!((x[n] - 1.0).abs() < 1e-9 && y[n].abs() < 1e-9);
            let d: Vec<f64> = (0..n).map(|i| chord(&x, &y, i)).collect();
            let avg = d.iter().sum::<f64>() / n as f64;
            for di in d {
                assert!((di - avg).abs() < 1e-3, "n={n} r={r}");
            }
        }
    }

    #[test]
    fn unit_maps() {
        let m = make_unit_square_map([0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]).unwrap();
        let p = math::mul_v3_m4v3(&m, [0.0, 1.0, 0.0]);
        assert!(math::len_v3v3(p, [0.0, 1.0, 0.0]) < 1e-6);
        let c = make_unit_cube_map(
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        );
        let p = math::mul_v3_m4v3(&c, [1.0, 1.0, 1.0]);
        assert!(math::len_v3v3(p, [1.0, 1.0, 1.0]) < 1e-6);
        assert!(make_unit_square_map([0.0; 3], [0.0; 3], [1.0, 0.0, 0.0]).is_none());
    }
}
