// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh objects → glTF meshes: runs the primitive driver once per mesh datablock, writes each
//! primitive's accessors, and builds one glTF mesh per (mesh, material slots) pair.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/nodes.py
//! (`__gather_mesh`: `materials = tuple(ms.material for ms in material_slots)`), obj_data.py
//! (`gather_mesh`, cache key `(id(mesh), modifiers, materials)`; name = mesh name; a mesh
//! without primitives is omitted with a warning), primitives.py (`gather_primitives`,
//! `__gather_cache_primitives` keyed by the mesh only, so objects that share a mesh but not
//! its materials share the accessors; `__gather_indices`), primitive_attributes.py
//! (POSITION is the only attribute with min/max; normalized integer attributes) and
//! material/materials.py (`get_material_from_idx`: an index past the last slot uses the last
//! slot; no material for `PLACEHOLDER`/`NONE`).

use frap_gltf::json::{Index, Mesh as GMesh, Mode, Primitive};
use frap_gltf::{AccessorData, AccessorOpts};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId, MeshId};
use frap_scene::id::{Object, SlotLink};
use indexmap::IndexMap;

use crate::export::gather::Gather;
use crate::export::mesh::ext::{AttrColumn, AttrValues, MeshExportCx, PrimitiveDraft};
use crate::export::mesh::primitives::{self, NO_MATERIAL_SLOT};
use crate::export::{ExportError, ExportMaterials, materials};

/// A primitive whose accessors are in the buffer (reused by every glTF mesh of its mesh).
#[derive(Clone, Debug)]
pub struct WrittenPrimitive {
    /// Attribute accessors in write order.
    pub attributes: IndexMap<String, Index>,
    /// Index accessor.
    pub indices: Index,
    /// glTF mode.
    pub mode: u32,
    /// Material slot index (`NO_MATERIAL_SLOT` with `export_materials='NONE'`).
    pub material_slot: usize,
    /// Morph target accessors.
    pub targets: Vec<IndexMap<String, Index>>,
}

/// The object's material per slot (`ob.material_slots[i].material`): the object's own material
/// for `link == 'OBJECT'` slots, the mesh's otherwise.
pub fn slot_materials(ob: &Object, mesh: &Mesh) -> Vec<Option<IdRef<MaterialId>>> {
    (0..mesh.materials.len())
        .map(|i| match ob.material_slot_overrides.get(i) {
            Some(o) if o.link == SlotLink::Object => o.material,
            _ => mesh.materials[i],
        })
        .collect()
}

/// `get_material_from_idx`.
pub fn material_for_slot(
    settings_materials: ExportMaterials,
    slot: usize,
    materials: &[Option<IdRef<MaterialId>>],
) -> Option<IdRef<MaterialId>> {
    if !matches!(
        settings_materials,
        ExportMaterials::Export | ExportMaterials::Viewport
    ) || slot == NO_MATERIAL_SLOT
        || materials.is_empty()
    {
        return None;
    }
    let i = slot.min(materials.len() - 1);
    materials[i]
}

/// Accessor data of one attribute column.
pub fn column_data(c: &AttrColumn) -> AccessorData {
    let d = match &c.values {
        AttrValues::F32x2(v) => AccessorData::from_elements(v),
        AttrValues::F32x3(v) => AccessorData::from_elements(v),
        AttrValues::F32x4(v) => AccessorData::from_elements(v),
        AttrValues::U8x4(v) => AccessorData::from_elements(v),
        AttrValues::U16x4(v) => AccessorData::from_elements(v),
    };
    AccessorData {
        normalized: c.normalized && d.component_type().can_normalize(),
        ..d
    }
}

/// The glTF mesh of a MESH object (`None` for other types and meshes without primitives).
pub fn gather_mesh(g: &mut Gather, ob: &Object) -> Result<Option<Index>, ExportError> {
    // `--stmt-timeout` cancellation point, once per exported object (§12).
    frap_math::budget::check_now();
    if ob.object_type() != "MESH" {
        return Ok(None);
    }
    let Some(me_ref) = ob.data.and_then(|d| d.typed::<MeshId>()) else {
        return Ok(None);
    };
    let (main, settings) = (g.main, g.settings);
    let Some(mesh) = main.meshes.get(me_ref) else {
        return Ok(None);
    };
    let mats = slot_materials(ob, mesh);
    let key = (me_ref, mats.clone());
    if let Some(i) = g.meshes.get(&key) {
        return Ok(*i);
    }
    // Primitives are extracted and written once per mesh (the add-on's
    // `__gather_cache_primitives`); each is followed by its material, in traversal order.
    enum Source {
        Fresh(std::vec::IntoIter<PrimitiveDraft>),
        Cached(std::vec::IntoIter<WrittenPrimitive>),
    }
    let mut source = match g.primitives.get(&me_ref) {
        Some(w) => Source::Cached(w.clone().into_iter()),
        None => {
            let cx = MeshExportCx::new(main, mesh, None, settings);
            Source::Fresh(primitives::extract(&cx)?.into_iter())
        }
    };
    let fresh = matches!(source, Source::Fresh(_));
    let mut out: Vec<Primitive> = Vec::new();
    let mut written_all: Vec<WrittenPrimitive> = Vec::new();
    loop {
        let written = match &mut source {
            Source::Fresh(drafts) => {
                let Some(mut d) = drafts.next() else { break };
                let cx = MeshExportCx::new(main, mesh, None, settings);
                primitives::post_process(&cx, &mut d, &mut g.buf)?;
                let w = write_accessors(g, &d)?;
                written_all.push(w.clone());
                w
            }
            Source::Cached(w) => match w.next() {
                Some(w) => w,
                None => break,
            },
        };
        let mat = material_for_slot(settings.export_materials, written.material_slot, &mats);
        let material = match mat {
            Some(m) => materials::gather(g, m, mesh)?,
            None => None,
        };
        out.push(Primitive {
            attributes: written.attributes,
            indices: Some(written.indices),
            material,
            mode: (written.mode != 4)
                .then(|| Mode::try_from(written.mode).ok())
                .flatten(),
            targets: written.targets,
            ..Primitive::default()
        });
    }
    if fresh {
        g.primitives.insert(me_ref, written_all);
    }
    let index = if out.is_empty() {
        g.warn(format!(
            "Mesh '{}' has no primitives and will be omitted.",
            mesh.header.name
        ));
        None
    } else {
        g.doc.meshes.push(GMesh {
            name: Some(mesh.header.name.to_string()),
            primitives: out,
            ..GMesh::default()
        });
        Some((g.doc.meshes.len() - 1) as Index)
    };
    g.meshes.insert(key, index);
    Ok(index)
}

/// Writes one primitive's accessors in the add-on's order: POSITION (with min/max), NORMAL,
/// TEXCOORD_n, every further attribute, then the indices (shared when identical), then the
/// morph targets.
fn write_accessors(g: &mut Gather, d: &PrimitiveDraft) -> Result<WrittenPrimitive, ExportError> {
    let n = d.positions.len();
    let check = |name: &str, len: usize| -> Result<(), ExportError> {
        if len == n {
            Ok(())
        } else {
            Err(ExportError::Invalid(format!(
                "attribute {name} has {len} values for {n} vertices"
            )))
        }
    };
    let mut attributes = IndexMap::new();
    let pos = g.buf.push_accessor(
        &AccessorData::from_elements(&d.positions),
        &AccessorOpts::attribute(true),
    );
    attributes.insert("POSITION".to_owned(), pos);
    if !d.normals.is_empty() {
        check("NORMAL", d.normals.len())?;
        let i = g.buf.push_accessor(
            &AccessorData::from_elements(&d.normals),
            &AccessorOpts::attribute(false),
        );
        attributes.insert("NORMAL".to_owned(), i);
    }
    for (k, tc) in d.texcoords.iter().enumerate() {
        check("TEXCOORD", tc.len())?;
        let i = g.buf.push_accessor(
            &AccessorData::from_elements(tc),
            &AccessorOpts::attribute(false),
        );
        attributes.insert(format!("TEXCOORD_{k}"), i);
    }
    for (name, col) in &d.attributes {
        let data = column_data(col);
        check(name, data.count())?;
        let i = g.buf.push_accessor(&data, &AccessorOpts::attribute(false));
        attributes.insert(name.to_string(), i);
    }
    let indices = match g.index_accessors.get(&d.indices) {
        Some(i) => *i,
        None => {
            if let Some(bad) = d.indices.iter().find(|i| **i as usize >= n) {
                return Err(ExportError::Invalid(format!(
                    "index {bad} out of range ({n} vertices)"
                )));
            }
            let i = g.buf.push_indices(&d.indices);
            g.index_accessors.insert(d.indices.clone(), i);
            i
        }
    };
    let mut targets = Vec::with_capacity(d.targets.len());
    for t in &d.targets {
        let mut m = IndexMap::new();
        for (name, col) in t {
            let data = column_data(col);
            check(name, data.count())?;
            let i = g
                .buf
                .push_accessor(&data, &AccessorOpts::attribute(name == "POSITION"));
            m.insert(name.to_string(), i);
        }
        targets.push(m);
    }
    Ok(WrittenPrimitive {
        attributes,
        indices,
        mode: d.mode,
        material_slot: d.material_slot,
        targets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_index_past_last_slot_uses_last() {
        let a = Some(IdRef::<MaterialId>::from_key(frap_rna::IdKey::next()));
        let b = Some(IdRef::<MaterialId>::from_key(frap_rna::IdKey::next()));
        let mats = [a, b];
        assert_eq!(material_for_slot(ExportMaterials::Export, 0, &mats), a);
        assert_eq!(material_for_slot(ExportMaterials::Export, 7, &mats), b);
        assert_eq!(material_for_slot(ExportMaterials::Viewport, 1, &mats), b);
        assert_eq!(
            material_for_slot(ExportMaterials::Placeholder, 1, &mats),
            None
        );
        assert_eq!(material_for_slot(ExportMaterials::None, 1, &mats), None);
        assert_eq!(material_for_slot(ExportMaterials::Export, 0, &[]), None);
        assert_eq!(
            material_for_slot(ExportMaterials::Export, NO_MATERIAL_SLOT, &mats),
            None
        );
    }

    #[test]
    fn ushort_colours_are_normalized() {
        let c = AttrColumn {
            values: AttrValues::U16x4(vec![[65535, 0, 32768, 65535]]),
            normalized: true,
        };
        let d = column_data(&c);
        assert!(d.normalized);
        assert_eq!(
            d.component_type(),
            frap_gltf::json::ComponentType::UnsignedShort
        );
        assert_eq!(d.type_, frap_gltf::json::AccessorType::Vec4);
        // FLOAT cannot be normalized.
        let f = column_data(&AttrColumn {
            values: AttrValues::F32x4(vec![[1.0; 4]]),
            normalized: true,
        });
        assert!(!f.normalized);
    }
}
