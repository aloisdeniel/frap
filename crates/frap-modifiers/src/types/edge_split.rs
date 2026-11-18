// SPDX-License-Identifier: GPL-3.0-or-later
//! EDGE_SPLIT (`EdgeSplitModifier`): splits the edges whose faces meet at more than
//! `split_angle` (`use_edge_angle`; edges with three or more faces always) and the edges marked
//! sharp (`use_edge_sharp`), through a BMesh (`BM_mesh_edgesplit`). With neither option the
//! mesh passes unchanged.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_edgesplit.cc (Blender 5.2.4).

use frap_bmesh::ops::split_edges::mesh_edgesplit;
use frap_bmesh::{BMesh, ElemFlag, FromMeshParams, ToMeshParams};
use frap_scene::generated::modifier_settings::EdgeSplitSettings;

use super::weld::util::dot;
use crate::prelude::*;

#[modifier(
    EDGE_SPLIT,
    kind = Constructive,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::EdgeSplit(s) = &md.settings else {
        return Ok(mesh);
    };
    if !(s.use_edge_angle || s.use_edge_sharp) {
        return Ok(mesh);
    }
    Ok(do_edge_split(&mesh, s))
}

// Ported from blender/source/blender/modifiers/intern/MOD_edgesplit.cc (doEdgeSplit).
/// `doEdgeSplit`.
pub(crate) fn do_edge_split(mesh: &Mesh, s: &EdgeSplitSettings) -> Mesh {
    let threshold = libm::cosf(s.split_angle + 0.000_000_175);
    let do_split_angle = s.use_edge_angle && s.split_angle < std::f32::consts::PI;
    let do_split_all = do_split_angle && s.split_angle < f32::EPSILON;
    let calc_face_normals = do_split_angle && !do_split_all;
    let mut bm = BMesh::from_mesh(
        mesh,
        &FromMeshParams {
            calc_face_normal: calc_face_normals,
            calc_vert_normal: false,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );
    if do_split_angle {
        for e in bm.edge_ids() {
            // An edge with two face users.
            let Some(l1) = bm.edge(e).l else {
                continue;
            };
            let l2 = bm.lp(l1).radial_next;
            if l2 == l1 {
                continue;
            }
            let three_plus = l1 != bm.lp(l2).radial_next;
            let n1 = bm.face(bm.lp(l1).f).no;
            let n2 = bm.face(bm.lp(l2).f).no;
            if three_plus || do_split_all || dot(n1, n2) < threshold {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
        }
    }
    if s.use_edge_sharp {
        for e in bm.edge_ids() {
            // Two or more edge users (`e->l->next != e->l` holds for every face loop).
            if bm.edge(e).l.is_some() && !bm.edge(e).flag.contains(ElemFlag::SMOOTH) {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
        }
    }
    mesh_edgesplit(&mut bm, false, true, false);
    bm.to_mesh(mesh, &ToMeshParams::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cube_splits_into_six_quads() {
        let cube = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let out = do_edge_split(&cube, &EdgeSplitSettings::default());
        assert_eq!(out.verts_num(), 24);
        assert_eq!(out.edges_num(), 24);
        assert_eq!(out.faces_num(), 6);
        let s = EdgeSplitSettings {
            split_angle: 1.6,
            use_edge_sharp: false,
            ..EdgeSplitSettings::default()
        };
        let out = do_edge_split(&cube, &s);
        assert_eq!(out.verts_num(), 8, "90° is under the split angle");
    }


    /// Against Blender: the inputs of `tests/golden/cases/modifiers/a/edge_split/angle.py`.
    #[test]
    fn matches_blender_fixtures() {
        use super::super::weld::util::fixture::assert_evaluated;
        use frap_mesh::primitives::grid::{Grid, grid};
        use frap_mesh::primitives::monkey::{Monkey, monkey};
        let cube = super::super::weld::util::fixture::factory_cube();
        let out = do_edge_split(&cube, &EdgeSplitSettings::default());
        assert_evaluated("edge_split/angle", "Cube", &out, 1e-5);
        let suzanne = monkey(&Monkey {
            size: 2.0,
            ..Default::default()
        });
        let s = EdgeSplitSettings {
            split_angle: 0.349_065_85,
            ..EdgeSplitSettings::default()
        };
        assert_evaluated("edge_split/angle", "Suzanne", &do_edge_split(&suzanne, &s), 1e-5);
        let g = grid(&Grid {
            x_subdivisions: 3,
            y_subdivisions: 2,
            size: 2.0,
            ..Default::default()
        });
        let s = EdgeSplitSettings {
            split_angle: 0.0,
            ..EdgeSplitSettings::default()
        };
        assert_evaluated("edge_split/angle", "Grid", &do_edge_split(&g, &s), 1e-5);
    }
}
