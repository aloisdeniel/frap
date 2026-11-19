// SPDX-License-Identifier: GPL-3.0-or-later
//! TRIANGULATE (`TriangulateModifier`): splits every face with at least `min_vertices` corners
//! into triangles (`quad_method` BEAUTY, FIXED, FIXED_ALTERNATE, SHORTEST_DIAGONAL or
//! LONGEST_DIAGONAL; `ngon_method` BEAUTY or CLIP), removing triangles that duplicate existing
//! ones. `keep_custom_normals` carries the corner normals through the split and sets them back
//! as custom normals.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_triangulate.cc (Blender 5.2.4) over
//! `frap_bmesh::ops::triangulate::mesh_triangulate` (`BM_mesh_triangulate` without face maps:
//! the duplicate triangles are killed at the end).

use std::sync::Arc;

use frap_bmesh::ops::triangulate::{NgonMethod, QuadMethod, mesh_triangulate};
use frap_bmesh::{BMesh, FromMeshParams, ToMeshParams};
use frap_mesh::{AttrData, Attribute, Domain};
use frap_scene::generated::modifier_settings::TriangulateSettings;

use super::normal_edit::custom_normals::mesh_set_custom_normals;
use crate::prelude::*;

/// A corner layer carrying the corner normals through the BMesh (`CD_NORMAL`).
const NORMAL_LAYER: &str = ".frap_triangulate_corner_normal";

#[modifier(
    TRIANGULATE,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE | SUPPORTS_MAPPING | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Triangulate(s) = &md.settings else {
        return Ok(mesh);
    };
    Ok(triangulate(s, mesh))
}

/// `MOD_TRIANGULATE_QUAD_*` from the RNA identifier.
fn quad_method(id: &str) -> QuadMethod {
    match id {
        "BEAUTY" => QuadMethod::Beauty,
        "FIXED" => QuadMethod::Fixed,
        "FIXED_ALTERNATE" => QuadMethod::Alternate,
        "LONGEST_DIAGONAL" => QuadMethod::LongEdge,
        _ => QuadMethod::ShortEdge,
    }
}

/// `MOD_TRIANGULATE_NGON_*` from the RNA identifier.
fn ngon_method(id: &str) -> NgonMethod {
    match id {
        "CLIP" => NgonMethod::EarClip,
        _ => NgonMethod::Beauty,
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_triangulate.cc (triangulate_mesh).
/// The TRIANGULATE result of `mesh`.
pub(crate) fn triangulate(s: &TriangulateSettings, mut mesh: Mesh) -> Mesh {
    let keep_clnors = s.keep_custom_normals;
    if keep_clnors {
        let normals = mesh.corner_normals().to_vec();
        mesh.attributes_mut().insert(Attribute {
            name: NORMAL_LAYER.into(),
            domain: Domain::Corner,
            data: AttrData::Float3(Arc::new(normals)),
        });
    }
    let mut bm = BMesh::from_mesh(
        &mesh,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: false,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );
    let (_, doubles) = mesh_triangulate(
        &mut bm,
        quad_method(&s.quad_method),
        ngon_method(&s.ngon_method),
        s.min_vertices.max(0) as usize,
        false,
    );
    // `BM_mesh_triangulate` without face maps kills the duplicates last (most recent first).
    for &(f, _) in doubles.iter().rev() {
        if bm.faces.contains(f.0) {
            bm.face_kill(f);
        }
    }
    let mut result = bm.to_mesh(&mesh, &ToMeshParams::default());
    if keep_clnors {
        let normals = match result.attributes_mut().remove(NORMAL_LAYER) {
            Some(Attribute {
                data: AttrData::Float3(v),
                ..
            }) => v.to_vec(),
            _ => vec![[0.0; 3]; result.corners_num()],
        };
        let mut normals = normals;
        mesh_set_custom_normals(&mut result, &mut normals, false);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/triangulate.json`):
    /// `keep_custom_normals` on smooth meshes, exact element order.
    #[test]
    fn keep_custom_normals_matches_blender() {
        let d = load("vgroups/triangulate.json");
        for case in d["cases"].as_array().unwrap() {
            let p = &case["props"];
            let s = TriangulateSettings {
                quad_method: p["quad_method"].as_str().unwrap().into(),
                ngon_method: p["ngon_method"].as_str().unwrap().into(),
                min_vertices: p["min_vertices"].as_i64().unwrap() as i32,
                keep_custom_normals: p["keep_custom_normals"].as_bool().unwrap(),
            };
            let out = triangulate(&s, mesh_from_dump(&case["input"]));
            let want = &case["output"];
            assert_topology(&out, want);
            assert_attribute(&out, want, "sharp_edge", 0.0);
            assert_attribute(&out, want, "custom_normal", 0.0);
            assert_corner_normals(&out, want, 1e-5);
        }
    }

    #[test]
    fn cube_becomes_twelve_triangles() {
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let out = triangulate(&TriangulateSettings::default(), m);
        assert_eq!(out.faces_num(), 12);
        assert_eq!(out.edges_num(), 18);
        assert!((0..12).all(|f| out.face(f).len() == 3));
    }

    #[test]
    fn min_vertices_keeps_quads() {
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let s = TriangulateSettings {
            min_vertices: 5,
            ..TriangulateSettings::default()
        };
        assert_eq!(triangulate(&s, m).faces_num(), 6);
    }
}
