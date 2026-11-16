// SPDX-License-Identifier: GPL-3.0-or-later
//! Face/corner normals and loop triangles (ARCHITECTURE §6.1). `sqrt` is IEEE-exact, so no
//! `libm` is needed here (rule 2.2.12 concerns transcendentals).

use crate::mesh::Mesh;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

// Ported from blender/source/blender/blenlib/intern/math_vector_inline.c (normalize_v3_v3_length).
fn normalize(n: [f32; 3]) -> [f32; 3] {
    let d = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if d > 1.0e-35 {
        let inv = 1.0 / d.sqrt();
        [n[0] * inv, n[1] * inv, n[2] * inv]
    } else {
        [0.0, 0.0, 0.0]
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (normal_tri_v3, normal_quad_v3,
// normal_poly_v3) as dispatched by blender/source/blender/blenkernel/intern/mesh_normals.cc
// (face_normal_calc).
fn face_normal(p: &[[f32; 3]], verts: &[i32]) -> [f32; 3] {
    let v = |i: usize| p[verts[i] as usize];
    match verts.len() {
        3 => normalize(cross(sub(v(0), v(1)), sub(v(1), v(2)))),
        4 => normalize(cross(sub(v(0), v(2)), sub(v(1), v(3)))),
        n => {
            let mut acc = [0.0f32; 3];
            let mut prev = v(n - 1);
            for i in 0..n {
                let cur = v(i);
                acc[0] += (prev[1] - cur[1]) * (prev[2] + cur[2]);
                acc[1] += (prev[2] - cur[2]) * (prev[0] + cur[0]);
                acc[2] += (prev[0] - cur[0]) * (prev[1] + cur[1]);
                prev = cur;
            }
            normalize(acc)
        }
    }
}

pub(crate) fn face_normals(m: &Mesh) -> Vec<[f32; 3]> {
    let p = m.positions();
    let cv = m.corner_verts();
    (0..m.faces_num())
        .map(|f| face_normal(p, &cv[m.face(f)]))
        .collect()
}

/// Corner normals. Sharp faces (`sharp_face`) use the face normal, as Blender does. Smooth
/// faces use angle-weighted vertex normals over all faces (M0 limit: no fan splitting at
/// sharp edges or non-manifold edges and no custom normals yet; M1 ports
/// `mesh_normals.cc::normals_calc_corners`).
pub(crate) fn corner_normals(m: &Mesh, fnor: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let cv = m.corner_verts();
    let p = m.positions();
    let sharp = m.sharp_faces();
    let all_sharp = sharp.is_some_and(|s| s.iter().all(|b| *b));
    let mut vnor: Vec<[f32; 3]> = Vec::new();
    if !all_sharp {
        vnor = vec![[0.0; 3]; m.verts_num()];
        for (f, fn_) in fnor.iter().enumerate().take(m.faces_num()) {
            let r = m.face(f);
            let n = r.len();
            for (k, c) in r.clone().enumerate() {
                let prev = p[cv[r.start + (k + n - 1) % n] as usize];
                let cur = p[cv[c] as usize];
                let next = p[cv[r.start + (k + 1) % n] as usize];
                let a = normalize(sub(prev, cur));
                let b = normalize(sub(next, cur));
                let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
                // Ported from blender/source/blender/blenlib/intern/math_base_inline.c (saacos).
                let w = libm::acosf(dot);
                let vn = &mut vnor[cv[c] as usize];
                for (acc, c) in vn.iter_mut().zip(fn_) {
                    *acc += c * w;
                }
            }
        }
        for v in &mut vnor {
            *v = normalize(*v);
        }
    }
    let mut out = vec![[0.0f32; 3]; m.corners_num()];
    for f in 0..m.faces_num() {
        let face_sharp = sharp.map(|s| s[f]).unwrap_or(false);
        for c in m.face(f) {
            out[c] = if face_sharp {
                fnor[f]
            } else {
                vnor[cv[c] as usize]
            };
        }
    }
    out
}

/// Loop triangles: quads → `(0,1,2),(0,2,3)` (verified in Blender), triangles as is.
/// M0 limit: n-gons are fanned; M1 ports `BLI_polyfill_2d` (no beautify).
/// Vertex normals: angle-weighted face normals, zero-length → normalised position
/// (`mesh_normals.cc`, `normals_calc_verts`). M1 W0 stub: WP 1.4 ports it bit-exactly.
pub(crate) fn vert_normals(m: &Mesh, fnor: &[[f32; 3]]) -> Vec<[f32; 3]> {
    let _ = (m, fnor);
    todo!("WP 1.4: port normals_calc_verts (angle weighting as in Blender 5.2)")
}

pub(crate) fn looptris(m: &Mesh) -> Vec<[u32; 3]> {
    let mut out = Vec::new();
    for f in 0..m.faces_num() {
        let r = m.face(f);
        let s = r.start as u32;
        for k in 1..(r.len() as u32).saturating_sub(1) {
            out.push([s, s + k, s + k + 1]);
        }
    }
    out
}
