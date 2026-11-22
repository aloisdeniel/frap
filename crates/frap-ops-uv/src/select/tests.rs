// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the UV selection API on a 2×1 grid of quads (two UV islands when split).

use frap_bmesh::{BMesh, CreateFlag, ElemFlag, FaceId, LoopId, SelectMode};

use super::*;

/// Two quads sharing an edge: vertices 0..6 (`0 1 2` bottom, `3 4 5` top).
fn grid() -> BMesh {
    let mut bm = BMesh::new();
    let co = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, 1.0, 0.0],
    ];
    let v: Vec<_> = co
        .iter()
        .map(|c| bm.vert_create(*c, None, CreateFlag::empty()))
        .collect();
    bm.face_create_verts(&[v[0], v[1], v[4], v[3]], None, CreateFlag::empty(), true)
        .unwrap();
    bm.face_create_verts(&[v[1], v[2], v[5], v[4]], None, CreateFlag::empty(), true)
        .unwrap();
    bm.select_mode = SelectMode::VERT;
    bm
}

/// UVs equal to the vertex XY (`split`: the second face is moved by +3 in U).
fn uvs(bm: &BMesh, split: bool) -> Vec<Uv> {
    let mut out = vec![[0.0; 2]; bm.loops.capacity()];
    for (i, f) in bm.face_ids().into_iter().enumerate() {
        for l in bm.face_loops(f) {
            let c = bm.vert(bm.lp(l).v).co;
            let du = if split && i == 1 { 3.0 } else { 0.0 };
            out[l.slot()] = [c[0] + du, c[1]];
        }
    }
    out
}

fn ts(sync: bool) -> Ts {
    Ts {
        sync,
        selectmode: SelectMode::VERT,
        uv_selectmode: UV_SELECT_VERT,
        sticky: Sticky::Location,
        island: false,
    }
}

fn loops_of(bm: &BMesh, f: FaceId) -> Vec<LoopId> {
    bm.face_loops(f)
}

#[test]
fn select_all_and_invert_without_sync() {
    let mut bm = grid();
    for f in bm.face_ids() {
        bm.face_select_set(f, true);
    }
    let uv = uvs(&bm, false);
    let mut s = UvSel::with_uvs(ts(false), &mut bm, uv);
    s.deselect_all(SelAction::Select);
    let faces = s.faces();
    assert!(faces.iter().all(|&f| s.face_select_test(f)));
    s.deselect_all(SelAction::Invert);
    assert!(!s.is_any_selected());
    // Mesh selection is untouched without sync.
    assert_eq!(s.bm.selected_counts().2, 2);
}

#[test]
fn sticky_location_selects_shared_uvs_only() {
    let mut bm = grid();
    for f in bm.face_ids() {
        bm.face_select_set(f, true);
    }
    let faces = bm.face_ids();
    // Loop of face 0 at vertex 1.
    let l = loops_of(&bm, faces[0])[1];
    // Joined UVs: the corner of face 1 at the same vertex follows.
    let uv = uvs(&bm, false);
    let mut s = UvSel::with_uvs(ts(false), &mut bm, uv);
    s.uv_select_set_with_sticky(l, true);
    let v = s.lv(l);
    let n =
        s.bm.vert_loops(v)
            .into_iter()
            .filter(|&x| s.lflag(x, ElemFlag::SELECT_UV))
            .count();
    assert_eq!(n, 2);
    // Split UVs: only the picked corner.
    let mut bm2 = grid();
    for f in bm2.face_ids() {
        bm2.face_select_set(f, true);
    }
    let uv = uvs(&bm2, true);
    let mut s = UvSel::with_uvs(ts(false), &mut bm2, uv);
    s.uv_select_set_with_sticky(l, true);
    let n =
        s.bm.vert_loops(v)
            .into_iter()
            .filter(|&x| s.lflag(x, ElemFlag::SELECT_UV))
            .count();
    assert_eq!(n, 1);
}

#[test]
fn sync_round_trip_keeps_the_mesh_selection() {
    let mut bm = grid();
    let faces = bm.face_ids();
    bm.face_select_set(faces[0], true);
    let uv = uvs(&bm, true);
    let mut s = UvSel::with_uvs(ts(true), &mut bm, uv);
    s.prepare_sync_select();
    assert!(s.sync_valid);
    // Vertex mode, sticky location: UV selection = vertex selection, so the corners of the
    // second face at the shared vertices are selected too.
    let sel: usize = s
        .faces()
        .into_iter()
        .map(|f| {
            s.face_loops(f)
                .into_iter()
                .filter(|&l| s.lflag(l, ElemFlag::SELECT_UV))
                .count()
        })
        .sum();
    assert_eq!(sel, 6);
    s.bm_sync_to_mesh();
    assert_eq!(s.bm.selected_counts(), (4, 4, 1));
}

#[test]
fn linked_selects_the_uv_island() {
    let mut bm = grid();
    for f in bm.face_ids() {
        bm.face_select_set(f, true);
    }
    let faces = bm.face_ids();
    let l = loops_of(&bm, faces[0])[0];
    let uv = uvs(&bm, true);
    let mut s = UvSel::with_uvs(ts(false), &mut bm, uv);
    s.uv_select_set(l, true);
    linked::select_linked(
        &mut s,
        true,
        false,
        false,
        false,
        linked::Delimit::default(),
    );
    assert!(s.face_select_test(faces[0]));
    assert!(!s.face_select_test(faces[1]));
    // Joined UVs: one island.
    let mut bm2 = grid();
    for f in bm2.face_ids() {
        bm2.face_select_set(f, true);
    }
    let uv = uvs(&bm2, false);
    let mut s = UvSel::with_uvs(ts(false), &mut bm2, uv);
    s.uv_select_set(l, true);
    linked::select_linked(
        &mut s,
        true,
        false,
        false,
        false,
        linked::Delimit::default(),
    );
    assert!(s.face_select_test(faces[1]));
}

#[test]
fn mode_bits_round_trip() {
    for b in 1..8u8 {
        assert_eq!(mode_bits(mode_of_bits(b)), b);
    }
    assert_eq!(uv_selectmode_of("EDGE"), UV_SELECT_EDGE);
    assert_eq!(SelAction::of("INVERT"), SelAction::Invert);
    let mut t = ts(true);
    t.selectmode = SelectMode::EDGE | SelectMode::FACE;
    assert_eq!(t.select_mode_get(), UV_SELECT_EDGE);
}
