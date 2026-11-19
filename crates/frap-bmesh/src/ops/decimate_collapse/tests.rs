// SPDX-License-Identifier: GPL-3.0-or-later
//! `decimate_collapse` against Blender 5.2.4: the counts of the DECIMATE modifier (COLLAPSE) on
//! the default primitives, read from `evaluated_get(depsgraph).to_mesh()` (the full geometry
//! comparisons are the golden cases of `tests/golden/cases/modifiers/bevel_decimate/`). The UV
//! and ico spheres are left out: Blender's own face order of those primitives is not stable
//! between runs (D-19), and the collapse order of their many equal-cost edges follows it.

use frap_mesh::Mesh;
use frap_mesh::primitives::cone::{Cone, cone};
use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::grid::{Grid, grid};
use frap_mesh::primitives::monkey::{Monkey, monkey};
use frap_mesh::primitives::torus::{Torus, torus};

use super::*;
use crate::FromMeshParams;

/// The modifier's conversion (`BKE_mesh_to_bmesh_ex`, face and vertex normals).
fn bm_of(me: &Mesh) -> BMesh {
    BMesh::from_mesh(
        me,
        &FromMeshParams {
            use_shapekey: false,
            ..FromMeshParams::default()
        },
    )
}

fn counts(bm: &BMesh) -> (usize, usize, usize) {
    (bm.verts_num(), bm.edges_num(), bm.faces_num())
}

/// `(ratio, use_collapse_triangulate, use_symmetry) -> (verts, edges, faces)` in Blender
/// 5.2.4 (symmetry on X, the modifier's `symmetry_eps` 0.00002).
type Table = [(f32, bool, bool, (usize, usize, usize)); 16];

fn check(name: &str, me: &Mesh, table: &Table) {
    let mut bad = Vec::new();
    for &(ratio, tri, sym, want) in table {
        let mut bm = bm_of(me);
        let i = DecimateCollapseIn {
            do_triangulate: tri,
            symmetry_axis: if sym { 0 } else { -1 },
            symmetry_eps: 0.00002,
            ..DecimateCollapseIn::with_factor(ratio)
        };
        decimate_collapse(&mut bm, &i);
        let problems = bm.validate();
        assert!(
            problems.is_empty(),
            "{name} {ratio} {tri} {sym}: {problems:?}"
        );
        if tri {
            assert!(bm.face_ids().iter().all(|&f| bm.face(f).len == 3));
        }
        if counts(&bm) != want {
            bad.push(format!(
                "{name} ratio={ratio} triangulate={tri} symmetry={sym}: frap {:?}, Blender {want:?}",
                counts(&bm)
            ));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn monkey_counts_equal_blender() {
    check(
        "monkey",
        &monkey(&Monkey::default()),
        &[
            (0.99999, false, false, (506, 1007, 503)),
            (0.99999, false, true, (505, 1006, 503)),
            (0.99999, true, false, (506, 1469, 965)),
            (0.99999, true, true, (505, 1466, 963)),
            (0.5, false, false, (262, 605, 345)),
            (0.5, false, true, (261, 596, 337)),
            (0.5, true, false, (262, 743, 483)),
            (0.5, true, true, (261, 742, 483)),
            (0.25, false, false, (138, 337, 201)),
            (0.25, false, true, (138, 339, 203)),
            (0.25, true, false, (138, 376, 240)),
            (0.25, true, true, (138, 375, 239)),
            (0.1, false, false, (59, 147, 90)),
            (0.1, false, true, (58, 143, 87)),
            (0.1, true, false, (59, 153, 96)),
            (0.1, true, true, (58, 151, 95)),
        ],
    );
}

#[test]
fn cube_counts_equal_blender() {
    check(
        "cube",
        &cube_mesh(&Cube::default()),
        &[
            (0.99999, false, false, (7, 11, 6)),
            (0.99999, false, true, (7, 11, 6)),
            (0.99999, true, false, (7, 15, 10)),
            (0.99999, true, true, (7, 15, 10)),
            (0.5, false, false, (5, 9, 6)),
            (0.5, false, true, (5, 8, 5)),
            (0.5, true, false, (5, 9, 6)),
            (0.5, true, true, (5, 9, 6)),
            (0.25, false, false, (3, 3, 2)),
            (0.25, false, true, (3, 3, 2)),
            (0.25, true, false, (3, 3, 2)),
            (0.25, true, true, (3, 3, 2)),
            (0.1, false, false, (3, 3, 2)),
            (0.1, false, true, (3, 3, 2)),
            (0.1, true, false, (3, 3, 2)),
            (0.1, true, true, (3, 3, 2)),
        ],
    );
}

#[test]
fn grid_counts_equal_blender() {
    check(
        "grid",
        &grid(&Grid::default()),
        &[
            (0.99999, false, false, (120, 219, 100)),
            (0.99999, false, true, (119, 218, 100)),
            (0.99999, true, false, (120, 318, 199)),
            (0.99999, true, true, (119, 316, 198)),
            (0.5, false, false, (63, 119, 57)),
            (0.5, false, true, (63, 124, 62)),
            (0.5, true, false, (63, 162, 100)),
            (0.5, true, true, (63, 162, 100)),
            (0.25, false, false, (36, 66, 31)),
            (0.25, false, true, (33, 70, 38)),
            (0.25, true, false, (36, 85, 50)),
            (0.25, true, true, (33, 82, 50)),
            (0.1, false, false, (17, 31, 15)),
            (0.1, false, true, (14, 31, 18)),
            (0.1, true, false, (17, 36, 20)),
            (0.1, true, true, (14, 33, 20)),
        ],
    );
}

#[test]
fn cone_counts_equal_blender() {
    check(
        "cone",
        &cone(&Cone::default()),
        &[
            (0.99999, false, false, (32, 62, 32)),
            (0.99999, false, true, (31, 60, 31)),
            (0.99999, true, false, (32, 90, 60)),
            (0.99999, true, true, (31, 87, 58)),
            (0.5, false, false, (17, 32, 17)),
            (0.5, false, true, (17, 32, 17)),
            (0.5, true, false, (17, 45, 30)),
            (0.5, true, true, (17, 45, 30)),
            (0.25, false, false, (9, 16, 9)),
            (0.25, false, true, (9, 16, 9)),
            (0.25, true, false, (9, 21, 14)),
            (0.25, true, true, (9, 21, 14)),
            (0.1, false, false, (5, 8, 5)),
            (0.1, false, true, (6, 10, 6)),
            (0.1, true, false, (5, 9, 6)),
            (0.1, true, true, (6, 12, 8)),
        ],
    );
}

#[test]
fn torus_counts_equal_blender() {
    check(
        "torus",
        &torus(&Torus::default()),
        &[
            (0.99999, false, false, (575, 1151, 576)),
            (0.99999, false, true, (574, 1150, 576)),
            (0.99999, true, false, (575, 1725, 1150)),
            (0.99999, true, true, (574, 1722, 1148)),
            (0.5, false, false, (288, 719, 431)),
            (0.5, false, true, (288, 694, 406)),
            (0.5, true, false, (288, 864, 576)),
            (0.5, true, true, (288, 864, 576)),
            (0.25, false, false, (144, 397, 253)),
            (0.25, false, true, (143, 370, 227)),
            (0.25, true, false, (144, 432, 288)),
            (0.25, true, true, (143, 429, 286)),
            (0.1, false, false, (57, 166, 109)),
            (0.1, false, true, (57, 161, 104)),
            (0.1, true, false, (57, 171, 114)),
            (0.1, true, true, (57, 171, 114)),
        ],
    );
}

#[test]
fn cylinder_counts_equal_blender() {
    check(
        "cylinder",
        &cylinder(&Cylinder::default()),
        &[
            (0.99999, false, false, (63, 95, 34)),
            (0.99999, false, true, (62, 94, 34)),
            (0.99999, true, false, (63, 183, 122)),
            (0.99999, true, true, (62, 180, 120)),
            (0.5, false, false, (33, 50, 19)),
            (0.5, false, true, (32, 50, 20)),
            (0.5, true, false, (33, 93, 62)),
            (0.5, true, true, (32, 90, 60)),
            (0.25, false, false, (17, 26, 11)),
            (0.25, false, true, (17, 27, 12)),
            (0.25, true, false, (17, 45, 30)),
            (0.25, true, true, (17, 45, 30)),
            (0.1, false, false, (8, 16, 10)),
            (0.1, false, true, (8, 15, 9)),
            (0.1, true, false, (8, 18, 12)),
            (0.1, true, true, (8, 18, 12)),
        ],
    );
}

#[test]
fn ratio_one_joins_back_the_convex_pairs() {
    // `BM_mesh_decimate_collapse` with factor 1 collapses nothing; Suzanne's quads come back,
    // except the three whose triangle pairs fail the convexity test (as in Blender, whose
    // ratio 0.99999 collapses a single edge from these 503 faces).
    let mut bm = bm_of(&monkey(&Monkey::default()));
    decimate_collapse(&mut bm, &DecimateCollapseIn::with_factor(1.0));
    assert_eq!(counts(&bm), (507, 1008, 503));
    assert!(bm.validate().is_empty(), "{:?}", bm.validate());
}

#[test]
fn zero_weights_pin_vertices() {
    // Vertices with weight 0 never move: their edges stay off the heap.
    let me = monkey(&Monkey::default());
    let mut bm = bm_of(&me);
    let n = bm.verts_num();
    let weights: Vec<f32> = (0..n).map(|k| if k % 2 == 0 { 0.0 } else { 1.0 }).collect();
    let pinned: Vec<(VertId, V3)> = bm
        .vert_ids()
        .into_iter()
        .enumerate()
        .filter(|(k, _)| k % 2 == 0)
        .map(|(_, v)| (v, bm.vert(v).co))
        .collect();
    let i = DecimateCollapseIn {
        vweights: Some(weights),
        ..DecimateCollapseIn::with_factor(0.2)
    };
    decimate_collapse(&mut bm, &i);
    assert!(bm.validate().is_empty(), "{:?}", bm.validate());
    for (v, co) in pinned {
        if bm.verts.contains(v.0) {
            assert_eq!(bm.vert(v).co, co);
        }
    }
}
