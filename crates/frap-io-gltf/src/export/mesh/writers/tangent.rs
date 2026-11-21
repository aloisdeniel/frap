// SPDX-License-Identifier: GPL-3.0-or-later
//! TANGENT writer (`export_tangents`, only with `export_normals`): MikkTSpace corner tangents
//! of the active UV map, as `Mesh.calc_tangents()` computes them, rounded to 4 decimals (not
//! re-normalised), then Y-up; `w` is the bitangent sign, unchanged by the axis conversion. Key =
//! those four floats as f32 bits with `-0.0` folded (4 words per corner); `run` writes
//! `TANGENT` (FLOAT VEC4) from the representative corners. Loose edges and points have no
//! corner and get no tangent.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`prepare_data`: tangents only when the mesh has a UV map and `calc_tangents()` succeeds,
//! else a warning and none; `define_attributes`: TANGENT after the TEXCOORD_n;
//! `__get_tangents`: `np.round(tangents, ROUNDING_DIGIT)` then `zup2yup`;
//! `__get_bitangent_signs`). The tangents themselves come from
//! [`frap_geom::mikktspace::calc_uv_tangent_tris_quads`]; a mesh with n-gons ("Tangent space can
//! only be computed for tris/quads") exports without TANGENT, as the add-on's caught exception
//! does, with its WARNING `<mesh>: Could not calculate tangents. Please try to triangulate the
//! mesh first.` (WP 7.7, through [`MeshExportCx::warn`]).

use frap_mesh::{AttrData, Domain, Mesh};

use crate::export::ExportError;
use crate::export::mesh::convert::{axis, key_word, round4};
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter,
    PrimitiveDraft, SplitKey,
};

/// This writer's split-key column.
pub const NAME: &str = "tangent";

/// `export_tangents` is honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_tangents"];

/// `Mesh.calc_tangents()` with the active UV map: `None` when the mesh has no UV map or the
/// computation fails (n-gons).
pub fn mesh_tangents(mesh: &Mesh) -> Option<Vec<[f32; 4]>> {
    let name = mesh.uv_active.as_deref()?;
    let uv = mesh
        .attributes()
        .get(name)
        .and_then(|a| match (&a.domain, &a.data) {
            (Domain::Corner, AttrData::Float2(v)) => Some(v.clone()),
            _ => None,
        })?;
    let normals = mesh.corner_normals();
    frap_geom::mikktspace::calc_uv_tangent_tris_quads(
        mesh.positions(),
        mesh.face_offsets(),
        mesh.corner_verts(),
        &normals,
        &uv,
    )
    .ok()
}

/// `prepare_data`'s WARNING when `calc_tangents()` fails (`mesh.name` is the exported mesh's,
/// the evaluated copy keeping the original's name).
pub fn tangent_warning(mesh: &Mesh) -> String {
    format!(
        "{}: Could not calculate tangents. Please try to triangulate the mesh first.",
        mesh.header.name
    )
}

/// The exported value of one tangent: rounded xyz (Y-up when `export_yup`) and the sign.
pub fn tangent_value(t: [f32; 4], export_yup: bool) -> [f32; 4] {
    let [x, y, z] = axis([round4(t[0]), round4(t[1]), round4(t[2])], export_yup);
    [x, y, z, t[3]]
}

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    let s = cx.settings;
    if !(s.export_normals && s.props.export_tangents.value) || cx.mesh.uv_maps().is_empty() {
        return Ok(());
    }
    let Some(tangents) = mesh_tangents(cx.mesh) else {
        // `prepare_data`: `calc_tangents()` raised (WP 7.7; a popup, so the operator reports
        // it).
        cx.warn(tangent_warning(cx.mesh));
        return Ok(());
    };
    let mut words = Vec::with_capacity(4 * tangents.len());
    // A skinned mesh's dots hold the tangents rotated by `tangent_transform` (`__get_tangents`).
    let skin = cx.skin.map(|s| s.tangent_transform());
    for t in tangents {
        let v = match &skin {
            Some(m) => {
                let [x, y, z] = axis(
                    crate::export::mesh::writers::skin::skinned_tangent([t[0], t[1], t[2]], m),
                    s.export_yup,
                );
                [x, y, z, t[3]]
            }
            None => tangent_value(t, s.export_yup),
        };
        words.extend(v.map(key_word));
    }
    k.push(NAME, 4, words);
    Ok(())
}

fn run(_: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(col) = k.column(NAME) else {
        return Ok(());
    };
    if d.corners.contains(&u32::MAX) {
        return Ok(());
    }
    let values = d
        .corners
        .iter()
        .map(|&c| {
            let w = &col.words[4 * c as usize..4 * c as usize + 4];
            [0, 1, 2, 3].map(|i| f32::from_bits(w[i]))
        })
        .collect();
    d.attributes.insert(
        "TANGENT".into(),
        AttrColumn {
            values: AttrValues::F32x4(values),
            normalized: false,
        },
    );
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static TANGENT: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 300,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;
    use crate::export::mesh::primitives;

    fn settings(tangents: bool, normals: bool) -> ExportSettings {
        let mut p = crate::export::settings::default_props();
        p.export_tangents.value = tangents;
        ExportSettings {
            export_normals: normals,
            props: std::sync::Arc::new(p),
            ..ExportSettings::default()
        }
    }

    #[test]
    fn cube_tangents_split_nothing_more_and_are_unit_x_like() {
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let main = Main::default();
        let s = settings(true, true);
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert_eq!(d.len(), 1);
        let t = &d[0].attributes["TANGENT"];
        let AttrValues::F32x4(v) = &t.values else {
            panic!("TANGENT is FLOAT VEC4")
        };
        // The flat cube already splits into 24 vertices; tangents are constant per face.
        assert_eq!(v.len(), 24);
        assert_eq!(d[0].positions.len(), 24);
        for t in v {
            let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-3, "{t:?}");
            assert!(t[3] == 1.0 || t[3] == -1.0);
        }
    }

    #[test]
    fn off_by_default_and_without_normals() {
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let main = Main::default();
        for s in [settings(false, true), settings(true, false)] {
            let cx = MeshExportCx::new(&main, &m, None, &s);
            let d = primitives::extract(&cx).unwrap();
            assert!(!d[0].attributes.contains_key("TANGENT"));
        }
    }

    /// WP 7.7 (Blender 5.2.4, `followups/tangent_warning`): the default cylinder (n-gon caps,
    /// UVs) with `export_tangents` exports no TANGENT and reports `Cylinder: Could not
    /// calculate tangents. Please try to triangulate the mesh first.`.
    #[test]
    fn ngon_mesh_warns_and_exports_no_tangent() {
        let mut m = frap_mesh::primitives::cylinder::cylinder(
            &frap_mesh::primitives::cylinder::Cylinder::default(),
        );
        m.header.name = "Cylinder".into();
        let main = Main::default();
        let s = settings(true, true);
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert!(d.iter().all(|p| !p.attributes.contains_key("TANGENT")));
        let r = cx.take_reports();
        assert!(
            r.contains(&
                "Cylinder: Could not calculate tangents. Please try to triangulate the mesh first."
                    .to_owned()),
            "{r:?}"
        );
        // A cube triangulates fine: no warning.
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let cx = MeshExportCx::new(&main, &m, None, &s);
        primitives::extract(&cx).unwrap();
        assert!(cx.take_reports().is_empty());
    }

    #[test]
    fn rounding_and_axis() {
        assert_eq!(
            tangent_value([0.123_456, -0.5, 0.25, -1.0], true),
            [0.1235, 0.25, 0.5, -1.0]
        );
        assert_eq!(
            tangent_value([1.0, 0.0, 0.0, 1.0], false),
            [1.0, 0.0, 0.0, 1.0]
        );
    }
}
