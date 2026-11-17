// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh centers used by `object.origin_set`. Ports of `BKE_mesh_center_median`,
//! `BKE_mesh_center_median_from_faces`, `BKE_mesh_center_of_surface`,
//! `BKE_mesh_center_of_volume` and their face helpers (`blenkernel/intern/mesh_evaluate.cc`),
//! Blender 5.2.4, GPL-2.0-or-later. Summation order follows Blender's loops.

use frap_mesh::Mesh;
use glam::Vec3;

fn positions(mesh: &Mesh) -> Vec<Vec3> {
    mesh.positions()
        .iter()
        .map(|p| Vec3::from_array(*p))
        .collect()
}

/// `BKE_mesh_center_median`: the mean of the vertex positions (zero without vertices).
pub fn median(mesh: &Mesh) -> Vec3 {
    let p = positions(mesh);
    let mut c = Vec3::ZERO;
    for v in &p {
        c += *v;
    }
    if !p.is_empty() {
        c *= 1.0 / p.len() as f32;
    }
    c
}

/// The middle of the vertex bounds (`mesh->bounds_min_max()`; zero without vertices).
pub fn bounds(mesh: &Mesh) -> Vec3 {
    let p = positions(mesh);
    let Some(first) = p.first() else {
        return Vec3::ZERO;
    };
    let (mut min, mut max) = (*first, *first);
    for v in &p[1..] {
        min = min.min(*v);
        max = max.max(*v);
    }
    (min + max) * 0.5
}

/// `BKE_mesh_center_median_from_faces`: the mean of the face corners' positions.
pub fn median_from_faces(mesh: &Mesh) -> Vec3 {
    let p = positions(mesh);
    let cv = mesh.corner_verts();
    let mut c = Vec3::ZERO;
    let mut tot = 0usize;
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        for &v in &cv[r.clone()] {
            c += p[v as usize];
        }
        tot += r.len();
    }
    if mesh.faces_num() > 0 && tot > 0 {
        c *= 1.0 / tot as f32;
    }
    c
}

/// `area_tri_signed_v3`: the triangle area, negative when its winding opposes `normal`.
fn area_tri_signed(v1: Vec3, v2: Vec3, v3: Vec3, normal: Vec3) -> f32 {
    let n = (v2 - v1).cross(v3 - v2);
    let area = n.length() / 2.0;
    if n.dot(normal) < 0.0 { -area } else { area }
}

/// `mesh_calc_face_area_centroid`: a fan triangulation from the first corner, each triangle's
/// centroid weighted by its signed area.
fn face_area_centroid(p: &[Vec3], verts: &[i32], normal: Vec3) -> (f32, Vec3) {
    let v1 = p[verts[0] as usize];
    let mut v2 = p[verts[1] as usize];
    let mut total = 0.0f32;
    let mut cent = Vec3::ZERO;
    for &vi in &verts[2..] {
        let v3 = p[vi as usize];
        let tri_area = area_tri_signed(v1, v2, v3, normal);
        total += tri_area;
        let tri_cent = (v1 + v2 + v3) * (1.0 / 3.0);
        cent += tri_cent * tri_area;
        v2 = v3;
    }
    (total, cent * (1.0 / total))
}

/// `BKE_mesh_center_of_surface`: face centroids weighted by face area; zero-area meshes fall
/// back to the median.
pub fn of_surface(mesh: &Mesh) -> Vec3 {
    let p = positions(mesh);
    let cv = mesh.corner_verts();
    let normals = mesh.face_normals();
    let mut c = Vec3::ZERO;
    let mut total = 0.0f32;
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        if r.len() < 3 {
            continue;
        }
        let (area, cent) = face_area_centroid(&p, &cv[r], Vec3::from_array(normals[f]));
        c += cent * area;
        total += area;
    }
    if mesh.faces_num() > 0 {
        c *= 1.0 / total;
    }
    if !c.is_finite() {
        return median(mesh);
    }
    c
}

/// `mesh_calc_face_volume_centroid_with_reference_center`: signed tetrahedra (×6) of a fan
/// triangulation against `reference`, centroids weighted by volume.
fn face_volume_centroid(p: &[Vec3], verts: &[i32], reference: Vec3) -> (f32, Vec3) {
    let v1 = p[verts[0] as usize] - reference;
    let mut v2 = p[verts[1] as usize] - reference;
    let mut total = 0.0f32;
    let mut cent = Vec3::ZERO;
    for &vi in &verts[2..] {
        let v3 = p[vi as usize] - reference;
        let tetra_volume = v1.cross(v2).dot(v3);
        total += tetra_volume;
        cent += (v1 + v2 + v3) * tetra_volume;
        v2 = v3;
    }
    (total, cent)
}

/// `BKE_mesh_center_of_volume`: the centroid of the enclosed volume, relative to the corner
/// median for precision; open meshes whose volume sums to non-finite values fall back to that
/// median.
pub fn of_volume(mesh: &Mesh) -> Vec3 {
    let p = positions(mesh);
    let cv = mesh.corner_verts();
    let init = median_from_faces(mesh);
    let mut c = Vec3::ZERO;
    let mut total = 0.0f32;
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        if r.len() < 3 {
            continue;
        }
        let (vol, cent) = face_volume_centroid(&p, &cv[r], init);
        c += cent;
        total += vol;
    }
    if total != 0.0 {
        c *= 0.25 / total;
    }
    if !c.is_finite() {
        return init;
    }
    c + init
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_with_moved_corner() -> Mesh {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        let i = m
            .positions()
            .iter()
            .position(|p| *p == [1.0, 1.0, 1.0])
            .expect("corner (1, 1, 1)");
        m.positions_mut()[i] = [3.0, 3.0, 3.0];
        m
    }

    #[test]
    fn centers_match_blender() {
        // The factory cube with vertex 0 moved to (3, 3, 3), verified on 5.2.4: median
        // (0.25, 0.25, 0.25), bounds (1, 1, 1), surface (0.5447, 0.5928, 0.5928), volume
        // (0.4091, 0.5, 0.5).
        let m = cube_with_moved_corner();
        assert!(median(&m).abs_diff_eq(Vec3::splat(0.25), 1e-6));
        assert!(bounds(&m).abs_diff_eq(Vec3::ONE, 1e-6));
        assert!(
            of_surface(&m).abs_diff_eq(Vec3::new(0.5447, 0.5928, 0.5928), 1e-4),
            "{:?}",
            of_surface(&m)
        );
        assert!(
            of_volume(&m).abs_diff_eq(Vec3::new(0.4091, 0.5, 0.5), 1e-4),
            "{:?}",
            of_volume(&m)
        );
    }

    #[test]
    fn empty_mesh_centers_are_zero() {
        let m = Mesh::new("Empty");
        assert_eq!(median(&m), Vec3::ZERO);
        assert_eq!(bounds(&m), Vec3::ZERO);
        assert_eq!(of_surface(&m), Vec3::ZERO);
        assert_eq!(of_volume(&m), Vec3::ZERO);
    }
}
