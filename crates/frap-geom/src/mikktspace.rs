// SPDX-License-Identifier: GPL-3.0-or-later
//! MikkTSpace tangents (ARCHITECTURE §3.2; M3, WP 3.7): per-corner tangents and bitangent signs
//! of a mesh of triangles and quads, as Blender's `Mesh.calc_tangents(uvmap)` computes them and
//! the glTF exporter writes them as `TANGENT` (`w` = bitangent sign).
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_tangent.cc
//! (`MeshToTangentQuadsTris`, `calc_uv_tangent_tris_quads`): the faces are the mesh faces
//! themselves (not the loop triangles), positions through `corner_verts`, the corner normals and
//! the UV map per corner, texture coordinates as `(u, v, 1)`. The algorithm is the reference
//! MikkTSpace (`bevy_mikktspace`, byte-for-byte equal to the original C code, which Blender's
//! C++ rewrite in intern/mikktspace reproduces); a corner the algorithm gives no tangent space
//! (a degenerate face without a good neighbour) gets Blender's default `TSpace`: tangent
//! `(1, 0, 0)`, orientation not preserving (`w = -1`).

use bevy_mikktspace::{Geometry, TangentSpace, generate_tangents};

/// Why tangents could not be computed (`calc_uv_tangent_tris_quads` reports).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TangentError {
    /// A face has more than four corners: "Tangent space can only be computed for tris/quads,
    /// aborting".
    NotTrisQuads,
}

impl std::fmt::Display for TangentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TangentError::NotTrisQuads => {
                f.write_str("Tangent space can only be computed for tris/quads, aborting")
            }
        }
    }
}

impl std::error::Error for TangentError {}

/// Geometry seen by MikkTSpace: faces as lists of corner indices into per-corner arrays.
struct Corners<'a> {
    positions: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    uvs: &'a [[f32; 2]],
    faces: &'a [&'a [u32]],
    out: Vec<[f32; 4]>,
}

impl Geometry for Corners<'_> {
    fn num_faces(&self) -> usize {
        self.faces.len()
    }
    fn num_vertices_of_face(&self, face: usize) -> usize {
        self.faces[face].len()
    }
    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.faces[face][vert] as usize]
    }
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.faces[face][vert] as usize]
    }
    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.faces[face][vert] as usize]
    }
    fn set_tangent(&mut self, ts: Option<TangentSpace>, face: usize, vert: usize) {
        let ts = ts.unwrap_or_default();
        self.out[self.faces[face][vert] as usize] = ts.tangent_encoded();
    }
}

/// Per-corner tangent (xyz) and bitangent sign (w) of the corners of `faces` (each face a list
/// of corner indices into the per-corner `positions`/`normals`/`uvs`, triangles or quads, as
/// MikkTSpace takes them). Corners no face lists stay zero (Blender's zeroed tangent layer);
/// faces of other sizes are skipped, as MikkTSpace skips them.
pub fn corner_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    faces: &[&[u32]],
) -> Vec<[f32; 4]> {
    let mut g = Corners {
        positions,
        normals,
        uvs,
        faces,
        out: vec![[0.0; 4]; positions.len()],
    };
    // The reference implementation cannot fail (`GenerateTangentSpaceError` has no variant).
    let _ = generate_tangents(&mut g);
    g.out
}

/// `bke::mesh::calc_uv_tangent_tris_quads`: the tangents of every corner of a mesh given as
/// vertex positions, face offsets (`len = faces + 1`), corner vertices, corner normals and the
/// UV map (one value per corner). Fails when a face has more than four corners.
pub fn calc_uv_tangent_tris_quads(
    vert_positions: &[[f32; 3]],
    face_offsets: &[i32],
    corner_verts: &[i32],
    corner_normals: &[[f32; 3]],
    uv_map: &[[f32; 2]],
) -> Result<Vec<[f32; 4]>, TangentError> {
    if face_offsets.windows(2).any(|w| w[1] - w[0] > 4) {
        return Err(TangentError::NotTrisQuads);
    }
    let positions: Vec<[f32; 3]> = corner_verts
        .iter()
        .map(|&v| vert_positions[v as usize])
        .collect();
    let corners: Vec<u32> = (0..corner_verts.len() as u32).collect();
    let faces: Vec<&[u32]> = face_offsets
        .windows(2)
        .map(|w| &corners[w[0] as usize..w[1] as usize])
        .collect();
    Ok(corner_tangents(&positions, corner_normals, uv_map, &faces))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One quad in the XY plane, normal +Z, UVs aligned with X/Y.
    fn quad(uv_flip: bool) -> Vec<[f32; 4]> {
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let n = [[0.0, 0.0, 1.0]; 4];
        let mut uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        if uv_flip {
            for t in &mut uv {
                t[1] = 1.0 - t[1];
            }
        }
        calc_uv_tangent_tris_quads(&p, &[0, 4], &[0, 1, 2, 3], &n, &uv).unwrap()
    }

    #[test]
    fn tangent_follows_u_and_sign_follows_handedness() {
        for t in quad(false) {
            assert!((t[0] - 1.0).abs() < 1e-6 && t[1].abs() < 1e-6 && t[2].abs() < 1e-6);
            assert_eq!(t[3], 1.0);
        }
        // V mirrored: same tangent, bitangent sign flips.
        for t in quad(true) {
            assert!((t[0] - 1.0).abs() < 1e-6);
            assert_eq!(t[3], -1.0);
        }
    }

    #[test]
    fn ngons_are_refused() {
        let p = [[0.0; 3]; 5];
        let n = [[0.0, 0.0, 1.0]; 5];
        let uv = [[0.0; 2]; 5];
        assert_eq!(
            calc_uv_tangent_tris_quads(&p, &[0, 5], &[0, 1, 2, 3, 4], &n, &uv),
            Err(TangentError::NotTrisQuads)
        );
        assert_eq!(
            TangentError::NotTrisQuads.to_string(),
            "Tangent space can only be computed for tris/quads, aborting"
        );
    }

    #[test]
    fn degenerate_face_gets_the_default_space() {
        // A triangle with all corners at one point and constant UVs.
        let p = [[1.0, 1.0, 1.0]; 3];
        let n = [[0.0, 0.0, 1.0]; 3];
        let uv = [[0.5; 2]; 3];
        let t = calc_uv_tangent_tris_quads(&p, &[0, 3], &[0, 1, 2], &n, &uv).unwrap();
        assert_eq!(t, vec![[1.0, 0.0, 0.0, -1.0]; 3]);
    }
}
