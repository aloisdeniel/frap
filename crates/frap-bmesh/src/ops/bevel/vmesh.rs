// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex meshes of the ADJ pattern: Catmull-Clark style subdivision from a 2-segment
//! control mesh, interpolation to the segment count, the cube-corner, pipe and square-profile
//! special cases. Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc (Blender
//! 5.2.4, GPL-2.0-or-later).

use super::math::{self, V3};
use super::profile::{make_unit_cube_map, make_unit_square_map, snap_to_superellipsoid};
use super::types::{
    Affect, AngleKind, BEVEL_EPSILON, BEVEL_EPSILON_BIG, BEVEL_EPSILON_D, BEVEL_EPSILON_SQ, Bev,
    Bevel, Bnd, PRO_SQUARE_IN_R, PRO_SQUARE_R, Vm, bevel_epsilon_ang, bevel_small_ang,
};
use super::util::{co, edge_calc_face_angle_signed_ex, fno};
use crate::types::BMesh;

/// `sabin_gamma`: gamma for smooth Catmull-Clark, Sabin modification.
#[allow(clippy::excessive_precision)] // Blender's literals, rounded to f32 as in C.
fn sabin_gamma(n: i32) -> f32 {
    if n < 3 {
        return 0.0;
    }
    match n {
        3 => return 0.065_247_584,
        4 => return 0.25,
        5 => return 0.401_983_447,
        6 => return 0.523_423_277,
        _ => {}
    }
    let k = libm::cos(std::f64::consts::PI / f64::from(n));
    // x is the real root of x^3 + (4k^2 - 3)x - 2k = 0.
    let k2 = k * k;
    let k4 = k2 * k2;
    let k6 = k4 * k2;
    let y = libm::pow(
        3.0f64.sqrt() * (64.0 * k6 - 144.0 * k4 + 135.0 * k2 - 27.0).sqrt() + 9.0 * k,
        1.0 / 3.0,
    );
    let x = 0.480_749_856_769_136 * y - (0.231_120_424_783_545 * (12.0 * k2 - 9.0)) / y;
    ((k * x + 2.0 * k2 - 1.0) / (x * x * (k * x + 1.0))) as f32
}

/// `interp_range`: `i` such that `frac[i] <= f <= frac[i + 1]` and the fraction of the way.
fn interp_range(frac: &[f32], n: i32, f: f32) -> (i32, f32) {
    for i in 0..n {
        let iu = i as usize;
        if f <= frac[iu + 1] {
            let rest = f - frac[iu];
            let mut r_rest = if rest == 0.0 {
                0.0
            } else {
                rest / (frac[iu + 1] - frac[iu])
            };
            let mut i = i;
            if i == n - 1 && r_rest == 1.0 {
                i = n;
                r_rest = 0.0;
            }
            return (i, r_rest);
        }
    }
    (n, 0.0)
}

/// `avg4`.
fn avg4(a: V3, b: V3, c: V3, d: V3) -> V3 {
    math::scale(math::add(math::add(math::add(a, b), c), d), 0.25)
}

impl Bevel {
    #[inline]
    fn mco(&self, vm: Vm, i: i32, j: i32, k: i32) -> V3 {
        self.vm(vm).mv(i, j, k).co
    }
    #[inline]
    fn mco_canon(&self, vm: Vm, i: i32, j: i32, k: i32) -> V3 {
        let m = self.vm(vm);
        m.mesh[m.canon(i, j, k)].co
    }
    #[inline]
    fn set_mco(&mut self, vm: Vm, i: i32, j: i32, k: i32, p: V3) {
        self.vm_mut(vm).mv_mut(i, j, k).co = p;
    }
    #[inline]
    fn set_mco_canon(&mut self, vm: Vm, i: i32, j: i32, k: i32, p: V3) {
        let m = self.vm_mut(vm);
        let x = m.canon(i, j, k);
        m.mesh[x].co = p;
    }

    /// `vmesh_center`: the centroid of the center poly.
    fn vmesh_center(&self, vm: Vm) -> V3 {
        let m = self.vm(vm);
        let n = m.count;
        let ns2 = m.seg / 2;
        if m.seg % 2 == 1 {
            let mut c = [0.0f32; 3];
            for i in 0..n {
                c = math::add(c, m.mv(i, ns2, ns2).co);
            }
            math::scale(c, 1.0 / n as f32)
        } else {
            m.mv(0, ns2, ns2).co
        }
    }

    /// `fill_vmesh_fracs`: fractions of the way along ring 0 for vertex `i`.
    fn fill_vmesh_fracs(&self, vm: Vm, frac: &mut [f32], i: i32) {
        let mut total = 0.0f32;
        let ns = self.vm(vm).seg;
        frac[0] = 0.0;
        for k in 0..ns {
            total += math::len_v3v3(self.mco(vm, i, 0, k), self.mco(vm, i, 0, k + 1));
            frac[(k + 1) as usize] = total;
        }
        if total > 0.0 {
            for f in frac.iter_mut().take(ns as usize + 1).skip(1) {
                *f /= total;
            }
        } else {
            frac[ns as usize] = 1.0;
        }
    }

    /// `fill_profile_fracs`: fractions for the profile points of `bndv` with `ns` segments.
    fn fill_profile_fracs(&self, bndv: Bnd, frac: &mut [f32], ns: i32) {
        let mut total = 0.0f32;
        frac[0] = 0.0;
        let mut p = self.bnd(bndv).nv.co;
        for k in 0..ns {
            let nextco = self.get_profile_point(bndv, k + 1, ns);
            total += math::len_v3v3(p, nextco);
            frac[(k + 1) as usize] = total;
            p = nextco;
        }
        if total > 0.0 {
            for f in frac.iter_mut().take(ns as usize + 1).skip(1) {
                *f /= total;
            }
        } else {
            frac[ns as usize] = 1.0;
        }
    }

    /// `interp_vmesh`: interpolate `vm_in` to a vmesh with `nseg` segments on the profiles.
    fn interp_vmesh(&mut self, vm_in: Vm, nseg: i32) -> Vm {
        let n_bndv = self.vm(vm_in).count;
        let ns_in = self.vm(vm_in).seg;
        let nseg2 = nseg / 2;
        let odd = nseg % 2;
        let start = self.vm(vm_in).boundstart;
        let vm_out = self.new_adj_vmesh(n_bndv, nseg, start);

        let mut prev_frac = vec![0.0f32; (ns_in + 1) as usize];
        let mut frac = vec![0.0f32; (ns_in + 1) as usize];
        let mut new_frac = vec![0.0f32; (nseg + 1) as usize];
        let mut prev_new_frac = vec![0.0f32; (nseg + 1) as usize];

        self.fill_vmesh_fracs(vm_in, &mut prev_frac, n_bndv - 1);
        let mut bndv = start.expect("vmesh boundary");
        self.fill_profile_fracs(self.bprev(bndv), &mut prev_new_frac, nseg);
        for i in 0..n_bndv {
            self.fill_vmesh_fracs(vm_in, &mut frac, i);
            self.fill_profile_fracs(bndv, &mut new_frac, nseg);
            for j in 0..nseg2 + odd {
                for k in 0..=nseg2 {
                    // Where "fraction" fits into the previous and current "frac".
                    let fraction = new_frac[k as usize];
                    let (k_in, restk) = interp_range(&frac, ns_in, fraction);
                    let fraction = prev_new_frac[(nseg - j) as usize];
                    let (k_in_prev, restkprev) = interp_range(&prev_frac, ns_in, fraction);
                    let mut j_in = ns_in - k_in_prev;
                    let mut restj = -restkprev;
                    if restj > -BEVEL_EPSILON {
                        restj = 0.0;
                    } else {
                        j_in -= 1;
                        restj += 1.0;
                    }
                    // Bilinear interpolation within the source quad.
                    let p = if restj < BEVEL_EPSILON && restk < BEVEL_EPSILON {
                        self.mco_canon(vm_in, i, j_in, k_in)
                    } else {
                        let j0inc = if restj < BEVEL_EPSILON || j_in == ns_in { 0 } else { 1 };
                        let k0inc = if restk < BEVEL_EPSILON || k_in == ns_in { 0 } else { 1 };
                        let quad = [
                            self.mco_canon(vm_in, i, j_in, k_in),
                            self.mco_canon(vm_in, i, j_in, k_in + k0inc),
                            self.mco_canon(vm_in, i, j_in + j0inc, k_in + k0inc),
                            self.mco_canon(vm_in, i, j_in + j0inc, k_in),
                        ];
                        math::interp_bilinear_quad_v3(quad, restk, restj)
                    };
                    self.set_mco(vm_out, i, j, k, p);
                }
            }
            bndv = self.bnext(bndv);
            prev_frac.copy_from_slice(&frac);
            prev_new_frac.copy_from_slice(&new_frac);
        }
        if odd == 0 {
            let center = self.vmesh_center(vm_in);
            self.set_mco(vm_out, 0, nseg2, nseg2, center);
        }
        self.vm_mut(vm_out).copy_equiv_verts();
        vm_out
    }

    /// `cubic_subdiv`: one step of Catmull-Clark subdivision with special boundary rules
    /// (Levin 1999). `vm_in` may be modified.
    fn cubic_subdiv(&mut self, vm_in: Vm) -> Vm {
        let n_boundary = self.vm(vm_in).count;
        let ns_in = self.vm(vm_in).seg;
        let ns_in2 = ns_in / 2;
        debug_assert!(ns_in % 2 == 0);
        let ns_out = 2 * ns_in;
        let start = self.vm(vm_in).boundstart;
        let vm_out = self.new_adj_vmesh(n_boundary, ns_out, start);

        // First adjust the boundary vertices of the input mesh, storing them in the output.
        for i in 0..n_boundary {
            let c = self.mco(vm_in, i, 0, 0);
            self.set_mco(vm_out, i, 0, 0, c);
            for k in 1..ns_in {
                let mut p = self.mco(vm_in, i, 0, k);
                // Smooth boundary rule.
                let co1 = self.mco(vm_in, i, 0, k - 1);
                let co2 = self.mco(vm_in, i, 0, k + 1);
                let mut acc = math::add(co1, co2);
                acc = math::madd(acc, p, -2.0);
                p = math::madd(p, acc, -1.0 / 6.0);
                self.set_mco_canon(vm_out, i, 0, 2 * k, p);
            }
        }
        // Now adjust the odd boundary vertices in the output mesh, based on even ones.
        let mut bndv = start.expect("vmesh boundary");
        for i in 0..n_boundary {
            let mut k = 1;
            while k < ns_out {
                let mut p = self.get_profile_point(bndv, k, ns_out);
                // Smooth (non-custom profile).
                let co1 = self.mco_canon(vm_out, i, 0, k - 1);
                let co2 = self.mco_canon(vm_out, i, 0, k + 1);
                let mut acc = math::add(co1, co2);
                acc = math::madd(acc, p, -2.0);
                p = math::madd(p, acc, -1.0 / 6.0);
                self.set_mco_canon(vm_out, i, 0, k, p);
                k += 2;
            }
            bndv = self.bnext(bndv);
        }
        self.vm_mut(vm_out).copy_equiv_verts();

        // Copy adjusted verts back into vm_in.
        for i in 0..n_boundary {
            for k in 0..ns_in {
                let c = self.mco(vm_out, i, 0, 2 * k);
                self.set_mco(vm_in, i, 0, k, c);
            }
        }
        self.vm_mut(vm_in).copy_equiv_verts();

        // Now the internal vertices, using standard Catmull-Clark and assuming all boundary
        // vertices have valence 4.

        // The new face vertices.
        for i in 0..n_boundary {
            for j in 0..ns_in2 {
                for k in 0..ns_in2 {
                    // Face up and right from (j, k).
                    let p = avg4(
                        self.mco(vm_in, i, j, k),
                        self.mco(vm_in, i, j, k + 1),
                        self.mco(vm_in, i, j + 1, k),
                        self.mco(vm_in, i, j + 1, k + 1),
                    );
                    self.set_mco(vm_out, i, 2 * j + 1, 2 * k + 1, p);
                }
            }
        }
        // The new vertical edge vertices.
        for i in 0..n_boundary {
            for j in 0..ns_in2 {
                for k in 1..=ns_in2 {
                    // Vertical edge between (j, k) and (j+1, k).
                    let p = avg4(
                        self.mco(vm_in, i, j, k),
                        self.mco(vm_in, i, j + 1, k),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k - 1),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k + 1),
                    );
                    self.set_mco(vm_out, i, 2 * j + 1, 2 * k, p);
                }
            }
        }
        // The new horizontal edge vertices.
        for i in 0..n_boundary {
            for j in 1..ns_in2 {
                for k in 0..ns_in2 {
                    // Horizontal edge between (j, k) and (j, k+1).
                    let p = avg4(
                        self.mco(vm_in, i, j, k),
                        self.mco(vm_in, i, j, k + 1),
                        self.mco_canon(vm_out, i, 2 * j - 1, 2 * k + 1),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k + 1),
                    );
                    self.set_mco(vm_out, i, 2 * j, 2 * k + 1, p);
                }
            }
        }
        // The new vertices, not on the border.
        let gamma = 0.25f32;
        let beta = -gamma;
        for i in 0..n_boundary {
            for j in 1..ns_in2 {
                for k in 1..=ns_in2 {
                    // co1 = centroid of adjacent new edge verts.
                    let co1 = avg4(
                        self.mco_canon(vm_out, i, 2 * j, 2 * k - 1),
                        self.mco_canon(vm_out, i, 2 * j, 2 * k + 1),
                        self.mco_canon(vm_out, i, 2 * j - 1, 2 * k),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k),
                    );
                    // co2 = centroid of adjacent new face verts.
                    let co2 = avg4(
                        self.mco_canon(vm_out, i, 2 * j - 1, 2 * k - 1),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k - 1),
                        self.mco_canon(vm_out, i, 2 * j - 1, 2 * k + 1),
                        self.mco_canon(vm_out, i, 2 * j + 1, 2 * k + 1),
                    );
                    // Combine with the original vert with alpha, beta, gamma factors.
                    let mut p = co1;
                    p = math::madd(p, co2, beta);
                    p = math::madd(p, self.mco(vm_in, i, j, k), gamma);
                    self.set_mco(vm_out, i, 2 * j, 2 * k, p);
                }
            }
        }
        self.vm_mut(vm_out).copy_equiv_verts();

        // The center vertex is special.
        let gamma = sabin_gamma(n_boundary);
        let beta = -gamma;
        // Accumulate edge verts in co1, face verts in co2.
        let mut co1 = [0.0f32; 3];
        let mut co2 = [0.0f32; 3];
        for i in 0..n_boundary {
            co1 = math::add(co1, self.mco(vm_out, i, ns_in, ns_in - 1));
            co2 = math::add(co2, self.mco(vm_out, i, ns_in - 1, ns_in - 1));
            co2 = math::add(co2, self.mco(vm_out, i, ns_in - 1, ns_in + 1));
        }
        let mut p = math::scale(co1, 1.0 / n_boundary as f32);
        p = math::madd(p, co2, beta / (2.0 * n_boundary as f32));
        p = math::madd(p, self.mco(vm_in, 0, ns_in2, ns_in2), gamma);
        for i in 0..n_boundary {
            self.set_mco(vm_out, i, ns_in, ns_in, p);
        }

        // Final step: copy the profile vertices to the VMesh's boundary.
        let mut bndv = start.expect("vmesh boundary");
        for i in 0..n_boundary {
            let inext = (i + 1) % n_boundary;
            for k in 0..=ns_out {
                let p = self.get_profile_point(bndv, k, ns_out);
                self.set_mco(vm_out, i, 0, k, p);
                if k >= ns_in && k < ns_out {
                    self.set_mco(vm_out, inext, ns_out - k, 0, p);
                }
            }
            bndv = self.bnext(bndv);
        }
        vm_out
    }

    /// A fresh 3-sided vmesh with BoundVerts at the unit axis points.
    fn cube_corner_vmesh(&mut self, nseg: i32) -> Vm {
        let vm = self.new_adj_vmesh(3, nseg, None);
        self.vm_mut(vm).count = 0; // Reset, so the following loop ends with the right count.
        for i in 0..3 {
            let mut c = [0.0f32; 3];
            c[i] = 1.0;
            self.add_new_bound_vert(vm, c);
        }
        vm
    }

    /// `make_cube_corner_square`: cube corner for `PRO_SQUARE_R` (straight sides).
    fn make_cube_corner_square(&mut self, nseg: i32) -> Vm {
        let ns2 = nseg / 2;
        let vm = self.cube_corner_vmesh(nseg);
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=ns2 {
                    if !self.vm(vm).is_canon(i, j, k) {
                        continue;
                    }
                    let mut c = [0.0f32; 3];
                    c[i as usize] = 1.0;
                    c[((i + 1) % 3) as usize] = k as f32 * 2.0 / nseg as f32;
                    c[((i + 2) % 3) as usize] = j as f32 * 2.0 / nseg as f32;
                    self.set_mco(vm, i, j, k, c);
                }
            }
        }
        self.vm_mut(vm).copy_equiv_verts();
        vm
    }

    /// `make_cube_corner_square_in`: cube corner for `PRO_SQUARE_IN_R` (inward straight
    /// sides): only the boundary is filled.
    fn make_cube_corner_square_in(&mut self, nseg: i32) -> Vm {
        let ns2 = nseg / 2;
        let odd = nseg % 2;
        let vm = self.cube_corner_vmesh(nseg);
        let b = if odd == 1 {
            2.0 / (2.0 * ns2 as f32 + std::f64::consts::SQRT_2 as f32)
        } else {
            2.0 / nseg as f32
        };
        for i in 0..3 {
            for k in 0..=ns2 {
                let mut c = [0.0f32; 3];
                c[i as usize] = 1.0 - k as f32 * b;
                c[((i + 1) % 3) as usize] = 0.0;
                c[((i + 2) % 3) as usize] = 0.0;
                self.set_mco(vm, i, 0, k, c);
                c[((i + 1) % 3) as usize] = 1.0 - k as f32 * b;
                c[((i + 2) % 3) as usize] = 0.0;
                c[i as usize] = 0.0;
                self.set_mco(vm, i, 0, nseg - k, c);
            }
        }
        vm
    }

    /// `make_cube_corner_adj_vmesh`: a vmesh with `seg` segments covering the unit sphere
    /// octant.
    fn make_cube_corner_adj_vmesh(&mut self) -> Vm {
        let nseg = self.seg;
        let r = self.pro_super_r;
        if r == PRO_SQUARE_R {
            return self.make_cube_corner_square(nseg);
        }
        if r == PRO_SQUARE_IN_R {
            return self.make_cube_corner_square_in(nseg);
        }
        // The initial mesh has 3 sides and 2 segments on each side.
        let vm0 = self.cube_corner_vmesh(2);
        let mut bndv = self.bstart(vm0);
        for i in 0..3 {
            // The point halfway around the profile, on the arc between this and the next.
            let mut coc = [0.0f32; 3];
            coc[i] = 1.0;
            coc[(i + 1) % 3] = 1.0;
            coc[(i + 2) % 3] = 0.0;
            let start = self.bnd(bndv).nv.co;
            let end = self.bnd(self.bnext(bndv)).nv.co;
            {
                let pro = &mut self.bnd_mut(bndv).profile;
                pro.super_r = r;
                pro.start = start;
                pro.end = end;
                pro.middle = coc;
                pro.plane_co = start;
                pro.plane_no = math::cross(start, end);
                pro.proj_dir = pro.plane_no;
            }
            self.set_mco(vm0, i as i32, 0, 0, start);
            // Calculate profiles again because we started over with new boundverts.
            self.calculate_profile(bndv, false, false);
            // Just building the boundaries here, so sample the profile halfway through.
            let p = self.get_profile_point(bndv, 1, 2);
            self.set_mco(vm0, i as i32, 0, 1, p);
            bndv = self.bnext(bndv);
        }
        // Center vertex.
        let s3 = (1.0f64 / 3.0f64.sqrt()) as f32;
        let mut c = [s3; 3];
        if nseg > 2 {
            if r > 1.5 {
                c = math::scale(c, 1.4);
            } else if r < 0.75 {
                c = math::scale(c, 0.6);
            }
        }
        self.set_mco(vm0, 0, 1, 1, c);
        self.vm_mut(vm0).copy_equiv_verts();

        let mut vm1 = vm0;
        while self.vm(vm1).seg < nseg {
            vm1 = self.cubic_subdiv(vm1);
        }
        if self.vm(vm1).seg != nseg {
            vm1 = self.interp_vmesh(vm1, nseg);
        }
        // Now snap each vertex to the superellipsoid.
        let ns2 = nseg / 2;
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=nseg {
                    let p = snap_to_superellipsoid(self.mco(vm1, i, j, k), r, false);
                    self.set_mco(vm1, i, j, k, p);
                }
            }
        }
        vm1
    }

    /// `tri_corner_test`: is this a good candidate for `tri_corner_adj_vmesh`? (-1 no, 0
    /// almost, 1 yes)
    pub(crate) fn tri_corner_test(&self, bm: &BMesh, bv: Bev) -> i32 {
        let mut in_plane_e = 0;
        if self.affect_type == Affect::Vertices {
            return -1;
        }
        if self.vm(self.bvm(bv)).count != 3 {
            return 0;
        }
        let b = self.bev(bv);
        // Only use the tri-corner special case if the offset is the same for every edge.
        let offset = self.eh(b.edge(0)).offset_l;
        let mut totang = 0.0f32;
        for i in 0..b.edgecount {
            let e = b.edge(i);
            let ang = edge_calc_face_angle_signed_ex(bm, self.ehe(e), 0.0);
            let absang = ang.abs();
            // `absang <= M_PI_4` compares in double.
            if f64::from(absang) <= std::f64::consts::FRAC_PI_4 {
                in_plane_e += 1;
            } else if absang >= 3.0 * std::f32::consts::FRAC_PI_4 {
                return -1;
            }
            if self.eh(e).is_bev && !math::compare_ff(self.eh(e).offset_l, offset, BEVEL_EPSILON) {
                return -1;
            }
            totang += ang;
        }
        if in_plane_e != b.edgecount - 3 {
            return -1;
        }
        let angdiff = (totang.abs() - 3.0 * std::f32::consts::FRAC_PI_2).abs();
        if (self.pro_super_r == PRO_SQUARE_R && angdiff > math::PI_F / 16.0)
            || angdiff > std::f32::consts::FRAC_PI_4
        {
            return -1;
        }
        if b.edgecount != 3 || b.selcount != 3 {
            return 0;
        }
        1
    }

    /// `tri_corner_adj_vmesh`.
    pub(crate) fn tri_corner_adj_vmesh(&mut self, bm: &BMesh, bv: Bev) -> Vm {
        let bvm = self.bvm(bv);
        let mut bndv = self.bstart(bvm);
        let co0 = self.bnd(bndv).nv.co;
        bndv = self.bnext(bndv);
        let co1 = self.bnd(bndv).nv.co;
        bndv = self.bnext(bndv);
        let co2 = self.bnd(bndv).nv.co;
        let mat = make_unit_cube_map(co0, co1, co2, co(bm, self.bev(bv).v));
        let ns = self.seg;
        let ns2 = ns / 2;
        let vm = self.make_cube_corner_adj_vmesh();
        for i in 0..3 {
            for j in 0..=ns2 {
                for k in 0..=ns {
                    let c = self.mco(vm, i, j, k);
                    let v = math::mul_v4_m4v4(&mat, [c[0], c[1], c[2], 1.0]);
                    self.set_mco(vm, i, j, k, [v[0], v[1], v[2]]);
                }
            }
        }
        vm
    }

    /// `adj_vmesh`: the mesh replacing the original vertex, bounded by the profiles.
    pub(crate) fn adj_vmesh(&mut self, bm: &BMesh, bv: Bev) -> Vm {
        let bvm = self.bvm(bv);
        let n_bndv = self.vm(bvm).count;
        // Same bevel as that of 3 edges of a vert in a cube.
        if n_bndv == 3 && self.tri_corner_test(bm, bv) != -1 && self.pro_super_r != PRO_SQUARE_IN_R
        {
            return self.tri_corner_adj_vmesh(bm, bv);
        }
        // First construct an initial control mesh, with nseg == 2.
        let nseg = self.vm(bvm).seg;
        let start = self.vm(bvm).boundstart;
        let vm0 = self.new_adj_vmesh(n_bndv, 2, start);
        // The center of the boundverts that make up the vmesh.
        let mut bndv = start.expect("vmesh boundary");
        let mut boundverts_center = [0.0f32; 3];
        for i in 0..n_bndv {
            // Boundaries just divide input polygon edges into 2 even segments.
            let c = self.bnd(bndv).nv.co;
            self.set_mco(vm0, i, 0, 0, c);
            let p = self.get_profile_point(bndv, 1, 2);
            self.set_mco(vm0, i, 0, 1, p);
            boundverts_center = math::add(boundverts_center, c);
            bndv = self.bnext(bndv);
        }
        boundverts_center = math::scale(boundverts_center, 1.0 / n_bndv as f32);
        // 'fullness' is the fraction of the way from the boundverts' centroid to the original
        // vertex.
        let original_vertex = co(bm, self.bev(bv).v);
        let fullness = self.pro_spacing.fullness;
        let center_direction = math::sub(original_vertex, boundverts_center);
        let c = if math::len_sq(center_direction) > BEVEL_EPSILON_SQ {
            math::madd(boundverts_center, center_direction, fullness)
        } else {
            boundverts_center
        };
        self.set_mco(vm0, 0, 1, 1, c);
        self.vm_mut(vm0).copy_equiv_verts();

        // Subdivide from the two-segment start mesh to the final vertex mesh.
        let mut vm1 = vm0;
        loop {
            vm1 = self.cubic_subdiv(vm1);
            if self.vm(vm1).seg >= nseg {
                break;
            }
        }
        if self.vm(vm1).seg != nseg {
            vm1 = self.interp_vmesh(vm1, nseg);
        }
        vm1
    }

    /// `snap_to_pipe_profile`: snap `p` to the profile of `vpipe` projected onto the plane
    /// through `p` with normal along `vpipe->ebev`.
    fn snap_to_pipe_profile(&self, bm: &BMesh, vpipe: Bnd, midline: bool, p: V3) -> V3 {
        let pro = &self.bnd(vpipe).profile;
        let e = self.bnd(vpipe).ebev.expect("pipe edge");
        if math::compare_v3v3(pro.start, pro.end, BEVEL_EPSILON_D as f32) {
            return pro.start;
        }
        // A plane with the normal pointing along the beveled edge.
        let [v1, v2] = bm.edge(self.ehe(e)).v;
        let edir = math::sub(co(bm, v1), co(bm, v2));
        let plane = math::plane_from_point_normal_v3(p, edir);
        let start_plane = math::closest_to_plane_v3(plane, pro.start);
        let end_plane = math::closest_to_plane_v3(plane, pro.end);
        let middle_plane = math::closest_to_plane_v3(plane, pro.middle);
        if let Some(m) = make_unit_square_map(start_plane, middle_plane, end_plane)
            && let Some(minv) = math::invert_m4_m4(&m)
        {
            // Transform p and project it onto the superellipse.
            let q = math::mul_v3_m4v3(&minv, p);
            let q = snap_to_superellipsoid(q, pro.super_r, midline);
            return math::mul_v3_m4v3(&m, q);
        }
        // Planar case: just snap to the line start_plane--end_plane.
        math::closest_to_line_segment_v3(p, start_plane, end_plane)
    }

    /// `pipe_test`: do the edges at `bv` form a "pipe"? Returns the boundvert whose ebev is
    /// one of the pipe edges and whose next boundvert has a beveled non-pipe edge.
    pub(crate) fn pipe_test(&self, bm: &BMesh, bv: Bev) -> Option<Bnd> {
        let vm = self.bvm(bv);
        let b = self.bev(bv);
        let count = self.vm(vm).count;
        if !(3..=4).contains(&count) || b.selcount < 3 || b.selcount > 4 {
            return None;
        }
        let bvco = co(bm, b.v);
        // Find v1, v2, v3 all with beveled edges, where v1 and v3 have collinear edges.
        let mut found = None;
        let mut dir1 = [0.0f32; 3];
        for v1 in self.bounds(vm) {
            let v2 = self.bnext(v1);
            let v3 = self.bnext(v2);
            if let (Some(e1), Some(_), Some(e3)) =
                (self.bnd(v1).ebev, self.bnd(v2).ebev, self.bnd(v3).ebev)
            {
                let d1 = math::norm(math::sub(bvco, co(bm, self.other_vert(bm, e1, b.v))));
                let d3 = math::norm(math::sub(co(bm, self.other_vert(bm, e3, b.v)), bvco));
                if math::angle_normalized_v3v3(d1, d3) < bevel_epsilon_ang() {
                    dir1 = d1;
                    found = Some(v1);
                    break;
                }
            }
        }
        let v1 = found?;
        // Check face planes: all should have normals perpendicular to epipe.
        for i in 0..b.edgecount {
            if let Some(f) = self.eh(b.edge(i)).fnext
                && math::dot(dir1, fno(bm, f)).abs() > BEVEL_EPSILON_BIG
            {
                return None;
            }
        }
        Some(v1)
    }

    /// `pipe_adj_vmesh`: an ADJ mesh with interior points snapped to the pipe profile.
    pub(crate) fn pipe_adj_vmesh(&mut self, bm: &BMesh, bv: Bev, vpipe: Bnd) -> Vm {
        let vm = self.adj_vmesh(bm, bv);
        // Now snap all interior coordinates to be on the epipe profile.
        let bvm = self.bvm(bv);
        let n_bndv = self.vm(bvm).count;
        let ns = self.vm(bvm).seg;
        let half_ns = ns / 2;
        let ipipe1 = self.bnd(vpipe).index;
        let ipipe2 = self.bnd(self.bnext(self.bnext(vpipe))).index;
        for i in 0..n_bndv {
            for j in 1..=half_ns {
                for k in 0..=half_ns {
                    if !self.vm(vm).is_canon(i, j, k) {
                        continue;
                    }
                    // For the 'square' profiles and an even nseg, some vertices snap to the
                    // midline of the pipe, not just to one plane or the other.
                    let even = ns % 2 == 0;
                    let midline = even
                        && k == half_ns
                        && ((i == 0 && j == half_ns) || i == ipipe1 || i == ipipe2);
                    let p = self.snap_to_pipe_profile(bm, vpipe, midline, self.mco(vm, i, j, k));
                    self.set_mco(vm, i, j, k, p);
                }
            }
        }
        vm
    }

    /// `square_out_adj_vmesh`: the vmesh for profile 1 and 3 or more beveled edges (parallel
    /// offset lines on each side of the center).
    pub(crate) fn square_out_adj_vmesh(&mut self, bm: &BMesh, bv: Bev) -> Vm {
        let bvm = self.bvm(bv);
        let n_bndv = self.vm(bvm).count;
        let ns = self.vm(bvm).seg;
        let ns2 = ns / 2;
        let odd = ns % 2;
        let mut ns2inv = 1.0 / ns2 as f32;
        let start = self.vm(bvm).boundstart;
        let vm = self.new_adj_vmesh(n_bndv, ns, start);
        let bvv = self.bev(bv).v;
        let bvco = co(bm, bvv);
        let clstride = (ns2 + 1) as usize;
        let mut centerline = vec![[0.0f32; 3]; clstride * n_bndv as usize];
        let mut cset = vec![false; n_bndv as usize];
        let cl = |i: i32| clstride * i as usize;

        // Find on_edge, the place on bndv[i]'s elast where the offset line would meet, taking
        // the min distance to bv->v with where the next sector's offset line would meet.
        let mut bndv = start.expect("vmesh boundary");
        let mut i = 0;
        while i < n_bndv {
            let bndco = self.bnd(bndv).nv.co;
            let e1 = self.bnd(bndv).efirst;
            let e2 = self.bnd(bndv).elast;
            let ang_kind = match (e1, e2) {
                (Some(a), Some(b)) => self.edges_angle_kind(bm, a, b, bvv),
                _ => AngleKind::Straight,
            };
            if self.bnd(bndv).is_patch_start {
                let nx = self.bnext(bndv);
                centerline[cl(i)] = math::mid(self.bnd(bndv).nv.co, self.bnd(nx).nv.co);
                cset[i as usize] = true;
                bndv = nx;
                i += 1;
                let nx = self.bnext(bndv);
                centerline[cl(i)] = math::mid(self.bnd(bndv).nv.co, self.bnd(nx).nv.co);
                cset[i as usize] = true;
                bndv = nx;
                i += 1;
                // Leave cset[i] where it was.
            } else if self.bnd(bndv).is_arc_start {
                centerline[cl(i)] = self.bnd(bndv).profile.middle;
                bndv = self.bnext(bndv);
                cset[i as usize] = true;
                i += 1;
            } else if ang_kind == AngleKind::Smaller {
                let (e1, e2) = (e1.expect("efirst"), e2.expect("elast"));
                let [a1, a2] = bm.edge(self.ehe(e1)).v;
                let [b1, b2] = bm.edge(self.ehe(e2)).v;
                let dir1 = math::sub(co(bm, a1), co(bm, a2));
                let dir2 = math::sub(co(bm, b1), co(bm, b2));
                let co1 = math::add(bndco, dir1);
                let co2 = math::add(bndco, dir2);
                // Intersect e1 with the line through bndv parallel to e2 to get v1co.
                let (ikind, meet1, _) = math::isect_line_line_v3(co(bm, a1), co(bm, a2), bndco, co2);
                let v1co = (ikind != 0).then_some(meet1);
                // Intersect e2 with the line through bndv parallel to e1 to get v2co.
                let (ikind, meet1, _) = math::isect_line_line_v3(co(bm, b1), co(bm, b2), bndco, co1);
                let v2co = (ikind != 0).then_some(meet1);
                // on_edge[i] is the min dist to bv->v of v2co and the next iteration's v1co.
                let iprev = if i == 0 { n_bndv - 1 } else { i - 1 };
                if let Some(v2co) = v2co {
                    if cset[i as usize] {
                        centerline[cl(i)] = closer(centerline[cl(i)], v2co, bvco);
                    } else {
                        centerline[cl(i)] = v2co;
                        cset[i as usize] = true;
                    }
                }
                if let Some(v1co) = v1co {
                    if cset[iprev as usize] {
                        centerline[cl(iprev)] = closer(centerline[cl(iprev)], v1co, bvco);
                    } else {
                        centerline[cl(iprev)] = v1co;
                        cset[iprev as usize] = true;
                    }
                }
            }
            bndv = self.bnext(bndv);
            i += 1;
        }
        // Maybe not everything was set by the previous loop.
        let mut bndv = start.expect("vmesh boundary");
        for i in 0..n_bndv {
            if !cset[i as usize] {
                let nx = self.bnext(bndv);
                let e1 = self.bnd(nx).efirst;
                let co1 = self.bnd(bndv).nv.co;
                let co2 = self.bnd(nx).nv.co;
                if let Some(e1) = e1 {
                    let [a1, a2] = bm.edge(self.ehe(e1)).v;
                    let prev_arc = self.bnd(self.bprev(bndv)).is_arc_start;
                    if prev_arc && self.bnd(nx).is_arc_start {
                        let (ikind, meet1, _) =
                            math::isect_line_line_v3(co(bm, a1), co(bm, a2), co1, co2);
                        if ikind != 0 {
                            centerline[cl(i)] = meet1;
                            cset[i as usize] = true;
                        }
                    } else {
                        let p = if prev_arc { co1 } else { co2 };
                        centerline[cl(i)] = math::closest_to_line_segment_v3(p, co(bm, a1), co(bm, a2));
                        cset[i as usize] = true;
                    }
                }
                if !cset[i as usize] {
                    centerline[cl(i)] = math::mid(co1, co2);
                    cset[i as usize] = true;
                }
            }
            bndv = self.bnext(bndv);
        }

        // Fill in the rest of the center-lines by interpolation.
        let mut co1 = [0.0f32; 3];
        let co2 = bvco;
        let mut bndv = start.expect("vmesh boundary");
        for i in 0..n_bndv {
            if odd == 1 {
                let nx = self.bnext(bndv);
                let ang = 0.5 * math::angle_v3v3v3(self.bnd(bndv).nv.co, co1, self.bnd(nx).nv.co);
                let finalfrac = if ang > bevel_small_ang() {
                    (0.5 / libm::sinf(ang)).min(0.8)
                } else {
                    0.8
                };
                ns2inv = 1.0 / (ns2 as f32 + finalfrac);
            }
            co1 = centerline[cl(i)];
            for j in 1..=ns2 {
                centerline[cl(i) + j as usize] = math::interp(co1, co2, j as f32 * ns2inv);
            }
            bndv = self.bnext(bndv);
        }

        // Coords of edges and the mid or near-mid line.
        let mut bndv = start.expect("vmesh boundary");
        for i in 0..n_bndv {
            let c1 = self.bnd(bndv).nv.co;
            let c2 = centerline[cl(if i == 0 { n_bndv - 1 } else { i - 1 })];
            for j in 0..ns2 + odd {
                self.set_mco(vm, i, j, 0, math::interp(c1, c2, j as f32 * ns2inv));
            }
            let c2 = centerline[cl(i)];
            for k in 1..=ns2 {
                self.set_mco(vm, i, 0, k, math::interp(c1, c2, k as f32 * ns2inv));
            }
            bndv = self.bnext(bndv);
        }
        if odd == 0 {
            self.set_mco(vm, 0, ns2, ns2, bvco);
        }
        self.vm_mut(vm).copy_equiv_verts();

        // Fill in interior points by interpolation from edges to center-lines.
        for i in 0..n_bndv {
            let im1 = if i == 0 { n_bndv - 1 } else { i - 1 };
            for j in 1..ns2 + odd {
                for k in 1..=ns2 {
                    let a = self.mco(vm, i, 0, k);
                    let b = centerline[cl(im1) + k as usize];
                    let (ikind, meet1, meet2) = math::isect_line_line_v3(
                        a,
                        b,
                        self.mco(vm, i, j, 0),
                        centerline[cl(i) + j as usize],
                    );
                    let p = match ikind {
                        // Fall back on interpolation in one direction.
                        0 => math::interp(a, b, j as f32 * ns2inv),
                        1 => meet1,
                        _ => math::mid(meet1, meet2),
                    };
                    self.set_mco(vm, i, j, k, p);
                }
            }
        }
        self.vm_mut(vm).copy_equiv_verts();
        vm
    }
}

/// `closer_v3_v3v3v3`: whichever of `a` and `b` is closer to `v`.
fn closer(a: V3, b: V3, v: V3) -> V3 {
    if math::len_sq_v3v3(a, v) <= math::len_sq_v3v3(b, v) {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sabin_gamma_matches_table_continuation() {
        assert_eq!(sabin_gamma(4), 0.25);
        let g7 = sabin_gamma(7);
        assert!(g7 > 0.52 && g7 < 0.7, "{g7}");
    }

    #[test]
    fn interp_range_edges() {
        let frac = [0.0, 0.25, 0.5, 1.0];
        assert_eq!(interp_range(&frac, 3, 0.0), (0, 0.0));
        let (i, r) = interp_range(&frac, 3, 0.75);
        assert_eq!(i, 2);
        assert!((r - 0.5).abs() < 1e-6);
        assert_eq!(interp_range(&frac, 3, 1.0), (3, 0.0));
    }
}
