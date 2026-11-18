// SPDX-License-Identifier: GPL-3.0-or-later
//! WELD (`WeldModifier`): merges vertices closer than `merge_threshold`, either every pair
//! (`ALL`, a kd-tree, `mesh_merge_by_distance_all`) or only along edges (`CONNECTED`,
//! `mesh_merge_by_distance_connected`, optionally loose edges only), restricted to a vertex
//! group (`vertex_group`, `invert_vertex_group`). Merged vertices, edges and corners mix their
//! attributes; nothing merged returns the input unchanged.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_weld.cc (Blender 5.2.4). The merge
//! itself is [`merge`] (`geometry/intern/mesh_merge_verts.cc`), shared with MIRROR and ARRAY;
//! [`util`] holds the attribute helpers of every WP 4.2 constructive modifier.

pub(crate) mod merge;
pub(crate) mod util;
#[cfg(test)]
mod vgroup_tests;

use crate::prelude::*;

use self::util::{defgroup_index, defvert_find_weight};

#[modifier(
    WELD,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Weld(s) = &md.settings else {
        return Ok(mesh);
    };
    Ok(weld(&mesh, s).unwrap_or(mesh))
}

/// `calculate_weld`: the welded mesh, `None` when nothing merges.
pub(crate) fn weld(
    mesh: &Mesh,
    s: &frap_scene::generated::modifier_settings::WeldSettings,
) -> Option<Mesh> {
    let invert = s.invert_vertex_group;
    // `get_vertex_group`: a known group name on a mesh with weights.
    let group = defgroup_index(mesh, &s.vertex_group).zip(mesh.deform_verts());
    let selection: Option<Vec<bool>> = group.map(|(g, dv)| {
        dv.iter()
            .map(|d| (defvert_find_weight(d, g) > 0.0) != invert)
            .collect()
    });
    match s.mode.as_str() {
        "CONNECTED" => {
            let sel = selection.unwrap_or_else(|| vec![true; mesh.verts_num()]);
            merge::mesh_merge_by_distance_connected(mesh, &sel, s.merge_threshold, s.loose_edges)
        }
        _ => merge::mesh_merge_by_distance_all(mesh, selection.as_deref(), s.merge_threshold),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::generated::modifier_settings::WeldSettings;

    fn two_cubes_touching() -> Mesh {
        // A cube and its copy shifted by 2 on X share the face at x = 1 (4 coincident verts).
        let a = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let mut pos = a.positions().to_vec();
        let n = pos.len() as i32;
        pos.extend(a.positions().iter().map(|p| [p[0] + 2.0, p[1], p[2]]));
        let mut faces: Vec<Vec<i32>> = Vec::new();
        for f in 0..a.faces_num() {
            faces.push(a.face(f).map(|c| a.corner_verts()[c]).collect());
        }
        for f in 0..a.faces_num() {
            faces.push(a.face(f).map(|c| a.corner_verts()[c] + n).collect());
        }
        let mut m = Mesh::new("m");
        m.from_pydata(&pos, &[], &faces, false);
        m
    }

    #[test]
    fn all_mode_merges_coincident_vertices_and_faces() {
        let m = two_cubes_touching();
        let out = weld(&m, &WeldSettings::default()).expect("merges");
        // 16 → 12 vertices; the two coincident faces collapse into one.
        assert_eq!(out.verts_num(), 12);
        assert_eq!(out.edges_num(), 20);
        assert_eq!(out.faces_num(), 11);
    }

    #[test]
    fn connected_mode_needs_an_edge() {
        let m = two_cubes_touching();
        let s = WeldSettings {
            mode: "CONNECTED".into(),
            ..WeldSettings::default()
        };
        assert!(weld(&m, &s).is_none(), "the touching cubes share no edge");
    }

    /// Against Blender: the inputs of `tests/golden/cases/modifiers/a/weld/*.py`.
    #[test]
    fn matches_blender_fixtures() {
        use super::util::fixture::assert_evaluated;
        use frap_mesh::primitives::grid::{Grid, grid};
        let g = grid(&Grid {
            x_subdivisions: 5,
            y_subdivisions: 4,
            size: 2.0,
            ..Default::default()
        });
        let s = WeldSettings {
            merge_threshold: 0.6,
            ..WeldSettings::default()
        };
        assert_evaluated(
            "weld/all_grid_collapse",
            "Grid",
            &weld(&g, &s).unwrap(),
            1e-5,
        );
        let g = grid(&Grid {
            x_subdivisions: 6,
            y_subdivisions: 3,
            size: 2.0,
            ..Default::default()
        });
        let s = WeldSettings {
            mode: "CONNECTED".into(),
            merge_threshold: 0.45,
            ..WeldSettings::default()
        };
        assert_evaluated("weld/connected_grid", "Grid", &weld(&g, &s).unwrap(), 1e-5);
        let mut w = Mesh::new("W");
        w.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 0.05, 0.0],
                [2.0, 0.0, 0.0],
                [2.05, 0.0, 0.0],
                [2.1, 0.0, 0.0],
                [3.0, 0.0, 0.0],
            ],
            &[[3, 4], [4, 5], [5, 6]],
            &[vec![0, 1, 2]],
            false,
        );
        for (case, loose) in [
            ("weld/connected_loose_edges", true),
            ("weld/connected_all_edges", false),
        ] {
            let s = WeldSettings {
                mode: "CONNECTED".into(),
                merge_threshold: 0.1,
                loose_edges: loose,
                ..WeldSettings::default()
            };
            assert_evaluated(case, "W", &weld(&w, &s).unwrap(), 1e-5);
        }
    }
}
