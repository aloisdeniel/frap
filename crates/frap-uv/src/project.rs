// SPDX-License-Identifier: GPL-3.0-or-later
//! Projection helpers of `uvedit_unwrap_ops.cc` and `BLI_math_geom` on plain arrays (WP 3.3):
//! cube projection by dominant axis, `map_to_tube`/`map_to_sphere` and the seam fix-up
//! `uv_map_mirror`. `frap-ops-uv` (WP 3.8) applies the view/object transforms and writes the
//! corner UVs. Stub bodies until WP 3.3.

/// `map_to_tube`: cylindrical UV of a point already in the projection frame.
pub fn map_to_tube(co: [f32; 3]) -> [f32; 2] {
    let _ = co;
    todo!("WP 3.3: map_to_tube")
}

/// `map_to_sphere`: spherical UV of a point already in the projection frame.
pub fn map_to_sphere(co: [f32; 3]) -> [f32; 2] {
    let _ = co;
    todo!("WP 3.3: map_to_sphere")
}

/// `uv_map_mirror`: moves the corners of a face that wraps across the seam (`u` jumps by more
/// than half) to the same side as the face's first corner (`mirror_u`), or to its `u` average.
pub fn uv_map_mirror(face_uvs: &mut [[f32; 2]], fix_u: bool, fix_v: bool) {
    let _ = (face_uvs, fix_u, fix_v);
    todo!("WP 3.3: uv_map_mirror")
}

/// `uvedit_unwrap_cube_project` for one face: projects `cos` (object space) on the plane of
/// the face normal's dominant axis, `uv = (co - loc) * cube_size_inv + 0.5`.
pub fn cube_project_face(
    normal: [f32; 3],
    cos: &[[f32; 3]],
    loc: [f32; 3],
    cube_size: f32,
    out: &mut [[f32; 2]],
) {
    let _ = (normal, cos, loc, cube_size, out);
    todo!("WP 3.3: cube projection")
}
