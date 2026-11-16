// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh → glTF primitive extraction with io_scene_gltf2 5.2.41 semantics.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`PrimitiveCreator`: dots per triangle corner, `fast_structured_np_unique`, normals rounded
//! to 4 decimals then normalised, UV `v → 1 - v`, Y-up conversion). The vertex order is the
//! add-on's: records sorted by their bytes viewed as UCS4 code points, i.e. lexicographically
//! by little-endian u32 words (vertex index first), with `-0.0` folded into `0.0`.
//! M0: triangles only; one primitive per used material slot; no colours, tangents, skins.

//!
//! M1 W0 moved this file from `export/primitives.rs` to `export/mesh/primitives.rs` (§17.2):
//! WP 1.16 turns it into the driver that runs `PRIMITIVE_EXTRACTORS`,
//! `PRIMITIVE_ATTRIBUTE_WRITERS` and `PRIMITIVE_POST_PROCESS` (`ext.rs`) in `order`.

use frap_mesh::Mesh;

pub use super::ext::PrimitiveDraft;

/// Blender (Z-up) → glTF (Y-up): (x, y, z) → (x, z, -y).
pub fn yup(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], -v[1]]
}

fn unneg_zero(x: f32) -> f32 {
    if x == 0.0 { 0.0 } else { x }
}

/// `np.round(x, 4)` on float32: `rint(x * 10000) / 10000` in f32 (ties to even).
fn round4(x: f32) -> f32 {
    (x * 10000.0f32).round_ties_even() / 10000.0f32
}

fn normalize_vec(n: [f32; 3]) -> [f32; 3] {
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len != 0.0 {
        [n[0] / len, n[1] / len, n[2] / len]
    } else {
        n
    }
}

/// Extracts the triangle primitives of a mesh (`export_apply=False`: original data).
pub fn extract(mesh: &Mesh) -> Vec<PrimitiveDraft> {
    let cv = mesh.corner_verts();
    let positions = mesh.positions();
    let corner_normals = mesh.corner_normals();
    let uv_maps = mesh.uv_maps();
    let looptris = mesh.looptris();
    // Per-face material slot (no material_index attribute in M0 → slot 0).
    let mut tri_face = Vec::with_capacity(looptris.len());
    for f in 0..mesh.faces_num() {
        let n = mesh.face(f).len().saturating_sub(2);
        tri_face.extend(std::iter::repeat_n(f, n));
    }
    let face_slot = |_f: usize| 0usize;

    // Record = [vertex_index, nx, ny, nz, (u, v)*] as u32 words (the numpy dtype's bytes).
    let words = 4 + 2 * uv_maps.len();
    let dot = |corner: usize| -> Vec<u32> {
        let mut r = Vec::with_capacity(words);
        r.push(cv[corner] as u32);
        let n = corner_normals[corner];
        let mut n = normalize_vec([round4(n[0]), round4(n[1]), round4(n[2])]);
        if n == [0.0; 3] {
            n = [0.0, 0.0, 1.0];
        }
        for c in yup(n) {
            r.push(unneg_zero(c).to_bits());
        }
        for (_, uv) in &uv_maps {
            let [u, v] = uv[corner];
            r.push(unneg_zero(u).to_bits());
            r.push(unneg_zero(-v + 1.0).to_bits());
        }
        r
    };

    let mut slots: Vec<usize> = tri_face.iter().map(|f| face_slot(*f)).collect();
    slots.sort_unstable();
    slots.dedup();
    let mut out = Vec::new();
    for slot in slots {
        let dots: Vec<Vec<u32>> = looptris
            .iter()
            .zip(&tri_face)
            .filter(|(_, f)| face_slot(**f) == slot)
            .flat_map(|(t, _)| t.iter().map(|c| dot(*c as usize)))
            .collect();
        if dots.is_empty() {
            continue;
        }
        let mut order: Vec<usize> = (0..dots.len()).collect();
        order.sort_by(|a, b| dots[*a].cmp(&dots[*b]));
        let mut unique: Vec<&Vec<u32>> = Vec::new();
        let mut inverse = vec![0u32; dots.len()];
        for i in order {
            if unique.last().is_none_or(|u| **u != dots[i]) {
                unique.push(&dots[i]);
            }
            inverse[i] = (unique.len() - 1) as u32;
        }
        let f = |w: u32| f32::from_bits(w);
        let mut prim = PrimitiveDraft {
            mode: 4,
            material_slot: slot,
            positions: Vec::with_capacity(unique.len()),
            normals: Vec::with_capacity(unique.len()),
            texcoords: vec![Vec::with_capacity(unique.len()); uv_maps.len()],
            indices: inverse,
            vertices: unique.iter().map(|r| r[0]).collect(),
            ..PrimitiveDraft::default()
        };
        for r in unique {
            prim.positions
                .push(yup(positions[r[0] as usize]).map(unneg_zero));
            prim.normals.push([f(r[1]), f(r[2]), f(r[3])]);
            for (k, tc) in prim.texcoords.iter_mut().enumerate() {
                tc.push([f(r[4 + 2 * k]), f(r[5 + 2 * k])]);
            }
        }
        out.push(prim);
    }
    out
}
