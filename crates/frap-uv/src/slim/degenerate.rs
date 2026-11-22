// SPDX-License-Identifier: GPL-3.0-or-later
//! `p_chart_correct_degenerate_triangles` (Degenerate Geometry Fixing of
//! `uv_parametrizer.cc`): before SLIM, the 3D positions of triangles with an area below
//! `min_area` or an angle below `min_angle` are moved (in the parametrizer's copy only) so that
//! the triangle is not degenerate and no neighbour becomes so.
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).

use crate::param::bli::{self, M3, V3};
use crate::param::{NIL, PFACE_DONE, ParamMesh};

/// `CORR_ZERO_AREA_EPS`.
const CORR_ZERO_AREA_EPS: f32 = 1.0e-10;

/// `cos_v3v3v3(p1, p2, p3)`: the cosine of the angle at `p2`.
fn cos_v3v3v3(p1: V3, p2: V3, p3: V3) -> f32 {
    let (v1, _) = bli::normalize3(bli::sub3(p2, p1));
    let (v2, _) = bli::normalize3(bli::sub3(p2, p3));
    bli::dot3(v1, v2)
}

/// `p_triangle_cos`.
fn triangle_cos(v1: V3, v2: V3, v3: V3) -> [f32; 3] {
    [
        cos_v3v3v3(v3, v1, v2),
        cos_v3v3v3(v1, v2, v3),
        cos_v3v3v3(v2, v3, v1),
    ]
}

fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul3(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `p_edge_matrix(R, edge_dir)`.
fn edge_matrix(edge_dir: V3) -> Option<M3> {
    let eps = 1.0e-5f32;
    let n1 = [0.0f32, 0.0, 1.0];
    let n2 = [0.0f32, 1.0, 0.0];
    let edge_len = bli::len3(edge_dir);
    if edge_len < eps {
        return None;
    }
    let edge_dir_norm = mul3(edge_dir, 1.0 / edge_len);
    let mut normal_dir = bli::cross3(edge_dir_norm, n1);
    let mut normal_len = bli::len3(normal_dir);
    if normal_len < eps {
        normal_dir = bli::cross3(edge_dir_norm, n2);
        normal_len = bli::len3(normal_dir);
        if normal_len < eps {
            return None;
        }
    }
    let normal_dir = mul3(normal_dir, 1.0 / normal_len);
    let tangent_dir = bli::cross3(edge_dir_norm, normal_dir);
    // `R[i][0] = edge_dir_norm[i]`, `R[i][1] = normal_dir[i]`, `R[i][2] = tangent_dir[i]`.
    let mut r = [[0.0f32; 3]; 3];
    for i in 0..3 {
        r[i][0] = edge_dir_norm[i];
        r[i][1] = normal_dir[i];
        r[i][2] = tangent_dir[i];
    }
    Some(r)
}

impl ParamMesh {
    fn co(&self, v: u32) -> V3 {
        self.verts[v as usize].co
    }

    fn evert(&self, e: u32) -> u32 {
        self.edges[e as usize].vert
    }

    fn enext(&self, e: u32) -> u32 {
        self.edges[e as usize].next
    }

    fn face_done(&self, e: u32) -> bool {
        let f = self.edges[e as usize].face;
        self.faces[f as usize].flag & PFACE_DONE != 0
    }

    /// `p_validate_corrected_coords_point`.
    fn validate_corrected_coords_point(
        &self,
        corr_e: u32,
        corr_co1: V3,
        corr_co2: V3,
        min_area: f32,
        min_angle_cos: f32,
    ) -> bool {
        let corr_v = self.evert(corr_e);
        let start = self.verts[corr_v as usize].edge;
        let corr_pair = self.edges[corr_e as usize].pair;
        let mut e = start;
        loop {
            if e != corr_e && self.face_done(e) {
                let en = self.enext(e);
                let enn = self.enext(en);
                let (area, f_cos) = if enn == corr_pair {
                    let other = self.co(self.evert(enn));
                    (
                        bli::area_tri_v3(corr_co1, corr_co2, other),
                        triangle_cos(corr_co1, corr_co2, other),
                    )
                } else {
                    let o1 = self.co(self.evert(en));
                    let o2 = self.co(self.evert(enn));
                    (
                        bli::area_tri_v3(corr_co1, o1, o2),
                        triangle_cos(corr_co1, o1, o2),
                    )
                };
                if area < min_area {
                    return false;
                }
                if f_cos.iter().any(|&c| c > min_angle_cos) {
                    return false;
                }
            }
            e = self.wheel_edge_next(e);
            if e == NIL || e == start {
                break;
            }
        }
        true
    }

    /// `p_validate_corrected_coords`.
    fn validate_corrected_coords(
        &self,
        corr_e: u32,
        corr_co: V3,
        min_area: f32,
        min_angle_cos: f32,
    ) -> bool {
        let corr_v = self.evert(corr_e);
        let start = self.verts[corr_v as usize].edge;
        let mut e = start;
        loop {
            if self.face_done(e) || e == corr_e {
                let en = self.enext(e);
                let o1 = self.co(self.evert(en));
                let o2 = self.co(self.evert(self.enext(en)));
                let area = bli::area_tri_v3(corr_co, o1, o2);
                if area < min_area {
                    return false;
                }
                let f_cos = triangle_cos(corr_co, o1, o2);
                if f_cos.iter().any(|&c| c > min_angle_cos) {
                    return false;
                }
            }
            e = self.wheel_edge_next(e);
            if e == NIL || e == start {
                break;
            }
        }
        true
    }

    /// `p_chart_correct_degenerate_triangle_point`.
    fn correct_degenerate_triangle_point(
        &mut self,
        f: u32,
        min_area: f32,
        min_angle_cos: f32,
    ) -> bool {
        const REF_EDGES: [V3; 12] = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 0.0],
            [0.0, 0.5, 1.0],
            [0.5, 0.0, 1.0],
            [0.5, 1.0, 0.0],
            [0.0, 1.0, 0.5],
            [1.0, 0.0, 0.5],
            [1.0, 0.5, 0.0],
        ];
        const LEN_MULTIPLIER_COUNT: i32 = 3;
        let mut corr_co_found = false;
        let corr_len = 2.0f32 * ((min_area + CORR_ZERO_AREA_EPS) / 3.0f32 / 3.0f32.sqrt()).sqrt();
        let fe = self.faces[f as usize].edge;
        'outer: for m in 0..LEN_MULTIPLIER_COUNT {
            for re in REF_EDGES {
                let Some(mm) = edge_matrix(re) else {
                    continue;
                };
                let mut corr_co = [[0.0f32; 3]; 3];
                let mut e = fe;
                for (i, cc) in corr_co.iter_mut().enumerate() {
                    let angle = (i as f32 / 3.0f32) * (2.0 * core::f64::consts::PI) as f32;
                    // C++ `cos(float)`: `cosf`.
                    let corr_dir = [0.0f32, libm::cosf(angle), libm::sinf(angle)];
                    let corr_len_multiplied = corr_len * (m + 1) as f32;
                    let corr_dir = mul3(bli::mul_m3_v3(&mm, corr_dir), corr_len_multiplied);
                    *cc = add3(self.co(self.evert(e)), corr_dir);
                    e = self.enext(e);
                }
                let mut e = fe;
                for i in 0..3 {
                    if !self.validate_corrected_coords_point(
                        e,
                        corr_co[i],
                        corr_co[(i + 1) % 3],
                        min_area,
                        min_angle_cos,
                    ) {
                        return false;
                    }
                    e = self.enext(e);
                }
                let mut e = fe;
                for cc in corr_co {
                    let v = self.evert(e);
                    self.verts[v as usize].co = cc;
                    e = self.enext(e);
                }
                corr_co_found = true;
                break 'outer;
            }
        }
        if !corr_co_found {
            return false;
        }
        self.faces[f as usize].flag |= PFACE_DONE;
        true
    }

    /// `p_chart_correct_degenerate_triangles2`.
    fn correct_degenerate_triangles2(&mut self, c: usize, min_area: f32, min_angle: f32) {
        let eps = 1.0e-6f32;
        let min_angle_cos = libm::cosf(min_angle);
        let min_angle_sin = libm::sinf(min_angle + CORR_ZERO_AREA_EPS);
        for f in self.chart_face_list(c) {
            if self.faces[f as usize].flag & PFACE_DONE != 0 {
                continue;
            }
            let face_area = self.face_area(f);
            let mut max_edge = NIL;
            let mut max_edge_len = f32::NEG_INFINITY;
            let mut min_edge = NIL;
            let mut min_edge_len = f32::INFINITY;
            let mut middle_edge = NIL;
            let fe = self.faces[f as usize].edge;
            let mut e = fe;
            loop {
                let len = self.edge_length(e);
                if len > max_edge_len {
                    max_edge = e;
                    max_edge_len = len;
                    middle_edge = if self.enext(max_edge) == min_edge {
                        self.enext(min_edge)
                    } else {
                        self.enext(max_edge)
                    };
                }
                if len < min_edge_len {
                    min_edge = e;
                    min_edge_len = len;
                    middle_edge = if self.enext(min_edge) == max_edge {
                        self.enext(max_edge)
                    } else {
                        self.enext(min_edge)
                    };
                }
                e = self.enext(e);
                if e == fe {
                    break;
                }
            }
            let small_uniside_tri = face_area <= min_area && min_edge == max_edge;
            if max_edge_len < eps || small_uniside_tri {
                let _ = self.correct_degenerate_triangle_point(f, min_area, min_angle_cos);
                continue;
            }
            if min_edge == max_edge {
                self.faces[f as usize].flag |= PFACE_DONE;
                continue;
            }
            let ed = bli::sub3(
                self.co(self.evert(self.enext(max_edge))),
                self.co(self.evert(max_edge)),
            );
            let Some(mm) = edge_matrix(ed) else {
                continue;
            };
            let max_face_cos = if self.enext(middle_edge) == max_edge {
                cos_v3v3v3(
                    self.co(self.evert(middle_edge)),
                    self.co(self.evert(max_edge)),
                    self.co(self.evert(min_edge)),
                )
            } else {
                cos_v3v3v3(
                    self.co(self.evert(max_edge)),
                    self.co(self.evert(middle_edge)),
                    self.co(self.evert(min_edge)),
                )
            };
            let angle_corr_len = if max_face_cos > min_angle_cos {
                self.edge_length(middle_edge) * min_angle_sin
            } else {
                0.0
            };
            if face_area > min_area && angle_corr_len == 0.0 {
                self.faces[f as usize].flag |= PFACE_DONE;
                continue;
            }
            let mut corr_len = (min_area + CORR_ZERO_AREA_EPS) * 2.0 / max_edge_len;
            corr_len = corr_len.max(angle_corr_len);
            let corr_e = self.enext(self.enext(max_edge));
            let corr_v = self.evert(corr_e);
            const DIR_COUNT: i32 = 16;
            const LEN_MULTIPLIER_COUNT: i32 = 2;
            let mut found = None;
            'search: for m in 0..LEN_MULTIPLIER_COUNT {
                for d in 0..DIR_COUNT {
                    let angle =
                        (d as f32 / DIR_COUNT as f32) * (2.0 * core::f64::consts::PI) as f32;
                    // C++ `cos(float)`: `cosf`.
                    let corr_dir = [0.0f32, libm::cosf(angle), libm::sinf(angle)];
                    let corr_len_multiplied = corr_len * (m + 1) as f32;
                    let corr_dir = mul3(bli::mul_m3_v3(&mm, corr_dir), corr_len_multiplied);
                    let corr_co = add3(self.co(corr_v), corr_dir);
                    if self.validate_corrected_coords(corr_e, corr_co, min_area, min_angle_cos) {
                        found = Some(corr_co);
                        break 'search;
                    }
                }
            }
            let Some(corr_co) = found else {
                continue;
            };
            self.faces[f as usize].flag |= PFACE_DONE;
            self.verts[corr_v as usize].co = corr_co;
        }
    }

    /// `p_chart_correct_degenerate_triangles`: whether every triangle could be corrected
    /// (Blender ignores the result).
    pub(crate) fn chart_correct_degenerate_triangles(
        &mut self,
        c: usize,
        min_area: f32,
        min_angle: f32,
    ) -> bool {
        self.correct_degenerate_triangles2(c, min_area, min_angle);
        let mut ret = true;
        for f in self.chart_face_list(c) {
            if self.faces[f as usize].flag & PFACE_DONE == 0 {
                ret = false;
            }
            self.faces[f as usize].flag &= !PFACE_DONE;
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_matrix_is_orthonormal() {
        let r = edge_matrix([1.0, 2.0, 0.5]).unwrap();
        // Columns of the C array (`r[col]`) are unit vectors here by construction of rows:
        // each `r[i]` holds the i-th components of the three basis vectors.
        let basis: Vec<V3> = (0..3).map(|k| [r[0][k], r[1][k], r[2][k]]).collect();
        for a in 0..3 {
            for b in 0..3 {
                let d = bli::dot3(basis[a], basis[b]);
                let e = if a == b { 1.0 } else { 0.0 };
                assert!((d - e).abs() < 1e-5, "{a} {b} {d}");
            }
        }
        assert!(edge_matrix([0.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn cosine_at_middle_vertex() {
        let c = cos_v3v3v3([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!(c.abs() < 1e-7);
    }
}
