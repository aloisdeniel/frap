// SPDX-License-Identifier: GPL-3.0-or-later
//! Island scale averaging (`uv_parametrizer_average`: 3D area vs UV area per chart, optional
//! non-uniform scale and shear correction), used by `uv.average_islands_scale` and
//! `uv.unwrap` (`average(ignore_pinned=true, false, false)` before packing).
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).

use crate::param::ParamMesh;
use crate::param::bli::{self, M2};

impl ParamMesh {
    /// `uv_parametrizer_average(ignore_pinned, scale_uv, shear)`: scales every chart so its
    /// UV area is proportional to its 3D area (keeping the bounding box centre), after an
    /// optional correction of non-uniform scale (`scale_uv`) and shear (`shear`).
    pub fn average(&mut self, ignore_pinned: bool, scale_uv: bool, shear: bool) {
        if self.charts.is_empty() {
            return;
        }
        let mut tot_area_3d = 0.0f32;
        let mut tot_area_uv = 0.0f32;
        for c in 0..self.charts.len() {
            if ignore_pinned && self.charts[c].has_pins {
                continue;
            }
            // Store the original bounding box midpoint.
            let (minv, maxv) = self.chart_uv_bbox(c);
            self.charts[c].origin = [(minv[0] + maxv[0]) * 0.5, (minv[1] + maxv[1]) * 0.5];

            if scale_uv || shear {
                let faces = self.chart_face_list(c);
                let max_iter = 10;
                for _ in 0..max_iter {
                    frap_math::budget::check();
                    let mut scale_cou = 0.0f64;
                    let mut scale_cov = 0.0f64;
                    let mut scale_cross = 0.0f64;
                    let mut weight_sum = 0.0f64;
                    for &f in &faces {
                        let [a, b, cc] = self.face_verts(f);
                        let (va, vb, vc) = (
                            &self.verts[a as usize],
                            &self.verts[b as usize],
                            &self.verts[cc as usize],
                        );
                        let s: M2 = [
                            [va.uv[0] - vc.uv[0], va.uv[1] - vc.uv[1]],
                            [vb.uv[0] - vc.uv[0], vb.uv[1] - vc.uv[1]],
                        ];
                        // Find the "U" and "V" axes in triangle coordinates.
                        let Some(m) = bli::invert_m2_m2(&s) else {
                            continue;
                        };
                        let mut cou = [0.0f32; 3];
                        let mut cov = [0.0f32; 3];
                        for k in 0..3 {
                            cou[k] =
                                m[0][0] * (va.co[k] - vc.co[k]) + m[0][1] * (vb.co[k] - vc.co[k]);
                            cov[k] =
                                m[1][0] * (va.co[k] - vc.co[k]) + m[1][1] * (vb.co[k] - vc.co[k]);
                        }
                        let weight = self.face_area(f);
                        scale_cou += f64::from(bli::len3(cou) * weight);
                        scale_cov += f64::from(bli::len3(cov) * weight);
                        if shear {
                            let (cov_n, _) = bli::normalize3(cov);
                            let (cou_n, _) = bli::normalize3(cou);
                            scale_cross += f64::from(bli::dot3(cou_n, cov_n) * weight);
                        }
                        weight_sum += f64::from(weight);
                    }
                    if scale_cou * scale_cov < f64::from(1e-10f32) {
                        break;
                    }
                    let scale_factor_u = if scale_uv {
                        ((scale_cou / scale_cov) as f32).sqrt()
                    } else {
                        1.0
                    };
                    // Correction transform (column-major).
                    let t: M2 = [
                        [scale_factor_u, 0.0],
                        [
                            ((scale_cross / weight_sum) as f32).clamp(-0.5, 0.5),
                            1.0 / scale_factor_u,
                        ],
                    ];
                    self.chart_uv_transform(c, &t);
                    let err = (t[0][0] - 1.0).abs()
                        + t[1][0].abs()
                        + t[0][1].abs()
                        + (t[1][1] - 1.0).abs();
                    let tolerance = 1e-6f32;
                    if err < tolerance {
                        break;
                    }
                }
            }

            let mut area_3d = 0.0f32;
            let mut area_uv = 0.0f32;
            let mut f = self.charts[c].faces;
            while f != crate::param::NIL {
                area_3d += self.face_area(f);
                area_uv += self.face_uv_area_signed(f).abs();
                f = self.faces[f as usize].nextlink;
            }
            self.charts[c].area_3d = area_3d;
            self.charts[c].area_uv = area_uv;
            tot_area_3d += area_3d;
            tot_area_uv += area_uv;
        }

        if tot_area_3d == 0.0 || tot_area_uv == 0.0 {
            // Prevent divide by zero.
            return;
        }
        let tot_fac = tot_area_3d / tot_area_uv;
        for c in 0..self.charts.len() {
            if ignore_pinned && self.charts[c].has_pins {
                continue;
            }
            let (a3, auv) = (self.charts[c].area_3d, self.charts[c].area_uv);
            if a3 != 0.0 && auv != 0.0 {
                let fac = a3 / auv;
                // Average scale.
                self.chart_uv_scale(c, (fac / tot_fac).sqrt());
                // Move back to the original midpoint.
                let (minv, maxv) = self.chart_uv_bbox(c);
                let mid = [(minv[0] + maxv[0]) * 0.5, (minv[1] + maxv[1]) * 0.5];
                let o = self.charts[c].origin;
                self.chart_uv_translate(c, [o[0] - mid[0], o[1] - mid[1]]);
            }
        }
    }
}
