// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimum-area enclosing rectangle of a chart (`p_chart_convex_hull`,
//! `p_chart_minimum_area_angle` with rotating calipers) and the rotations built on it
//! (`p_chart_rotate_minimum_area`, `p_chart_rotate_fit_aabb`).
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc (Blender 5.2.4).

use super::bli::{self, V2};
use super::{NIL, ParamMesh};

/// `p_area_signed`.
#[inline]
fn area_signed(v1: V2, v2: V2, v3: V2) -> f32 {
    0.5 * (((v2[0] - v1[0]) * (v3[1] - v1[1])) - ((v3[0] - v1[0]) * (v2[1] - v1[1])))
}

/// `p_intersect_line_2d_dir`.
fn intersect_line_2d_dir(v1: V2, dir1: V2, v2: V2, dir2: V2) -> Option<V2> {
    let div = dir2[0] * dir1[1] - dir2[1] * dir1[0];
    if div == 0.0 {
        return None;
    }
    let lmbda = ((v1[1] - v2[1]) * dir1[0] - (v1[0] - v2[0]) * dir1[1]) / div;
    Some([v1[0] + lmbda * dir2[0], v1[1] + lmbda * dir2[1]])
}

/// `p_rectangle_area`.
fn rectangle_area(p1: V2, dir: V2, p2: V2, p3: V2, p4: V2) -> f32 {
    let orthodir = [dir[1], -dir[0]];
    let Some(corner1) = intersect_line_2d_dir(p1, dir, p2, orthodir) else {
        return 1e10;
    };
    let Some(corner2) = intersect_line_2d_dir(p1, dir, p4, orthodir) else {
        return 1e10;
    };
    let Some(corner3) = intersect_line_2d_dir(p3, dir, p4, orthodir) else {
        return 1e10;
    };
    bli::len_v2v2(corner1, corner2) * bli::len_v2v2(corner2, corner3)
}

impl ParamMesh {
    /// `p_chart_convex_hull` (Graham / Andrew over the outer boundary): the hull vertex UVs
    /// and the index of the right-most point.
    fn chart_convex_hull(&mut self, c: usize) -> Option<(Vec<V2>, usize)> {
        let be = self.chart_boundaries(c);
        if be == NIL {
            return None;
        }
        let mut points: Vec<V2> = Vec::new();
        let mut e = be;
        loop {
            points.push(self.verts[self.edges[e as usize].vert as usize].uv);
            e = self.boundary_edge_next(e);
            if e == be {
                break;
            }
        }
        // `qsort` with `p_compare_geometric_uv` (u, then v); equal points are interchangeable.
        points.sort_by(|a, b| {
            a[0].partial_cmp(&b[0])
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(
                    a[1].partial_cmp(&b[1])
                        .unwrap_or(core::cmp::Ordering::Equal),
                )
        });
        let mut u: Vec<V2> = Vec::with_capacity(points.len());
        let mut l: Vec<V2> = Vec::with_capacity(points.len());
        for &p in &points {
            while u.len() > 1 && area_signed(u[u.len() - 2], p, u[u.len() - 1]) <= 0.0 {
                u.pop();
            }
            while l.len() > 1 && area_signed(l[l.len() - 2], p, l[l.len() - 1]) >= 0.0 {
                l.pop();
            }
            u.push(p);
            l.push(p);
        }
        let right = u.len() - 1;
        let mut out = u;
        // The first and last point of L are left out, they are also in U.
        let mut i = l.len() as isize - 2;
        while i > 0 {
            out.push(l[i as usize]);
            i -= 1;
        }
        Some((out, right))
    }

    /// `p_chart_minimum_area_angle`.
    pub(crate) fn chart_minimum_area_angle(&mut self, c: usize) -> f32 {
        let Some((points, right)) = self.chart_convex_hull(c) else {
            return 0.0;
        };
        let npoints = points.len();
        let mut angles = vec![0.0f32; npoints];
        let mut i_min = 0usize;
        let mut i_max = 0usize;
        let mut miny = 1e10f32;
        let mut maxy = -1e10f32;
        for i in 0..npoints {
            let p1 = if i == 0 {
                points[npoints - 1]
            } else {
                points[i - 1]
            };
            let p2 = points[i];
            let p3 = if i == npoints - 1 {
                points[0]
            } else {
                points[i + 1]
            };
            angles[i] = core::f32::consts::PI - bli::angle_v2v2v2(p1, p2, p3);
            if points[i][1] < miny {
                miny = points[i][1];
                i_min = i;
            }
            if points[i][1] > maxy {
                maxy = points[i][1];
                i_max = i;
            }
        }
        // Left, top, right, bottom.
        let mut idx = [0usize, i_max, right, i_min];
        let mut a = [0.0f32; 4];
        let offs: [V2; 4] = [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0]];
        for k in 0..4 {
            let p = points[idx[k]];
            let v = [p[0] + offs[k][0], p[1] + offs[k][1]];
            a[k] = bli::angle_v2v2v2(points[(idx[k] + 1) % npoints], p, v);
        }

        let mut rotated = 0.0f32;
        let mut minarea = 1e10f32;
        let mut minangle = 0.0f32;
        while rotated <= core::f32::consts::FRAC_PI_2 {
            frap_math::budget::check();
            // Rotate with the smallest angle.
            let mut i_min = 0usize;
            let mut mina = 1e10f32;
            for (i, &ai) in a.iter().enumerate() {
                if ai < mina {
                    mina = ai;
                    i_min = i;
                }
            }
            rotated += mina;
            let nextidx = (idx[i_min] + 1) % npoints;
            a[i_min] = angles[nextidx];
            a[(i_min + 1) % 4] -= mina;
            a[(i_min + 2) % 4] -= mina;
            a[(i_min + 3) % 4] -= mina;

            let p1 = points[idx[i_min]];
            let p1n = points[nextidx];
            let p2 = points[idx[(i_min + 1) % 4]];
            let p3 = points[idx[(i_min + 2) % 4]];
            let p4 = points[idx[(i_min + 3) % 4]];
            let len = bli::len_v2v2(p1, p1n);
            if len > 0.0 {
                let inv = 1.0 / len;
                let v = [(p1n[0] - p1[0]) * inv, (p1n[1] - p1[1]) * inv];
                let area = rectangle_area(p1, v, p2, p3, p4);
                if area < minarea {
                    minarea = area;
                    minangle = rotated;
                }
            }
            idx[i_min] = nextidx;
        }
        // Keep the rotation as small as possible.
        if minangle > core::f32::consts::FRAC_PI_4 {
            minangle -= core::f32::consts::FRAC_PI_2;
        }
        minangle
    }

    /// `p_chart_rotate_minimum_area`.
    pub(crate) fn chart_rotate_minimum_area(&mut self, c: usize) {
        let angle = self.chart_minimum_area_angle(c);
        let sine = libm::sinf(angle);
        let cosine = libm::cosf(angle);
        self.chart_uv_map(c, |uv| {
            [cosine * uv[0] - sine * uv[1], sine * uv[0] + cosine * uv[1]]
        });
    }

    /// `p_chart_rotate_fit_aabb`.
    pub(crate) fn chart_rotate_fit_aabb(&mut self, c: usize) {
        let points: Vec<V2> = self
            .chart_vert_list(c)
            .iter()
            .map(|&v| self.verts[v as usize].uv)
            .collect();
        let angle = crate::pack::convexhull::convexhull_aabb_fit_points_2d(&points);
        if angle != 0.0 {
            let m = bli::angle_to_mat2(angle);
            self.chart_uv_transform(c, &m);
        }
    }
}
