// SPDX-License-Identifier: GPL-3.0-or-later
//! LINES extractor (`use_mesh_edges`): one primitive with every loose edge (an edge no face
//! uses), after the triangle primitives. Its vertices are the edges' vertices, deduplicated and
//! sorted by vertex index (the add-on's records hold only the vertex index and POINT-domain
//! data, so `fast_structured_np_unique` sorts them by index); `indices` are each edge's two
//! vertices in edge order; material slot 0 (the add-on's `'material': 0`). Loose vertices carry
//! no corner (`corners` is `u32::MAX`), so only per-vertex attributes are written for them.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`create_dots_data_structure`: `loose_edges = [e for e in edges if e.is_loose]`,
//! `blender_idxs_edges`; `primitive_creation_edges_and_points`: `mode: 1`, `material: 0`).

use crate::export::mesh::convert::axis;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_EXTRACTORS, PrimitiveDraft, PrimitiveExtractor, SplitKey,
};

/// glTF `LINES`.
pub const MODE_LINES: u32 = 1;

/// `use_mesh_edges` is honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["use_mesh_edges"];

/// `Mesh.edges[i].is_loose` for every edge: no corner uses it.
pub fn loose_edge_flags(mesh: &frap_mesh::Mesh) -> Vec<bool> {
    let mut loose = vec![true; mesh.edges_num()];
    for &e in mesh.corner_edges() {
        if let Some(l) = loose.get_mut(e as usize) {
            *l = false;
        }
    }
    loose
}

/// The LINES primitive of the loose edges (`None` without any).
pub fn primitive(cx: &MeshExportCx) -> Option<PrimitiveDraft> {
    let loose = loose_edge_flags(cx.mesh);
    let idxs: Vec<u32> = cx
        .mesh
        .edges()
        .iter()
        .zip(&loose)
        .filter(|(_, l)| **l)
        .flat_map(|(e, _)| [e[0] as u32, e[1] as u32])
        .collect();
    if idxs.is_empty() {
        return None;
    }
    let mut vertices = idxs.clone();
    vertices.sort_unstable();
    vertices.dedup();
    let indices = idxs
        .iter()
        .map(|v| vertices.binary_search(v).map_or(0, |i| i as u32))
        .collect();
    let positions = cx.mesh.positions();
    Some(PrimitiveDraft {
        mode: MODE_LINES,
        material_slot: 0,
        positions: vertices
            .iter()
            .map(|&v| axis(positions[v as usize], cx.settings.export_yup))
            .collect(),
        corners: vec![u32::MAX; vertices.len()],
        vertices,
        indices,
        ..PrimitiveDraft::default()
    })
}

fn run(cx: &MeshExportCx, _: &SplitKey, out: &mut Vec<PrimitiveDraft>) {
    if !cx.settings.props.use_mesh_edges.value {
        return;
    }
    out.extend(primitive(cx));
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_EXTRACTORS)]
static LOOSE_EDGES: PrimitiveExtractor = PrimitiveExtractor {
    name: "loose_edges",
    order: 200,
    run,
};

#[cfg(test)]
mod tests {
    use frap_mesh::Mesh;
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;
    use crate::export::mesh::primitives;

    pub(crate) fn with_edges(on: bool) -> ExportSettings {
        let mut p = crate::export::settings::default_props();
        p.use_mesh_edges.value = on;
        ExportSettings {
            props: std::sync::Arc::new(p),
            ..ExportSettings::default()
        }
    }

    #[test]
    fn loose_edges_become_lines_after_triangles() {
        // A triangle (0, 1, 2) plus loose edges 5-3 and 3-4.
        let mut m = Mesh::new("L");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [2.0, 2.0, 1.0],
            ],
            &[[5, 3], [3, 4]],
            &[vec![0, 1, 2]],
            false,
        );
        let main = Main::default();
        let s = with_edges(true);
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert_eq!(d.iter().map(|p| p.mode).collect::<Vec<_>>(), [4, 1]);
        let l = &d[1];
        assert_eq!(l.vertices, [3, 4, 5]);
        let edges = loose_edge_flags(&m);
        assert_eq!(edges.iter().filter(|l| **l).count(), 2);
        // Every index pair is one loose edge, in edge order.
        let mut pairs: Vec<[u32; 2]> = l
            .indices
            .chunks(2)
            .map(|c| [l.vertices[c[0] as usize], l.vertices[c[1] as usize]])
            .collect();
        pairs.sort_unstable();
        assert!(pairs.contains(&[3, 4]) && (pairs.contains(&[3, 5]) || pairs.contains(&[5, 3])));
        assert_eq!(l.material_slot, 0);
        // Corner attributes are not written for loose geometry.
        assert!(l.normals.is_empty() && l.texcoords.is_empty());
        assert_eq!(l.positions[2], [2.0, 1.0, -2.0]);
        // Off by default.
        let s = ExportSettings::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        assert_eq!(primitives::extract(&cx).unwrap().len(), 1);
    }
}
