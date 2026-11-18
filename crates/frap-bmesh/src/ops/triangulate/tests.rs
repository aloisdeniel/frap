// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure unit tests of the WP 2.10 bmesh ops that need no other W2 op (split, split_edges,
//! triangulate + beautify, join_triangles, poke, smooth_vert, wireframe): counts, Blender's
//! documented results ([MESH §4.11–4.14]) and `validate` after each op. Behaviour against
//! Blender is covered by the goldens in `tests/golden/cases/edit/misc/`.

use crate::ops::join_triangles::{JoinTrianglesIn, join_triangles};
use crate::ops::poke::{PokeIn, poke};
use crate::ops::smooth_vert::{SmoothVertIn, smooth_vert};
use crate::ops::split::{SplitIn, split};
use crate::ops::split_edges::{SplitEdgesIn, split_edges};
use crate::ops::triangulate::{TriangulateIn, beautify, triangulate};
use crate::ops::wireframe::{WireframeIn, wireframe};
use crate::types::CreateFlag;
use crate::{BMElem, BMesh, FaceId, VertId};

/// Blender's default cube (`primitive_cube_add`, size 2): vertex and face order of
/// `spec/blender-5.2/primitives/cube.json`, normals updated.
fn cube() -> BMesh {
    let mut bm = BMesh::new();
    let co = [
        [-1.0, -1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, 1.0, 1.0],
        [1.0, -1.0, -1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, -1.0],
        [1.0, 1.0, 1.0],
    ];
    let v: Vec<VertId> = co
        .iter()
        .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
        .collect();
    for f in [
        [0, 1, 3, 2],
        [2, 3, 7, 6],
        [6, 7, 5, 4],
        [4, 5, 1, 0],
        [2, 6, 4, 0],
        [7, 3, 1, 5],
    ] {
        let vs: Vec<VertId> = f.iter().map(|&i| v[i]).collect();
        bm.face_create_verts(&vs, None, CreateFlag::empty(), true)
            .unwrap();
    }
    bm.normals_update();
    bm
}

/// A regular `n`-gon of radius 1 in the XY plane.
fn ngon(n: usize) -> BMesh {
    let mut bm = BMesh::new();
    let v: Vec<VertId> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            bm.vert_create([libm::cosf(a), libm::sinf(a), 0.0], None, CreateFlag::empty())
        })
        .collect();
    bm.face_create_verts(&v, None, CreateFlag::empty(), true)
        .unwrap();
    bm.normals_update();
    bm
}

fn valid(bm: &BMesh) {
    let errs = bm.validate();
    assert!(errs.is_empty(), "invalid BMesh: {errs:?}");
}

fn counts(bm: &BMesh) -> (usize, usize, usize) {
    (bm.verts_num(), bm.edges_num(), bm.faces_num())
}

fn face_vert_indices(bm: &BMesh, f: FaceId) -> Vec<usize> {
    bm.face_verts(f).into_iter().map(|v| v.slot()).collect()
}

#[test]
fn triangulate_fixed_splits_corners_0_2() {
    let mut bm = cube();
    let out = triangulate(
        &mut bm,
        &TriangulateIn {
            faces: vec![FaceId(0)],
            quad_method: "FIXED",
            ngon_method: "BEAUTY",
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (8, 13, 7));
    // Face 0 is [0, 1, 3, 2]: the first triangle [0, 1, 3] is appended (slot 6), the last
    // [0, 3, 2] is swapped into the face's slot.
    assert_eq!(face_vert_indices(&bm, FaceId(6)), [0, 1, 3]);
    assert_eq!(face_vert_indices(&bm, FaceId(0)), [0, 3, 2]);
    assert_eq!(out.faces, [FaceId(0), FaceId(6)]);
    assert_eq!(out.face_map, [(FaceId(0), FaceId(0)), (FaceId(6), FaceId(0))]);
}

#[test]
fn triangulate_whole_cube_reuses_freed_slots() {
    let mut bm = cube();
    let faces = bm.face_ids();
    triangulate(
        &mut bm,
        &TriangulateIn {
            faces,
            ..TriangulateIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (8, 18, 12));
    // Each face's freed scratch slot is taken by the next face's first triangle: slots stay
    // dense but for the last face's scratch slot.
    assert_eq!(bm.faces.capacity(), 13);
    assert!(!bm.faces.contains(12));
    assert!(bm.face_ids().iter().all(|&f| bm.face(f).len == 3));
}

#[test]
fn triangulate_beauty_rejects_folding_diagonal() {
    // A quad folded along 0-2: BEAUTY (and is_quad_flip_v3) must split along 1-3.
    let mut bm = BMesh::new();
    let v: Vec<VertId> = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.2, 0.3, 0.0],
    ]
    .iter()
    .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
    .collect();
    bm.face_create_verts(&v, None, CreateFlag::empty(), true)
        .unwrap();
    bm.normals_update();
    triangulate(
        &mut bm,
        &TriangulateIn {
            faces: vec![FaceId(0)],
            ..TriangulateIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert!(bm.edge_exists(v[1], v[3]).is_some());
    assert!(bm.edge_exists(v[0], v[2]).is_none());
}

#[test]
fn triangulate_ngon_keeps_area_and_winding() {
    for method in ["BEAUTY", "EAR_CLIP"] {
        let mut bm = ngon(9);
        let no = bm.face(FaceId(0)).no;
        triangulate(
            &mut bm,
            &TriangulateIn {
                faces: vec![FaceId(0)],
                quad_method: "BEAUTY",
                ngon_method: method,
            },
        )
        .unwrap();
        valid(&bm);
        assert_eq!(bm.faces_num(), 7, "{method}");
        let mut area = 0.0f32;
        for f in bm.face_ids() {
            let c: Vec<[f32; 3]> = bm.face_verts(f).iter().map(|&v| bm.vert(v).co).collect();
            let n = crate::query::geom::cross(
                crate::query::geom::sub(c[1], c[0]),
                crate::query::geom::sub(c[2], c[0]),
            );
            assert!(crate::query::geom::dot(n, no) > 0.0, "{method}: flipped triangle");
            area += crate::query::geom::dot(n, no) * 0.5;
        }
        let expect = 0.5 * 9.0 * libm::sinf(std::f32::consts::TAU / 9.0);
        assert!((area - expect).abs() < 1e-5, "{method}: area {area} != {expect}");
    }
}

#[test]
fn beautify_prefers_fat_triangles() {
    // A long thin hexagon: the fan from corner 0 gives slivers; beautify rotates them away.
    let coords = [
        [0.0, 0.0],
        [4.0, -0.5],
        [8.0, 0.0],
        [8.0, 1.0],
        [4.0, 1.5],
        [0.0, 1.0],
    ];
    let mut tris = vec![[0u32, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 5]];
    beautify::polyfill_beautify(&coords, &mut tris);
    assert_eq!(tris.len(), 4);
    // The fan's long diagonals 0-2 and 0-3 are gone.
    let has = |a: u32, b: u32| {
        tris.iter()
            .any(|t| (0..3).any(|k| (t[k] == a && t[(k + 1) % 3] == b) || (t[k] == b && t[(k + 1) % 3] == a)))
    };
    assert!(!has(0, 2) || !has(0, 3));
    assert!(has(1, 4));
}

#[test]
fn split_top_face_keeps_its_index() {
    let mut bm = cube();
    bm.act_face = Some(FaceId(5));
    let out = split(
        &mut bm,
        &SplitIn {
            geom: vec![BMElem::Face(FaceId(5))],
            ..SplitIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (12, 16, 6));
    // The copy took slot 6, the original's slot 5 is free: the copy is face 5 once compacted.
    assert!(!bm.faces.contains(5));
    assert_eq!(bm.act_face, Some(FaceId(6)));
    assert_eq!(out.geom.len(), 4 + 4 + 1);
    assert_eq!(out.boundary_map.len(), 4);
}

#[test]
fn split_edges_single_edge_and_ring() {
    let mut bm = cube();
    let e = bm.edge_ids()[0];
    split_edges(
        &mut bm,
        &SplitEdgesIn {
            edges: vec![e],
            ..SplitEdgesIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    // One edge of a closed cube: its ends stay fan-connected, so the edge copy is spliced
    // back (`bmesh_kernel_vert_separate__cleanup`): nothing changes.
    assert_eq!(counts(&bm), (8, 12, 6));

    let mut bm = cube();
    let ring = bm.face_edges(FaceId(5));
    let out = split_edges(
        &mut bm,
        &SplitEdgesIn {
            edges: ring,
            ..SplitEdgesIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (12, 16, 6));
    assert_eq!(out.edges.len(), 8);
}

#[test]
fn poke_top_face_fans_around_centre() {
    let mut bm = cube();
    let out = poke(
        &mut bm,
        &PokeIn {
            faces: vec![FaceId(5)],
            center_mode: "MEAN",
            ..PokeIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (9, 16, 9));
    assert_eq!(out.verts, [VertId(8)]);
    assert_eq!(bm.vert(VertId(8)).co, [0.0, 0.0, 1.0]);
    // Face [7, 3, 1, 5]: triangles [7,3,8] [3,1,8] [1,5,8] [5,7,8], the first in slot 6 (the
    // face's slot 5 is freed after the fan).
    assert_eq!(face_vert_indices(&bm, FaceId(6)), [7, 3, 8]);
    assert_eq!(face_vert_indices(&bm, FaceId(9)), [5, 7, 8]);
    assert!(!bm.faces.contains(5));
}

#[test]
fn smooth_vert_half_factor_on_cube() {
    let mut bm = cube();
    let verts = bm.vert_ids();
    smooth_vert(
        &mut bm,
        &SmoothVertIn {
            verts,
            factor: 0.5,
            use_axis_x: true,
            use_axis_y: true,
            use_axis_z: true,
            ..SmoothVertIn::default()
        },
    )
    .unwrap();
    for v in bm.vert_ids() {
        for c in bm.vert(v).co {
            assert!((c.abs() - 2.0 / 3.0).abs() < 1e-6, "{c}");
        }
    }
}

#[test]
fn smooth_vert_mirror_clip_and_axes() {
    // A vertex near the X mirror plane is clipped onto it; Z is not written.
    let mut bm = BMesh::new();
    let v: Vec<VertId> = [[0.05, 0.0, 0.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]]
        .iter()
        .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
        .collect();
    bm.edge_create(v[0], v[1], None, CreateFlag::empty());
    bm.edge_create(v[0], v[2], None, CreateFlag::empty());
    smooth_vert(
        &mut bm,
        &SmoothVertIn {
            verts: vec![v[0]],
            factor: 1.0,
            mirror_clip_x: true,
            clip_dist: 0.1,
            use_axis_x: true,
            use_axis_y: true,
            use_axis_z: false,
            ..SmoothVertIn::default()
        },
    )
    .unwrap();
    assert_eq!(bm.vert(v[0]).co, [0.0, 0.0, 0.0]);
}

#[test]
fn wireframe_cube_counts() {
    let mut bm = cube();
    let faces = bm.face_ids();
    let out = wireframe(
        &mut bm,
        &WireframeIn {
            faces,
            thickness: 0.01,
            offset: 0.01,
            use_replace: true,
            use_boundary: true,
            use_even_offset: true,
            ..WireframeIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (40, 96, 48));
    assert_eq!(out.faces.len(), 48);
}

#[test]
fn join_triangles_rebuilds_the_cube() {
    let mut bm = cube();
    let faces = bm.face_ids();
    triangulate(
        &mut bm,
        &TriangulateIn {
            faces,
            quad_method: "FIXED",
            ngon_method: "BEAUTY",
        },
    )
    .unwrap();
    let faces = bm.face_ids();
    let forty = 40.0f32.to_radians();
    let out = join_triangles(
        &mut bm,
        &JoinTrianglesIn {
            faces,
            angle_face_threshold: forty,
            angle_shape_threshold: forty,
            ..JoinTrianglesIn::default()
        },
    )
    .unwrap();
    valid(&bm);
    assert_eq!(counts(&bm), (8, 12, 6));
    assert_eq!(out.faces.len(), 6);
    assert!(bm.face_ids().iter().all(|&f| bm.face(f).len == 4));
}
