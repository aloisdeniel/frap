// SPDX-License-Identifier: GPL-3.0-or-later
//! One generator per primitive (ARCHITECTURE §6.1, rule 2.2.1): `primitives/<name>.rs`, included
//! with `automod`, so adding a generator needs no `mod` edit. Callers name the file module
//! (`primitives::uv_sphere::uv_sphere(…)`); the M0 re-export of `cube` below stays for the
//! existing call sites.
//!
//! Every generator takes the **operator's** properties (Blender's names, defaults in
//! `Default`) plus the creation matrix the operator passes to its `bmesh.ops.create_*` call
//! (`matrix`, Blender layout `m[col][row]`), and returns the mesh exactly as
//! `mesh.primitive_*_add` leaves it in object mode: Blender's vertex, edge, face and corner
//! order, positions computed with Blender's float operations, a `UVMap` when `calc_uvs`, every
//! element selected (`.select_*`), flat faces (`sharp_face`), and the 5.x UV-selection
//! attributes. Verified against `spec/blender-5.2/primitives/*.json`
//! (`tests/primitives_vs_blender.rs`).
//!
//! **The creation matrix** (WP 1.19): in object mode Blender adds the object at
//! `location`/`rotation` and passes `ED_object_new_primitive_matrix(…)`: the inverse of the new
//! object's rotation applied to the operator rotation (≈ identity up to float noise) with the
//! operator `scale` folded in (`rescale_m4`) for the operators that pass a scale (cube,
//! cylinder, cone, UV sphere, ico sphere). Plane, circle, grid and monkey ignore `scale`
//! (monkey multiplies the 3×3 part by `size / 2` itself); the torus is a Python operator whose
//! mesh never sees the matrix. [`creation_matrix`] computes it.

automod::dir!(pub "src/primitives");

pub(crate) mod bm;

pub use cube::cube;

/// Default UV map name (`DATA_("UVMap")`).
pub const UV_MAP_NAME: &str = "UVMap";

/// A Blender `float[4][4]` (`m[col][row]`, translation in `m[3]`).
pub type Matrix4 = [[f32; 4]; 4];

/// The identity matrix.
pub const IDENTITY: Matrix4 = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `rescale_m4(unit, scale)`: the creation matrix of an object-mode add with no rotation.
pub fn scale_matrix(scale: [f32; 3]) -> Matrix4 {
    let mut m = IDENTITY;
    for (col, s) in m.iter_mut().zip(scale) {
        for x in col.iter_mut().take(3) {
            *x *= s;
        }
    }
    m
}

/// Euler XYZ (radians) → 3×3, `eul_to_mat3` (`blender/source/blender/blenlib/intern/
/// math_rotation_c.cc`), as `m[col][row]`.
fn eul_to_mat3(eul: [f32; 3]) -> [[f32; 3]; 3] {
    // `cos(eul[0])` of a `float` argument is C++'s `cos(float)` overload: `cosf`, widened.
    let (ci, cj, ch) = (
        f64::from(libm::cosf(eul[0])),
        f64::from(libm::cosf(eul[1])),
        f64::from(libm::cosf(eul[2])),
    );
    let (si, sj, sh) = (
        f64::from(libm::sinf(eul[0])),
        f64::from(libm::sinf(eul[1])),
        f64::from(libm::sinf(eul[2])),
    );
    let (cc, cs) = (ci * ch, ci * sh);
    let (sc, ss) = (si * ch, si * sh);
    [
        [(cj * ch) as f32, (cj * sh) as f32, (-sj) as f32],
        [
            (sj * sc - cs) as f32,
            (sj * ss + cc) as f32,
            (cj * si) as f32,
        ],
        [
            (sj * cc + ss) as f32,
            (sj * cs - sc) as f32,
            (cj * ci) as f32,
        ],
    ]
}

fn mul_m3_m3m3(a: &[[f32; 3]; 3], b: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut r = [[0.0f32; 3]; 3];
    for j in 0..3 {
        for k in 0..3 {
            r[j][k] = b[j][0] * a[0][k] + b[j][1] * a[1][k] + b[j][2] * a[2][k];
        }
    }
    r
}

/// `invert_m3_m3` (adjoint / determinant, Blender's order; zero matrix when singular).
fn invert_m3(m: &[[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let adj = [
        [
            m[1][1] * m[2][2] - m[1][2] * m[2][1],
            -m[0][1] * m[2][2] + m[0][2] * m[2][1],
            m[0][1] * m[1][2] - m[0][2] * m[1][1],
        ],
        [
            -m[1][0] * m[2][2] + m[1][2] * m[2][0],
            m[0][0] * m[2][2] - m[0][2] * m[2][0],
            -m[0][0] * m[1][2] + m[0][2] * m[1][0],
        ],
        [
            m[1][0] * m[2][1] - m[1][1] * m[2][0],
            -m[0][0] * m[2][1] + m[0][1] * m[2][0],
            m[0][0] * m[1][1] - m[0][1] * m[1][0],
        ],
    ];
    let det = m[0][0] * adj[0][0] + m[1][0] * adj[0][1] + m[2][0] * adj[0][2];
    if det == 0.0 {
        return [[0.0; 3]; 3];
    }
    // `det = 1.0f / det; inverse[a][b] *= det`.
    let inv = 1.0 / det;
    let mut r = adj;
    for row in r.iter_mut() {
        for x in row.iter_mut() {
            *x *= inv;
        }
    }
    r
}

/// `ED_object_new_primitive_matrix(C, obedit, loc, rot, scale, r_primmat)` for an object just
/// added at `loc`/`rot` (`obmat` = its `object_to_world`): the matrix the `create_*` ops get.
/// `scale` is `None` for operators that do not pass one.
pub fn creation_matrix(
    obmat: &Matrix4,
    loc: [f32; 3],
    rot: [f32; 3],
    scale: Option<[f32; 3]>,
) -> Matrix4 {
    let mut rmat = eul_to_mat3(rot);
    rmat = invert_m3(&rmat);
    let mat: [[f32; 3]; 3] = [
        [obmat[0][0], obmat[0][1], obmat[0][2]],
        [obmat[1][0], obmat[1][1], obmat[1][2]],
        [obmat[2][0], obmat[2][1], obmat[2][2]],
    ];
    let cmat = mul_m3_m3m3(&rmat, &mat);
    let imat = invert_m3(&cmat);
    let mut r = IDENTITY;
    for c in 0..3 {
        for k in 0..3 {
            r[c][k] = imat[c][k];
        }
    }
    let mut t = [
        loc[0] - obmat[3][0],
        loc[1] - obmat[3][1],
        loc[2] - obmat[3][2],
    ];
    let imat2 = invert_m3(&mat);
    t = [
        imat2[0][0] * t[0] + imat2[1][0] * t[1] + imat2[2][0] * t[2],
        imat2[0][1] * t[0] + imat2[1][1] * t[1] + imat2[2][1] * t[2],
        imat2[0][2] * t[0] + imat2[1][2] * t[1] + imat2[2][2] * t[2],
    ];
    r[3][0] = t[0];
    r[3][1] = t[1];
    r[3][2] = t[2];
    if let Some(s) = scale {
        for (col, sc) in r.iter_mut().zip(s) {
            for x in col.iter_mut().take(3) {
                *x *= sc;
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_matrix_identity_without_rotation() {
        let mut ob = IDENTITY;
        ob[3] = [1.0, 2.0, 3.0, 1.0];
        let m = creation_matrix(&ob, [1.0, 2.0, 3.0], [0.0; 3], Some([1.0, 2.0, 3.0]));
        assert_eq!(m, scale_matrix([1.0, 2.0, 3.0]));
    }
}

/// Element counts a primitive will create, computed from its properties before building
/// (`--max-elements` pre-check, ARCHITECTURE §12; `frap_math::budget::reserve_corners`). Exact
/// for every primitive except the cone, where it is an upper bound (a zero radius merges a ring
/// into one vertex). `None` from an `estimate()` means the counts overflow `u64`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Estimate {
    /// Vertices.
    pub verts: u64,
    /// Edges.
    pub edges: u64,
    /// Faces.
    pub faces: u64,
    /// Face corners.
    pub corners: u64,
}

impl Estimate {
    /// The largest per-domain count.
    pub fn max_domain(&self) -> u64 {
        self.verts.max(self.edges).max(self.faces).max(self.corners)
    }
}

/// `Some(Estimate)` from checked `u64` arithmetic.
pub(crate) fn estimate(
    verts: Option<u64>,
    edges: Option<u64>,
    faces: Option<u64>,
    corners: Option<u64>,
) -> Option<Estimate> {
    Some(Estimate {
        verts: verts?,
        edges: edges?,
        faces: faces?,
        corners: corners?,
    })
}

/// Counts of the side ring of a cylinder/cone with `n` segments and `fill` caps.
pub(crate) fn tube_estimate(n: i32, fill: cone::FillType) -> Option<Estimate> {
    let n = u64::try_from(n.max(0)).ok()?;
    let (cv, ce, cf, cc) = match fill {
        cone::FillType::Nothing => (0, 0, 0, 0),
        cone::FillType::Ngon => (0, 0, 2, n.checked_mul(2)?),
        cone::FillType::Trifan => (2, n.checked_mul(2)?, n.checked_mul(2)?, n.checked_mul(6)?),
    };
    estimate(
        n.checked_mul(2)?.checked_add(cv),
        n.checked_mul(3)?.checked_add(ce),
        n.checked_add(cf),
        n.checked_mul(4)?.checked_add(cc),
    )
}

#[cfg(test)]
mod estimate_tests {
    use super::cone::FillType;
    use super::*;

    fn counts(m: &crate::Mesh) -> Estimate {
        Estimate {
            verts: m.verts_num() as u64,
            edges: m.edges_num() as u64,
            faces: m.faces_num() as u64,
            corners: m.corners_num() as u64,
        }
    }

    #[test]
    fn estimates_equal_the_generated_counts() {
        for (x, y) in [(1, 1), (3, 2), (10, 10), (0, 5)] {
            let p = grid::Grid {
                x_subdivisions: x,
                y_subdivisions: y,
                ..Default::default()
            };
            assert_eq!(p.estimate(), Some(counts(&grid::grid(&p))), "grid {x}x{y}");
        }
        for fill in [FillType::Nothing, FillType::Ngon, FillType::Trifan] {
            for n in [3, 8, 32] {
                let c = circle::Circle {
                    vertices: n,
                    fill_type: fill,
                    ..Default::default()
                };
                assert_eq!(
                    c.estimate(),
                    Some(counts(&circle::circle(&c))),
                    "circle {n} {fill:?}"
                );
                let c = cylinder::Cylinder {
                    vertices: n,
                    end_fill_type: fill,
                    ..Default::default()
                };
                assert_eq!(
                    c.estimate(),
                    Some(counts(&cylinder::cylinder(&c))),
                    "cyl {n} {fill:?}"
                );
                for (r1, r2) in [(1.0, 0.0), (1.0, 0.5), (0.0, 0.0)] {
                    let c = cone::Cone {
                        vertices: n,
                        radius1: r1,
                        radius2: r2,
                        end_fill_type: fill,
                        ..Default::default()
                    };
                    let e = c.estimate().expect("small");
                    let m = counts(&cone::cone(&c));
                    assert!(
                        m.verts <= e.verts
                            && m.edges <= e.edges
                            && m.faces <= e.faces
                            && m.corners <= e.corners,
                        "cone {n} {fill:?} {r1} {r2}: {m:?} > {e:?}"
                    );
                }
            }
        }
        for (s, r) in [(3, 3), (32, 16), (5, 7)] {
            let p = uv_sphere::UvSphere {
                segments: s,
                ring_count: r,
                ..Default::default()
            };
            assert_eq!(
                p.estimate(),
                Some(counts(&uv_sphere::uv_sphere(&p))),
                "uv {s}x{r}"
            );
        }
        for k in 1..=4 {
            let p = ico_sphere::IcoSphere {
                subdivisions: k,
                ..Default::default()
            };
            assert_eq!(
                p.estimate(),
                Some(counts(&ico_sphere::ico_sphere(&p))),
                "ico {k}"
            );
        }
        for (a, b) in [(3, 3), (48, 12)] {
            let p = torus::Torus {
                major_segments: a,
                minor_segments: b,
                ..Default::default()
            };
            assert_eq!(
                p.estimate(),
                Some(counts(&torus::torus(&p))),
                "torus {a}x{b}"
            );
        }
        let p = plane::Plane::default();
        assert_eq!(p.estimate(), Some(counts(&plane::plane(&p))));
        let p = cube::Cube::default();
        assert_eq!(p.estimate(), Some(counts(&cube::cube_mesh(&p))));
        let p = monkey::Monkey::default();
        assert_eq!(p.estimate(), Some(counts(&monkey::monkey(&p))));
    }

    #[test]
    fn absurd_requests_do_not_overflow() {
        let g = grid::Grid {
            x_subdivisions: 10_000_000,
            y_subdivisions: 10_000_000,
            ..Default::default()
        };
        let e = g.estimate().expect("fits u64");
        assert_eq!(e.faces, 100_000_000_000_000);
        assert_eq!(e.corners, 400_000_000_000_000);
        let s = uv_sphere::UvSphere {
            segments: 100_000,
            ring_count: 100_000,
            ..Default::default()
        };
        assert_eq!(s.estimate().expect("fits").faces, 10_000_000_000);
        let c = circle::Circle {
            vertices: i32::MAX,
            fill_type: FillType::Trifan,
            ..Default::default()
        };
        assert!(c.estimate().expect("fits").corners > u32::MAX as u64);
    }
}
