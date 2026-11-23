// SPDX-License-Identifier: GPL-3.0-or-later
//! TRIANGLES extractor: one primitive per used material index (ascending), from Blender's loop
//! triangles, each corner split on `(vertex index, every SplitKey word)` and deduplicated in
//! the byte order of the add-on's `fast_structured_np_unique`; `corners`, `vertices`,
//! `positions` (Y-up) and `indices` filled.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`primitive_split`, `primitive_creation_not_shared`, `__set_positions_attribute`) and
//! blender/com/gltf2_blender_utils.py (`fast_structured_np_unique`).

use std::collections::BTreeMap;

use frap_mesh::{AttrData, Domain, Mesh};

use crate::export::ExportMaterials;
use crate::export::mesh::convert::{axis, record_cmp};
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_EXTRACTORS, PrimitiveDraft, PrimitiveExtractor, SplitKey,
};
use crate::export::mesh::primitives::NO_MATERIAL_SLOT;

/// glTF `TRIANGLES`.
pub const MODE_TRIANGLES: u32 = 4;

/// `loop_triangles[i].material_index` as the add-on reads it (`np.uint32`): the face's
/// `material_index` attribute (INT on faces), 0 without one.
fn tri_material_indices(mesh: &Mesh) -> Vec<u32> {
    let faces = mesh.looptri_faces();
    let mi = mesh
        .attributes()
        .get("material_index")
        .and_then(|a| match (&a.domain, &a.data) {
            (Domain::Face, AttrData::Int(v)) => Some(v.clone()),
            _ => None,
        });
    match mi {
        Some(v) => faces
            .iter()
            .map(|f| v.get(*f as usize).copied().unwrap_or(0) as u32)
            .collect(),
        None => vec![0; faces.len()],
    }
}

/// One primitive from the corners of its triangles (`corners.len() % 3 == 0`).
pub fn primitive(
    cx: &MeshExportCx,
    key: &SplitKey,
    material_slot: usize,
    corners: &[u32],
) -> PrimitiveDraft {
    let cv = cx.mesh.corner_verts();
    let stride = 1 + key.stride();
    let records: Vec<u32> = corners
        .iter()
        .flat_map(|&c| std::iter::once(cv[c as usize] as u32).chain(key.corner(c as usize)))
        .collect();
    let rec = |i: usize| &records[i * stride..(i + 1) * stride];
    let mut order: Vec<usize> = (0..corners.len()).collect();
    // Stable: the representative corner of a vertex is its first corner in triangle order.
    order.sort_by(|&a, &b| record_cmp(rec(a), rec(b)));
    let mut indices = vec![0u32; corners.len()];
    let mut reps: Vec<usize> = Vec::new();
    for i in order {
        frap_math::budget::check();
        if reps.last().is_none_or(|&r| rec(r) != rec(i)) {
            reps.push(i);
        }
        indices[i] = (reps.len() - 1) as u32;
    }
    let positions = cx.mesh.positions();
    let vertices: Vec<u32> = reps.iter().map(|&i| rec(i)[0]).collect();
    PrimitiveDraft {
        mode: MODE_TRIANGLES,
        material_slot,
        positions: vertices
            .iter()
            .map(|&v| axis(positions[v as usize], cx.settings.export_yup))
            .collect(),
        corners: reps.iter().map(|&i| corners[i]).collect(),
        vertices,
        indices,
        ..PrimitiveDraft::default()
    }
}

fn run(cx: &MeshExportCx, key: &SplitKey, out: &mut Vec<PrimitiveDraft>) {
    let looptris = cx.mesh.looptris();
    if looptris.is_empty() {
        return;
    }
    // Buckets by material index, ascending (`np.unique`); one bucket for 'NONE'.
    let mut buckets: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    if cx.settings.export_materials == ExportMaterials::None {
        buckets.insert(u32::MAX, looptris.iter().flatten().copied().collect());
    } else {
        for (t, mi) in looptris.iter().zip(tri_material_indices(cx.mesh)) {
            buckets.entry(mi).or_default().extend_from_slice(t);
        }
    }
    let slot_of = |mi: u32| {
        if cx.settings.export_materials == ExportMaterials::None {
            NO_MATERIAL_SLOT
        } else {
            mi as usize
        }
    };
    if cx.settings.props.export_shared_accessors.value {
        // `primitive_creation_shared` (WP 8.12): the dots of every corner deduplicated
        // together (`fast_structured_np_unique(self.dots, return_inverse=True)`), each
        // primitive indexing that one vertex set. Every draft carries the whole set; the mesh
        // gatherer writes its accessors once.
        let all: Vec<u32> = (0..cx.mesh.corners_num() as u32).collect();
        let shared = primitive(cx, key, 0, &all);
        for (mi, corners) in buckets {
            let mut d = shared.clone();
            d.material_slot = slot_of(mi);
            d.indices = corners
                .iter()
                .map(|&c| shared.indices[c as usize])
                .collect();
            d.shared_vertices = true;
            out.push(d);
        }
        return;
    }
    for (mi, corners) in buckets {
        out.push(primitive(cx, key, slot_of(mi), &corners));
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_EXTRACTORS)]
static TRIANGLES: PrimitiveExtractor = PrimitiveExtractor {
    name: "triangles",
    order: 100,
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_mesh::Attribute;
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;

    fn quad_pair() -> Mesh {
        // Two quads sharing an edge: 6 vertices, 2 faces.
        let mut m = Mesh::new("Q");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 0.0, 0.0],
                [2.0, 1.0, 0.0],
            ],
            &[],
            &[vec![0, 1, 2, 3], vec![1, 4, 5, 2]],
            false,
        );
        m
    }

    fn extract(mesh: &Mesh, settings: &ExportSettings) -> Vec<PrimitiveDraft> {
        let main = Main::default();
        let cx = MeshExportCx::new(&main, mesh, None, settings);
        let mut out = Vec::new();
        run(&cx, &SplitKey::new(mesh.corners_num()), &mut out);
        out
    }

    #[test]
    fn looptris_and_dedup_without_key() {
        let m = quad_pair();
        let d = extract(&m, &ExportSettings::default());
        assert_eq!(d.len(), 1);
        let p = &d[0];
        assert_eq!(p.mode, 4);
        // Without any key column, corners merge per vertex: 6 vertices, sorted by index.
        assert_eq!(p.vertices, [0, 1, 2, 3, 4, 5]);
        // Quads split (0,1,2),(0,2,3) like Blender's looptris.
        assert_eq!(p.indices, [0, 1, 2, 0, 2, 3, 1, 4, 5, 1, 5, 2]);
        // Y-up positions.
        assert_eq!(p.positions[3], [0.0, 0.0, -1.0]);
        assert_eq!(p.corners.len(), 6);
    }

    #[test]
    fn split_on_key_words() {
        let m = quad_pair();
        let settings = ExportSettings::default();
        let main = Main::default();
        let cx = MeshExportCx::new(&main, &m, None, &settings);
        // A key that differs per face: every corner of face 1 gets word 1.
        let mut key = SplitKey::new(m.corners_num());
        let words: Vec<u32> = (0..m.corners_num()).map(|c| (c >= 4) as u32).collect();
        key.push("test", 1, words);
        let mut out = Vec::new();
        run(&cx, &key, &mut out);
        // Vertices 1 and 2 are shared by both faces: 8 glTF vertices.
        assert_eq!(out[0].vertices.len(), 8);
        // 8-byte records sort as little-endian u64: key word first, then vertex index.
        assert_eq!(out[0].vertices, [0, 1, 2, 3, 1, 2, 4, 5]);
    }

    #[test]
    fn one_primitive_per_used_material_index() {
        let mut m = quad_pair();
        m.attributes_mut().insert(Attribute {
            name: "material_index".into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new(vec![3, 1])),
        });
        let d = extract(&m, &ExportSettings::default());
        assert_eq!(
            d.iter().map(|p| p.material_slot).collect::<Vec<_>>(),
            [1, 3]
        );
        assert_eq!(d[0].vertices, [1, 2, 4, 5]);
        let none = ExportSettings {
            export_materials: ExportMaterials::None,
            ..ExportSettings::default()
        };
        let d = extract(&m, &none);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].material_slot, NO_MATERIAL_SLOT);
    }

    #[test]
    fn no_faces_no_primitive() {
        let m = Mesh::new("Empty");
        assert!(extract(&m, &ExportSettings::default()).is_empty());
    }
}
