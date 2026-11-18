// SPDX-License-Identifier: GPL-3.0-or-later
//! MikkTSpace tangents over `bevy_mikktspace` (ARCHITECTURE §3.2; M3, WP 3.7, stub created by
//! M3 W0): per-corner tangents and bitangent signs of a triangulated mesh, as Blender's
//! `Mesh.calc_tangents(uvmap)` and the glTF exporter's `TANGENT` (`w` = sign) compute them
//! (`blender/source/blender/blenkernel/intern/mesh_tangent.cc`).

/// Per-corner tangent (xyz) and bitangent sign (w) of the corners of `faces` (each face a
/// list of corner indices into `positions`/`normals`/`uvs`, already triangles or quads, as
/// MikkTSpace takes them). Stub until WP 3.7.
pub fn corner_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    faces: &[&[u32]],
) -> Vec<[f32; 4]> {
    let _ = (positions, normals, uvs, faces);
    todo!("WP 3.7: MikkTSpace tangents")
}
