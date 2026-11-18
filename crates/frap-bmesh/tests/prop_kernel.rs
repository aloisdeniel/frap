// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 2.1 acceptance: 10 000 random kernel sequences keep `BMesh::validate` green
//! (ARCHITECTURE §20.1, property layer). Each case starts from a cube, a grid, a cylinder
//! (n-gon caps) or an empty BMesh and applies up to 40 random operations drawn from every
//! kernel entry point: element creation (with and without `NO_DOUBLE`), kills (vertex, edge,
//! face, loose face), SEMV / edge split, JEKV, JVKE, SFME / face split, JFKE, face join,
//! vertex and edge splice, vertex and edge separate, winding flips, `delete` with every
//! context, `duplicate`, selection and normals. The structure is validated after every
//! operation, and at the end the mesh round trip `to_mesh` → `from_mesh` → `to_mesh` must
//! reproduce the same mesh.
#![allow(clippy::manual_is_multiple_of)] // `x % n == 0` reads better for random parameters

use frap_bmesh::ops::delete::{DeleteIn, delete};
use frap_bmesh::ops::duplicate::{DuplicateIn, duplicate};
use frap_bmesh::{BMElem, BMesh, CreateFlag, FromMeshParams, SelectMode, ToMeshParams};
use frap_mesh::Mesh;
use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::grid::{Grid, grid};
use proptest::prelude::*;

const CONTEXTS: [&str; 7] = [
    "VERTS",
    "EDGES",
    "FACES_ONLY",
    "EDGES_FACES",
    "FACES",
    "FACES_KEEP_BOUNDARY",
    "TAGGED_ONLY",
];

fn base(kind: u8) -> (Mesh, BMesh) {
    let me = match kind % 4 {
        0 => cube_mesh(&Cube::default()),
        1 => grid(&Grid {
            x_subdivisions: 3,
            y_subdivisions: 3,
            ..Grid::default()
        }),
        2 => cylinder(&Cylinder {
            vertices: 6,
            ..Cylinder::default()
        }),
        _ => Mesh::new("Empty"),
    };
    let bm = BMesh::from_mesh(&me, &FromMeshParams::default());
    (me, bm)
}

fn pick<T: Copy>(v: &[T], i: u32) -> Option<T> {
    if v.is_empty() {
        None
    } else {
        Some(v[i as usize % v.len()])
    }
}

/// Applies operation `op` with random parameters `a`, `b`, `c`; returns a label.
fn apply(bm: &mut BMesh, op: u8, a: u32, b: u32, c: u32) -> &'static str {
    let verts = bm.vert_ids();
    let edges = bm.edge_ids();
    let faces = bm.face_ids();
    let co = [a as f32 * 0.01, b as f32 * 0.01, c as f32 * 0.01];
    match op % 24 {
        0 => {
            let ex = if c % 2 == 0 { pick(&verts, b) } else { None };
            bm.vert_create(co, ex, CreateFlag::empty());
            "vert_create"
        }
        1 => {
            if let (Some(x), Some(y)) = (pick(&verts, a), pick(&verts, b))
                && x != y
            {
                let flag = if c % 2 == 0 {
                    CreateFlag::NO_DOUBLE
                } else {
                    CreateFlag::empty()
                };
                bm.edge_create(x, y, pick(&edges, c), flag);
            }
            "edge_create"
        }
        2 => {
            // A face over 3-5 distinct vertices (creating missing edges).
            let n = 3 + (c % 3) as usize;
            if verts.len() >= n {
                // Distinct vertices: start at `a`, step by `1 + b % len`, then by one.
                let len = verts.len();
                let step = 1 + b as usize % len;
                let mut vs = Vec::new();
                let mut k = a as usize % len;
                for _ in 0..len {
                    let v = verts[k];
                    if !vs.contains(&v) {
                        vs.push(v);
                        if vs.len() == n {
                            break;
                        }
                    }
                    k = (k + step) % len;
                }
                let mut k = 0;
                while vs.len() < n {
                    if !vs.contains(&verts[k]) {
                        vs.push(verts[k]);
                    }
                    k += 1;
                }
                let flag = if c % 2 == 0 {
                    CreateFlag::NO_DOUBLE
                } else {
                    CreateFlag::empty()
                };
                bm.face_create_verts(&vs, pick(&faces, b), flag, true);
            }
            "face_create_verts"
        }
        3 => {
            if let Some(v) = pick(&verts, a) {
                bm.vert_kill(v);
            }
            "vert_kill"
        }
        4 => {
            if let Some(e) = pick(&edges, a) {
                bm.edge_kill(e);
            }
            "edge_kill"
        }
        5 => {
            if let Some(f) = pick(&faces, a) {
                if b % 2 == 0 {
                    bm.face_kill(f);
                } else {
                    bm.face_kill_loose(f);
                }
            }
            "face_kill"
        }
        6 => {
            if let Some(e) = pick(&edges, a) {
                let v = bm.edge(e).v[(b % 2) as usize];
                if c % 2 == 0 {
                    bm.split_edge_make_vert(v, e);
                } else {
                    bm.edge_split(e, v, (c % 100) as f32 / 100.0);
                }
            }
            "split_edge_make_vert"
        }
        7 => {
            // JEKV on a valence-2 vertex.
            let cands: Vec<_> = verts
                .iter()
                .copied()
                .filter(|&v| bm.vert_edge_count(v) == 2)
                .collect();
            if let Some(v) = pick(&cands, a) {
                let e = bm.vert_edges(v)[(b % 2) as usize];
                // Degenerate (2-corner) faces are always killed: kernel functions do not accept
                // them as input.
                bm.join_edge_kill_vert(e, v, c % 7 != 0, c % 2 == 0, true, c % 5 == 0);
            }
            "join_edge_kill_vert"
        }
        8 => {
            // SFME / face split between two non-adjacent corners.
            let cands: Vec<_> = faces
                .iter()
                .copied()
                .filter(|&f| bm.face(f).len >= 4)
                .collect();
            if let Some(f) = pick(&cands, a) {
                let ls = bm.face_loops(f);
                let n = ls.len();
                let i = b as usize % n;
                let j = (i + 2 + c as usize % (n - 3)) % n;
                // Precondition: two different vertices (a collapsed face may repeat one).
                if bm.lp(ls[i]).v == bm.lp(ls[j]).v {
                } else if c % 2 == 0 {
                    bm.split_face_make_edge(f, ls[i], ls[j], None, c % 3 == 0);
                } else {
                    bm.face_split(f, ls[i], ls[j], pick(&edges, c), c % 3 == 0);
                }
            }
            "split_face_make_edge"
        }
        9 => {
            // JFKE over a manifold edge.
            let cands: Vec<_> = edges
                .iter()
                .copied()
                .filter(|&e| bm.edge_is_manifold(e))
                .collect();
            if let Some(e) = pick(&cands, a) {
                let fs = bm.edge_faces(e);
                bm.join_face_kill_edge(fs[0], fs[1], e);
            }
            "join_face_kill_edge"
        }
        10 => {
            // Face join of the faces around an edge.
            let cands: Vec<_> = edges
                .iter()
                .copied()
                .filter(|&e| bm.edge_is_manifold(e))
                .collect();
            if let Some(e) = pick(&cands, a) {
                let fs = bm.edge_faces(e);
                if fs[0] != fs[1] {
                    bm.faces_join(&fs, b % 2 == 0, c % 2 == 0);
                }
            }
            "faces_join"
        }
        11 => {
            // Vertex splice of two vertices sharing no edge and no face.
            if let (Some(x), Some(y)) = (pick(&verts, a), pick(&verts, b))
                && x != y
                && bm.edge_exists(x, y).is_none()
            {
                let fx = bm.vert_faces(x);
                if !bm.vert_faces(y).iter().any(|f| fx.contains(f)) {
                    // Splicing must not create a degenerate edge through a third vertex
                    // shared by both: only splice when their neighbourhoods are disjoint.
                    let nx: Vec<_> = bm
                        .vert_edges(x)
                        .iter()
                        .map(|&e| bm.edge_other_vert(e, x))
                        .collect();
                    let ny: Vec<_> = bm
                        .vert_edges(y)
                        .iter()
                        .map(|&e| bm.edge_other_vert(e, y))
                        .collect();
                    if !nx.iter().any(|v| ny.contains(v)) {
                        bm.vert_splice(x, y);
                    }
                }
            }
            "vert_splice"
        }
        12 => {
            // Duplicate an edge and splice it back.
            if let Some(e) = pick(&edges, a) {
                let [x, y] = bm.edge(e).v;
                let d = bm.edge_create(x, y, Some(e), CreateFlag::empty());
                if b % 2 == 0 {
                    bm.edge_splice(e, d);
                }
            }
            "edge_splice"
        }
        13 => {
            // Precondition (`BM_CHECK_ELEMENT`): no edge used twice by the face.
            if let Some(f) = pick(&faces, a) {
                let mut es = bm.face_edges(f);
                es.sort();
                es.dedup();
                if es.len() == bm.face(f).len as usize {
                    bm.face_normal_flip(f);
                }
            }
            "face_normal_flip"
        }
        14 => {
            if let Some(v) = pick(&verts, a) {
                let es = bm.vert_edges(v);
                let sel: Vec<_> = es
                    .iter()
                    .copied()
                    .enumerate()
                    .filter(|(i, _)| (b >> (i % 32)) & 1 == 1)
                    .map(|(_, e)| e)
                    .collect();
                bm.vert_separate(v, &sel, c % 2 == 0);
            }
            "vert_separate"
        }
        15 => {
            if let Some(e) = pick(&edges, a)
                && let Some(l) = bm.edge(e).l
            {
                bm.edge_separate(e, l, b % 2 == 0);
            }
            "edge_separate"
        }
        16 => {
            // Edge collapse (JVKE).
            // Precondition (as Blender's callers ensure): `e` is the only edge joining its
            // vertices, else the other one would become degenerate.
            if let Some(e) = pick(&edges, a) {
                let [x, y] = bm.edge(e).v;
                let doubles = bm
                    .vert_edges(x)
                    .into_iter()
                    .filter(|&o| bm.edge_other_vert(o, x) == y)
                    .count();
                // and no third vertex is adjacent to both (else faces would repeat a vertex,
                // which no kernel function accepts as input).
                let nx: Vec<_> = bm
                    .vert_edges(x)
                    .iter()
                    .map(|&o| bm.edge_other_vert(o, x))
                    .collect();
                let common = bm
                    .vert_edges(y)
                    .iter()
                    .map(|&o| bm.edge_other_vert(o, y))
                    .any(|w| w != x && nx.contains(&w));
                let shared_face = bm
                    .vert_faces(x)
                    .iter()
                    .any(|f| bm.vert_faces(y).contains(f) && !bm.edge_faces(e).contains(f));
                if doubles == 1 && !common && !shared_face {
                    let v = bm.edge(e).v[(b % 2) as usize];
                    bm.join_vert_kill_edge(e, v, true, true, true);
                }
            }
            "join_vert_kill_edge"
        }
        17 => {
            let mut geom = Vec::new();
            for k in 0..(1 + c % 4) {
                match (a + k) % 3 {
                    0 => geom.extend(pick(&verts, b.wrapping_add(k * 7)).map(BMElem::Vert)),
                    1 => geom.extend(pick(&edges, b.wrapping_add(k * 5)).map(BMElem::Edge)),
                    _ => geom.extend(pick(&faces, b.wrapping_add(k * 3)).map(BMElem::Face)),
                }
            }
            geom.sort();
            geom.dedup();
            delete(
                bm,
                &DeleteIn {
                    geom,
                    context: CONTEXTS[(c % 7) as usize],
                },
            )
            .unwrap();
            "delete"
        }
        18 => {
            let mut geom = Vec::new();
            for k in 0..(1 + c % 4) {
                match (a + k) % 3 {
                    0 => geom.extend(pick(&verts, b.wrapping_add(k * 7)).map(BMElem::Vert)),
                    1 => geom.extend(pick(&edges, b.wrapping_add(k * 5)).map(BMElem::Edge)),
                    _ => geom.extend(pick(&faces, b.wrapping_add(k * 3)).map(BMElem::Face)),
                }
            }
            duplicate(
                bm,
                &DuplicateIn {
                    geom,
                    use_select_history: c % 2 == 0,
                    use_edge_flip_from_face: b % 2 == 0,
                    ..Default::default()
                },
            )
            .unwrap();
            "duplicate"
        }
        19 => {
            let e = match a % 3 {
                0 => pick(&verts, b).map(BMElem::Vert),
                1 => pick(&edges, b).map(BMElem::Edge),
                _ => pick(&faces, b).map(BMElem::Face),
            };
            if let Some(e) = e {
                bm.select_set(e, c % 3 != 0);
                if c % 2 == 0 {
                    bm.select_history_store(e);
                }
            }
            "select_set"
        }
        20 => {
            bm.select_mode =
                [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE][(a % 3) as usize];
            if b % 2 == 0 {
                bm.select_flush_mode();
            } else {
                let m = [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE][(c % 3) as usize];
                bm.select_mode_convert(m, c % 2 == 0);
            }
            "select_mode"
        }
        21 => {
            let e = match c % 3 {
                0 => pick(&verts, a).map(BMElem::Vert),
                1 => pick(&edges, a).map(BMElem::Edge),
                _ => pick(&faces, a).map(BMElem::Face),
            };
            if let Some(e) = e {
                bm.elem_hide_set(e, b % 2 == 0);
            }
            bm.normals_update();
            "normals_update"
        }
        22 => {
            if let Some(e) = pick(&edges, a) {
                let _ = (bm.walk_edge_loop(e), bm.walk_edge_ring(e));
            }
            if let Some(f) = pick(&faces, b) {
                let _ = bm.walk_face_island(f, Default::default());
            }
            if let Some(v) = pick(&verts, c) {
                let _ = bm.walk_vert_shell(v);
            }
            "walkers"
        }
        _ => {
            if let Some(l) = pick(&faces, a).map(|f| bm.face_loops(f)[0])
                && let Some(src) = pick(&faces, b)
            {
                bm.loop_interp_from_face(l, src, c % 2 == 0);
            }
            "loop_interp_from_face"
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: cases(), max_shrink_iters: 2000, ..ProptestConfig::default() })]

    #[test]
    fn random_kernel_sequences_stay_valid(
        kind in 0u8..4,
        ops in prop::collection::vec((0u8..24, any::<u32>(), any::<u32>(), any::<u32>()), 1..40),
    ) {
        let (me, mut bm) = base(kind);
        for (i, &(op, a, b, c)) in ops.iter().enumerate() {
            let label = apply(&mut bm, op, a, b, c);
            let errs = bm.validate();
            prop_assert!(errs.is_empty(), "after op {i} ({label}): {errs:?}");
        }
        let out = bm.to_mesh(&me, &ToMeshParams::default());
        prop_assert!(out.validate_report().is_none(), "to_mesh: {:?}", out.validate_report());
        let again = BMesh::from_mesh(&out, &FromMeshParams::default());
        prop_assert!(again.validate().is_empty());
        let out2 = again.to_mesh(&out, &ToMeshParams::default());
        prop_assert_eq!(out.positions(), out2.positions());
        prop_assert_eq!(out.edges(), out2.edges());
        prop_assert_eq!(out.face_offsets(), out2.face_offsets());
        prop_assert_eq!(out.corner_verts(), out2.corner_verts());
        prop_assert_eq!(out.corner_edges(), out2.corner_edges());
        // Hidden elements cannot be selected through `from_mesh` (as in Blender), so the flag
        // attributes only survive the second trip when no element is hidden and selected.
        let hidden_selected = bm.validate_strict().iter().any(|e| e.contains("hidden and selected"));
        if !hidden_selected {
            // `.select_*` are skipped too: an unflushed selection (a selected edge with an
            // unselected vertex) is flushed down by `from_mesh`'s `BM_edge_select_set`.
            let names = |m: &Mesh| {
                m.attributes()
                    .iter()
                    .map(|a| a.name.to_string())
                    .filter(|n| !n.starts_with(".select_"))
                    .collect::<Vec<_>>()
            };
            prop_assert_eq!(names(&out), names(&out2));
        }
    }
}

/// 10 000 cases (the WP 2.1 acceptance), or `PROPTEST_CASES` when set.
fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000)
}
