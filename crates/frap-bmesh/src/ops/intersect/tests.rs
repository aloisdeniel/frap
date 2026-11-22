// SPDX-License-Identifier: GPL-3.0-or-later
//! `BM_mesh_intersect` regressions that need the whole cut (the goldens in
//! `tests/golden/cases/edit/intersect_float/` compare the same inputs with Blender 5.2.4).

use super::tessellate::mesh_calc_tessellation;
use super::{BooleanMode, IntersectIn, intersect};
use crate::types::CreateFlag;
use crate::{BMesh, ElemFlag, FaceId};

/// The default cube with `primitive_cube_add(size=1.2, location=(0.7, 0.4, 0.3),
/// rotation=(0.3, 0.2, 0.1))` added in Edit Mode (selected), as Blender 5.2.4 builds it.
/// (Python's `repr` of the f32 values, 10 significant digits.)
#[allow(clippy::excessive_precision)]
const VERTS: [[f32; 3]; 16] = [
    [1.0, 1.0, 1.0],
    [1.0, 1.0, -1.0],
    [1.0, -1.0, 1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [-1.0, -1.0, -1.0],
    [6.061_494_35e-3, -6.750_354_171e-2, -3.163_521_886e-1],
    [2.680_823_207e-1, -3.976_186_216e-1, 8.072_000_146e-1],
    [-3.828_686_476e-2, 1.080_206_513, 3.120_321_035e-2],
    [2.237_339_616e-1, 7.500_914_335e-1, 1.154_755_354],
    [1.176_265_955, 4.990_854_859e-2, -5.547_553_897e-1],
    [1.438_286_781, -2.802_065_313e-1, 5.687_968_135e-1],
    [1.131_917_715, 1.197_618_604, -2.071_999_907e-1],
    [1.393_938_541, 8.675_035_238e-1, 9.163_522_124e-1],
];
const FACES: [[usize; 4]; 12] = [
    [0, 4, 6, 2],
    [3, 2, 6, 7],
    [7, 6, 4, 5],
    [5, 1, 3, 7],
    [1, 0, 2, 3],
    [5, 4, 0, 1],
    [8, 9, 11, 10],
    [10, 11, 15, 14],
    [14, 15, 13, 12],
    [12, 13, 9, 8],
    [10, 14, 12, 8],
    [15, 11, 9, 13],
];

fn two_cubes() -> BMesh {
    let mut bm = BMesh::new();
    let vs: Vec<_> = VERTS
        .iter()
        .map(|&co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
    for (i, f) in FACES.iter().enumerate() {
        let ids: Vec<_> = f.iter().map(|&k| vs[k]).collect();
        let fid = bm
            .face_create_verts(&ids, None, CreateFlag::empty(), true)
            .expect("face");
        if i >= 6 {
            bm.face_select_set(fid, true);
        }
    }
    bm.normals_update();
    bm
}

/// `mesh.intersect(separate_mode=…, threshold=eps)` (`edbm_intersect_exec`).
fn knife(bm: &mut BMesh, eps: f32, use_separate: bool) {
    let side = |bm: &BMesh, f: FaceId| -> i32 {
        let fl = bm.face(f).flag;
        if fl.contains(ElemFlag::HIDDEN) {
            -1
        } else if fl.contains(ElemFlag::SELECT) {
            1
        } else {
            0
        }
    };
    let looptris = mesh_calc_tessellation(bm);
    let out = intersect(
        bm,
        &IntersectIn {
            looptris: &looptris,
            test_fn: &side,
            use_self: false,
            use_separate,
            use_dissolve: true,
            use_island_connect: true,
            use_partial_connect: true,
            use_edge_tag: true,
            boolean_mode: BooleanMode::None,
            eps,
        },
    )
    .expect("intersect");
    assert!(out.changed);
}

/// At thresholds near the RNA maximum (0.01) the cut snaps onto corners and the island
/// connection takes the partial-connect split (`bm_face_split_edgenet_partial_connect`): the
/// edges moved onto the temporary vertex keep loops on the original one until the splice, so
/// the splice check used to walk `BM_LOOPS_OF_VERT` forever. Blender 5.2.4: V26 E46 F21
/// (separate NONE), V37 E57 F21 (ALL); 0.005 does not snap (V25 E44 F21).
#[test]
fn partial_connect_split_at_large_thresholds() {
    for (eps, sep, counts) in [
        (0.009, false, (26, 46, 21)),
        (0.0095, false, (26, 46, 21)),
        (0.01, false, (26, 46, 21)),
        (0.009, true, (37, 57, 21)),
        (0.01, true, (37, 57, 21)),
        (0.005, false, (25, 44, 21)),
    ] {
        let mut bm = two_cubes();
        knife(&mut bm, eps, sep);
        assert_eq!(
            (bm.verts_num(), bm.edges_num(), bm.faces_num()),
            counts,
            "threshold {eps}, separate {sep}"
        );
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
    }
}

/// `BM_LOOPS_OF_VERT` on a vertex whose corner lost its edge to a disk-only swap
/// (`BM_vert_separate_tested_edges`, the state between the partial-connect split and the
/// splice) ends instead of walking another vertex's disk; the splice makes the mesh valid again.
#[test]
fn vert_loops_ends_on_a_disk_only_swap() {
    let mut bm = BMesh::new();
    let co = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [0.0, -1.0, 0.0],
        [-1.0, 0.0, 0.0],
        [-1.0, -1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let v: Vec<_> = co
        .iter()
        .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
        .collect();
    for f in [[0, 1, 2, 3], [1, 0, 5, 4], [0, 6, 7, 5]] {
        let ids: Vec<_> = f.iter().map(|&k| v[k]).collect();
        bm.face_create_verts(&ids, None, CreateFlag::empty(), true)
            .expect("face");
    }
    let corners = bm.vert_loops(v[0]).len();
    assert_eq!(corners, 3);
    let e = bm.edge_exists(v[0], v[1]).expect("edge");
    let v_split = bm.vert_create(co[0], None, CreateFlag::empty());
    bm.disk_vert_replace(e, v_split, v[0]);
    let loops = bm.vert_loops(v[0]);
    assert!(loops.len() <= corners, "{loops:?}");
    assert!(bm.vert_loops(v_split).is_empty());
    assert!(bm.vert_splice(v[0], v_split));
    assert_eq!(bm.vert_loops(v[0]).len(), corners);
    assert!(bm.validate().is_empty(), "{:?}", bm.validate());
}
