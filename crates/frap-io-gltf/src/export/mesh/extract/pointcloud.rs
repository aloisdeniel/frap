// SPDX-License-Identifier: GPL-3.0-or-later
//! POINTCLOUD objects → glTF meshes with one POINTS primitive (`use_mesh_vertices`; without it
//! a point cloud has no primitive and its node no mesh, with the add-on's "has no primitives"
//! warning).
//!
//! A point cloud is not a `Mesh`, so the extractor slice cannot see it: the node gatherer asks
//! [`gather`] for POINTCLOUD objects, which writes the primitive itself (the add-on's
//! `__gather_cache_primitives` likewise branches to `gather_point_cloud` before the mesh
//! extraction). The file's `PRIMITIVE_EXTRACTORS` entry keeps the one-file-one-entry rule and
//! extracts nothing from meshes. The
//! primitive has no `indices`, material slot 0, and the attributes `_<name>` of every
//! `_`-prefixed POINT attribute (FLOAT_COLOR as FLOAT VEC4, BYTE_COLOR as normalized
//! UNSIGNED_SHORT VEC4 of the linear colour, INT/BOOLEAN/INT8 as FLOAT), `POSITION` (with
//! min/max) and `_RADIUS` (FLOAT). Positions are written as stored, without the Y-up
//! conversion meshes get (the add-on's behaviour, verified on 5.2.4). A point cloud without a
//! `radius` attribute fails the export as Blender's does (`KeyError` on `attributes['radius']`),
//! with a frap message. Meshes are shared like meshes: per (point cloud, slot materials).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/pointcloud.py` (`gather_point_cloud`,
//! `__get_custom_attributes`), `blender/exp/primitives.py` (`__gather_cache_primitives`) and
//! `blender/exp/obj_data.py` (`gather_mesh`, `__gather_extras`), Apache-2.0, The
//! glTF-Blender-IO authors.

use std::collections::HashMap;

use frap_gltf::json::{AccessorType, Index, Mesh as GMesh, Mode, Primitive};
use frap_gltf::{AccessorData, AccessorOpts, accessor::Components};
use frap_mesh::{AttrData, Domain};
use frap_rna::{IdRef, MaterialId, PointCloudId};
use frap_scene::id::{Object, PointCloud, SlotLink};
use indexmap::IndexMap;

use crate::export::gather::Gather;
use crate::export::mesh::convert::byte_color_to_linear;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_EXTRACTORS, PrimitiveDraft, PrimitiveExtractor, SplitKey,
};
use crate::export::mesh::gather::material_for_slot;
use crate::export::{ExportError, materials};

/// A glTF mesh's identity: the point cloud and the object's material per slot.
type PcKey = (IdRef<PointCloudId>, Vec<Option<IdRef<MaterialId>>>);

/// Export-wide caches (`Gather::state`).
#[derive(Default)]
struct Cache {
    /// glTF mesh per (point cloud, slot materials).
    meshes: HashMap<PcKey, Option<Index>>,
    /// Written attributes per point cloud (shared by every glTF mesh of it).
    primitives: HashMap<IdRef<PointCloudId>, Option<IndexMap<String, Index>>>,
}

/// The object's material per slot of its point cloud.
fn slot_materials(ob: &Object, pc: &PointCloud) -> Vec<Option<IdRef<MaterialId>>> {
    (0..pc.materials.len())
        .map(|i| match ob.material_slot_overrides.get(i) {
            Some(o) if o.link == SlotLink::Object => o.material,
            _ => pc.materials[i],
        })
        .collect()
}

/// One custom attribute as accessor data (`__get_custom_attributes` + `__gather_attribute`).
fn custom(data: &AttrData) -> Option<AccessorData> {
    let f = |type_, v: Vec<f32>| AccessorData::new(type_, false, Components::F32(v)).ok();
    match data {
        AttrData::Float(v) => f(AccessorType::Scalar, v.to_vec()),
        AttrData::Int(v) => f(AccessorType::Scalar, v.iter().map(|&x| x as f32).collect()),
        AttrData::Int8(v) => f(AccessorType::Scalar, v.iter().map(|&x| f32::from(x)).collect()),
        AttrData::Bool(v) => f(
            AccessorType::Scalar,
            v.iter().map(|&x| if x { 1.0 } else { 0.0 }).collect(),
        ),
        AttrData::Float2(v) => f(AccessorType::Vec2, v.iter().flatten().copied().collect()),
        AttrData::Float3(v) => f(AccessorType::Vec3, v.iter().flatten().copied().collect()),
        AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            f(AccessorType::Vec4, v.iter().flatten().copied().collect())
        }
        AttrData::Float4x4(v) => f(
            AccessorType::Mat4,
            v.iter().flatten().flatten().copied().collect(),
        ),
        AttrData::ByteColor(v) => AccessorData::new(
            AccessorType::Vec4,
            true,
            Components::U16(
                v.iter()
                    .flat_map(|c| crate::export::mesh::convert::color_to_ushort(byte_color_to_linear(*c)))
                    .collect(),
            ),
        )
        .ok(),
        _ => None,
    }
}

/// Writes the POINTS primitive's attributes (`None` without `use_mesh_vertices`).
fn write_attributes(
    g: &mut Gather,
    pc: &PointCloud,
) -> Result<Option<IndexMap<String, Index>>, ExportError> {
    if !g.settings.props.use_mesh_vertices.value {
        return Ok(None);
    }
    let radius = match pc.attributes().get("radius") {
        Some(a) if matches!(a.data, AttrData::Float(_)) => pc.radii(),
        _ => {
            return Err(ExportError::Invalid(format!(
                "point cloud '{}' has no 'radius' attribute (Blender 5.2's exporter fails on it \
                 too: KeyError 'radius'); set one with points[i].radius",
                pc.header.name
            )));
        }
    };
    // `custom_attributes`, then POSITION and _RADIUS (a custom `_RADIUS` keeps its place but
    // gets the radii, as the dict assignment does).
    let mut cols: IndexMap<String, (AccessorData, bool)> = IndexMap::new();
    for a in pc.attributes().iter() {
        if a.domain != Domain::Point
            || a.name == "position"
            || a.name == "radius"
            || !a.name.starts_with('_')
        {
            continue;
        }
        if let Some(d) = custom(&a.data) {
            cols.insert(a.name.to_string(), (d, false));
        }
    }
    cols.insert(
        "POSITION".into(),
        (AccessorData::from_elements(&pc.positions()), true),
    );
    cols.insert(
        "_RADIUS".into(),
        (AccessorData::from_elements(&radius), false),
    );
    let mut out = IndexMap::new();
    for (name, (data, min_max)) in cols {
        let i = g.buf.push_accessor(&data, &AccessorOpts::attribute(min_max));
        out.insert(name, i);
    }
    Ok(Some(out))
}

/// The glTF mesh of a POINTCLOUD object (`None` for other objects, or without primitive).
pub fn gather(g: &mut Gather, ob: &Object) -> Result<Option<Index>, ExportError> {
    frap_math::budget::check_now();
    if ob.object_type() != "POINTCLOUD" {
        return Ok(None);
    }
    let Some(r) = ob.data.and_then(|d| d.typed::<PointCloudId>()) else {
        return Ok(None);
    };
    let main = g.main;
    let Some(pc) = main.pointclouds.get(r) else {
        return Ok(None);
    };
    let mats = slot_materials(ob, pc);
    let key = (r, mats.clone());
    if let Some(i) = g.state::<Cache>().meshes.get(&key) {
        return Ok(*i);
    }
    let attributes = match g.state::<Cache>().primitives.get(&r) {
        Some(a) => a.clone(),
        None => {
            let a = write_attributes(g, pc)?;
            g.state::<Cache>().primitives.insert(r, a.clone());
            a
        }
    };
    let index = match attributes {
        None => {
            g.warn(format!(
                "Mesh '{}' has no primitives and will be omitted.",
                pc.header.name
            ));
            None
        }
        Some(attributes) => {
            let material = match material_for_slot(g.settings.export_materials, 0, &mats) {
                Some(m) => materials::gather(g, m, &frap_mesh::Mesh::new(""))?,
                None => None,
            };
            let extras = crate::export::extras::id(g, &pc.header);
            g.doc.meshes.push(GMesh {
                name: Some(pc.header.name.to_string()),
                primitives: vec![Primitive {
                    attributes,
                    indices: None,
                    material,
                    mode: Some(Mode::Points),
                    ..Primitive::default()
                }],
                extras,
                ..GMesh::default()
            });
            Some((g.doc.meshes.len() - 1) as Index)
        }
    };
    g.state::<Cache>().meshes.insert(key, index);
    Ok(index)
}

/// Meshes have no point-cloud primitive (see the module docs).
fn run(_: &MeshExportCx, _: &SplitKey, _: &mut Vec<PrimitiveDraft>) {}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_EXTRACTORS)]
static POINTCLOUD: PrimitiveExtractor = PrimitiveExtractor {
    name: "pointcloud",
    order: 400,
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_rna::IdAny;
    use frap_scene::{ChangeLog, Context, Preferences, startup};

    use super::*;

    #[test]
    fn points_with_radius_and_custom_attributes() {
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let mut pc = PointCloud::new("PC");
        pc.resize(2);
        pc.set_position(1, [1.0, 2.0, 3.0]);
        pc.set_radius(0, 0.5);
        pc.set_attribute("_w", AttrData::Float(Arc::new(vec![3.0, 4.0])))
            .unwrap();
        pc.set_attribute("skip", AttrData::Float(Arc::new(vec![3.0, 4.0])))
            .unwrap();
        let r = main.pointclouds.insert("PC", pc, &mut log);
        let ob = main
            .objects
            .insert("PC", Object::new("", Some(IdAny::from(r))), &mut log);
        let coll = main.collections.find("Collection").unwrap();
        main.collections
            .get_mut(coll, &mut log)
            .unwrap()
            .objects
            .push(ob);
        let prefs = Preferences::default();
        let ctx = Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.use_mesh_vertices.value = true;
        let s = crate::export::ExportSettings {
            props: Arc::new(p),
            ..crate::export::ExportSettings::default()
        };
        let out = crate::export::export(&main, &ctx, &s).unwrap();
        let doc = out.document;
        let m = doc
            .meshes
            .iter()
            .find(|m| m.name.as_deref() == Some("PC"))
            .unwrap();
        let p = &m.primitives[0];
        assert_eq!(p.mode, Some(Mode::Points));
        assert!(p.indices.is_none());
        let keys: Vec<_> = p.attributes.keys().map(String::as_str).collect();
        assert_eq!(keys, ["_w", "POSITION", "_RADIUS"]);
        let pos = &doc.accessors[p.attributes["POSITION"] as usize];
        // Not converted to Y-up.
        assert_eq!(pos.max.as_deref(), Some(&[1.0, 2.0, 3.0][..]));

        // Off by default: no mesh for the point cloud.
        let s = crate::export::ExportSettings::default();
        let out = crate::export::export(&main, &ctx, &s).unwrap();
        assert!(!out.document.meshes.iter().any(|m| m.name.as_deref() == Some("PC")));
        assert!(out.reports.iter().any(|r| r.message.contains("'PC' has no primitives")));
    }
}
