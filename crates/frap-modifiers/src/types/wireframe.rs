// SPDX-License-Identifier: GPL-3.0-or-later
//! WIREFRAME (`WireframeModifier`): turns every edge into a square tube of `thickness`
//! (`offset` moves it along the normals, `use_even_offset`/`use_relative_offset` scale it,
//! `vertex_group` with `thickness_vertex_group` thins it), optionally keeping the original
//! faces (`use_replace` off), adding boundary rims (`use_boundary`) and creasing the wire edges
//! (`use_crease`, `crease_weight`); new faces use `material_offset`, clamped to the object's
//! slots.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_wireframe.cc (Blender 5.2.4) over
//! `frap_bmesh::ops::wireframe::mesh_wireframe_ex` (`BM_mesh_wireframe`).

use frap_bmesh::ops::wireframe::{WireframeVgroup, mesh_wireframe_ex};
use frap_bmesh::{BMesh, FromMeshParams, ToMeshParams};

use super::smooth::common::defgroup_index;
use crate::prelude::*;

#[modifier(
    WIREFRAME,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Wireframe(s) = &md.settings else {
        return Ok(mesh);
    };
    let vgroup = defgroup_index(&mesh, &s.vertex_group).map(|index| WireframeVgroup {
        index,
        invert: s.invert_vertex_group,
        offset_fac_vg: s.thickness_vertex_group,
    });
    let mut bm = BMesh::from_mesh(
        &mesh,
        &FromMeshParams {
            calc_face_normal: true,
            calc_vert_normal: true,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );
    // `std::max(ob->totcol - 1, 0)`: the object's slot count is its data's material count.
    let mat_max = (mesh.materials.len() as i32 - 1).max(0);
    mesh_wireframe_ex(
        &mut bm,
        s.thickness,
        s.offset,
        s.use_replace,
        s.use_boundary,
        s.use_even_offset,
        s.use_relative_offset,
        s.use_crease,
        s.crease_weight,
        vgroup,
        s.material_offset,
        mat_max,
        false,
    );
    Ok(bm.to_mesh(&mesh, &ToMeshParams::default()))
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;
    use frap_scene::generated::modifier_settings::WireframeSettings;

    fn settings(p: &serde_json::Value) -> WireframeSettings {
        let b = |k: &str| p[k].as_bool().unwrap();
        let f = |k: &str| p[k].as_f64().unwrap() as f32;
        WireframeSettings {
            thickness: f("thickness"),
            thickness_vertex_group: f("thickness_vertex_group"),
            offset: f("offset"),
            use_replace: b("use_replace"),
            use_boundary: b("use_boundary"),
            use_even_offset: b("use_even_offset"),
            use_relative_offset: b("use_relative_offset"),
            use_crease: false,
            crease_weight: 1.0,
            material_offset: 0,
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
            invert_vertex_group: b("invert_vertex_group"),
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/wireframe.json`).
    #[test]
    fn vertex_group_matches_blender() {
        let d = load("vgroups/wireframe.json");
        for case in d["cases"].as_array().unwrap() {
            let mesh = mesh_from_dump(&case["input"]);
            let md = modifier(ModifierSettings::Wireframe(settings(&case["props"])));
            let mut cx = TestCx::new();
            let out = eval(&md, mesh, &mut cx).unwrap();
            assert_topology(&out, &case["output"]);
            assert_positions(&out, &case["output"], 1e-5);
        }
    }
}
