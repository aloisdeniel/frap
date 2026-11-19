// SPDX-License-Identifier: GPL-3.0-or-later
//! POINTS extractor (`use_mesh_vertices`): one primitive with every vertex that no edge uses,
//! after the triangle and line primitives, in vertex order and without deduplication; material
//! slot 0. The add-on writes no `indices` for it; the mesh gatherer writes every primitive
//! with indices, so this one gets the identity `0..n`, which draws the same points (and which
//! the canonical comparison treats as absent indices). Loose vertices carry no corner.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`create_dots_data_structure`: `verts_in_edge`, `blender_idxs_points`;
//! `primitive_creation_edges_and_points`: `mode: 0`, `material: 0`, no `indices`).

use crate::export::mesh::convert::axis;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_EXTRACTORS, PrimitiveDraft, PrimitiveExtractor, SplitKey,
};

/// glTF `POINTS`.
pub const MODE_POINTS: u32 = 0;

/// `use_mesh_vertices` is honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["use_mesh_vertices"];

/// The vertices no edge uses, ascending.
pub fn loose_vertices(mesh: &frap_mesh::Mesh) -> Vec<u32> {
    let mut in_edge = vec![false; mesh.verts_num()];
    for e in mesh.edges() {
        for v in e {
            if let Some(x) = in_edge.get_mut(*v as usize) {
                *x = true;
            }
        }
    }
    (0..mesh.verts_num() as u32)
        .filter(|&v| !in_edge[v as usize])
        .collect()
}

/// The POINTS primitive of the loose vertices (`None` without any).
pub fn primitive(cx: &MeshExportCx) -> Option<PrimitiveDraft> {
    let vertices = loose_vertices(cx.mesh);
    if vertices.is_empty() {
        return None;
    }
    let positions = cx.mesh.positions();
    Some(PrimitiveDraft {
        mode: MODE_POINTS,
        material_slot: 0,
        positions: vertices
            .iter()
            .map(|&v| axis(positions[v as usize], cx.settings.export_yup))
            .collect(),
        corners: vec![u32::MAX; vertices.len()],
        indices: (0..vertices.len() as u32).collect(),
        vertices,
        ..PrimitiveDraft::default()
    })
}

fn run(cx: &MeshExportCx, _: &SplitKey, out: &mut Vec<PrimitiveDraft>) {
    if !cx.settings.props.use_mesh_vertices.value {
        return;
    }
    out.extend(primitive(cx));
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_EXTRACTORS)]
static LOOSE_POINTS: PrimitiveExtractor = PrimitiveExtractor {
    name: "loose_points",
    order: 300,
    run,
};

#[cfg(test)]
mod tests {
    use frap_mesh::Mesh;
    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;
    use crate::export::mesh::primitives;

    #[test]
    fn loose_vertices_become_points_last() {
        // Triangle 0-1-2, loose edge 3-4, loose vertices 5 and 6.
        let mut m = Mesh::new("P");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [2.0, 0.0, 0.0],
                [3.0, 0.0, 0.0],
                [5.0, 6.0, 7.0],
                [-1.0, 0.0, 0.0],
            ],
            &[[3, 4]],
            &[vec![0, 1, 2]],
            false,
        );
        let mut p = crate::export::settings::default_props();
        p.use_mesh_vertices.value = true;
        p.use_mesh_edges.value = true;
        let s = ExportSettings {
            props: std::sync::Arc::new(p),
            ..ExportSettings::default()
        };
        let main = Main::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let d = primitives::extract(&cx).unwrap();
        assert_eq!(d.iter().map(|p| p.mode).collect::<Vec<_>>(), [4, 1, 0]);
        let pts = &d[2];
        assert_eq!(pts.vertices, [5, 6]);
        assert_eq!(pts.indices, [0, 1]);
        assert_eq!(pts.positions[0], [5.0, 7.0, -6.0]);
        assert!(pts.normals.is_empty());
    }
}
