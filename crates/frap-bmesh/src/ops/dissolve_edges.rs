// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_edges` (Blender 5.2.4 `bmo_dissolve_edges_exec`, `operators/
//! bmo_dissolve.cc`), implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_edges(bmesh, edges=[], use_verts=False, use_face_split=False, angle_threshold=0.0, use_preserve_quads=False)`.
//! Blender's `bmo_dissolve_edges_init` sets `angle_threshold` to π (dissolve every vertex), so
//! that is the `Default` here.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/bmo_dissolve.cc):
//! the vertices and edges of the faces on both sides of each input edge are marked for
//! clean-up; with `use_verts`, the ends of each input edge (followed to the end of edge
//! chains) are marked for dissolving unless they would collapse a corner of a joined triangle
//! pair (`use_preserve_quads`) or only touch the selection incidentally. With
//! `use_face_split`, corners at vertices left with at most two unmarked edges are cut off
//! first (new edges and faces). Then, in input order, the two faces of each input edge are
//! joined (`BM_faces_join_pair`: the merged face is **created**, the two old ones freed);
//! edges left without faces and vertices left without edges are killed in pool order; and
//! the marked vertices still between exactly two edges whose angle passes `angle_threshold`
//! are dissolved in vertex order (`BM_vert_collapse_edge`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use crate::ops::dissolve_verts::{bm_face_split, bm_vert_collapse_edge_and_merge, mods};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, VertId};

/// Input slots of `dissolve_edges` (Blender's slot defaults, after
/// `bmo_dissolve_edges_init`, in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct DissolveEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_verts`.
    pub use_verts: bool,
    /// `use_face_split`.
    pub use_face_split: bool,
    /// `angle_threshold` (radians).
    pub angle_threshold: f32,
    /// `use_preserve_quads`.
    pub use_preserve_quads: bool,
}

impl Default for DissolveEdgesIn {
    fn default() -> Self {
        DissolveEdgesIn {
            edges: Vec::new(),
            use_verts: false,
            use_face_split: false,
            angle_threshold: core::f32::consts::PI,
            use_preserve_quads: false,
        }
    }
}

/// Output slots of `dissolve_edges`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveEdgesOut {
    /// `region.out` as elements (Blender never fills it for this operator).
    pub geom: Vec<BMElem>,
    /// `region.out` (declared by Blender, never filled).
    pub region: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_dissolve_edges_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const EDGE_TAG: u16 = 2;
const EDGE_ISGC: u16 = 8;
const EDGE_CHAIN: u16 = 16;
const VERT_MARK: u16 = 1;
const VERT_TAG: u16 = 2;
const VERT_ISGC: u16 = 8;

/// Runs `dissolve_edges` on `bm`.
pub fn dissolve_edges(bm: &mut BMesh, i: &DissolveEdgesIn) -> Result<DissolveEdgesOut, BmError> {
    if let Some(e) = i.edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("dissolve_edges: dead edge {e:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| dissolve_edges_exec(bm, i));
    Ok(DissolveEdgesOut::default())
}

/// `bmo_vert_calc_edge_angle_blended`: the angle at an edge-pair vertex, blending the raw
/// angle with the angle around the vertex normal by how folded its two faces are.
fn vert_calc_edge_angle_blended(bm: &BMesh, v: VertId) -> f32 {
    let (e0, e1) = mods::vert_edge_pair(bm, v).expect("edge pair");
    let v_a = bm.edge_other_vert(e0, v);
    let v_b = bm.edge_other_vert(e1, v);
    let (ca, cv, cb) = (bm.vert(v_a).co, bm.vert(v).co, bm.vert(v_b).co);
    let mut angle = core::f32::consts::PI - mods::angle_v3v3v3(ca, cv, cb);
    let ve = bm.vert(v).e.expect("edge pair");
    if let Some((fa, fb)) = mods::edge_face_pair(bm, ve) {
        // Due to merges, the normals are not currently trustworthy. Compute them.
        let no_a = bm.face_calc_normal(fa);
        let no_b = bm.face_calc_normal(fb);
        let raw_factor = (-crate::query::geom::dot(no_a, no_b)).clamp(0.0, 1.0);
        let normal_angle =
            core::f32::consts::PI - mods::angle_on_axis_v3v3v3_v3(ca, cv, cb, bm.vert(v).no);
        angle = mods::interpf(angle, normal_angle, raw_factor);
    }
    angle
}

/// `bmo_find_end_of_chain`: walks a chain of edge-pair vertices from `v` (away from `e`) to
/// its far end, tagging each edge with `edge_oflag` (0: no tagging); `None` when an already
/// tagged edge is met or the chain loops.
fn find_end_of_chain(bm: &mut BMesh, mut e: EdgeId, mut v: VertId, edge_oflag: u16) -> Option<VertId> {
    let v_init = v;
    while mods::vert_is_edge_pair(bm, v) {
        // Move one step down the chain.
        e = bm.disk_edge_next(e, v);
        v = bm.edge_other_vert(e, v);
        if edge_oflag != 0 && bm.oflag_test(e, edge_oflag) {
            return None;
        }
        bm.oflag_enable(e, edge_oflag);
        if v == v_init {
            return None;
        }
    }
    Some(v)
}

/// `bmo_vert_touches_unselected_face`.
fn vert_touches_unselected_face(bm: &BMesh, v: VertId) -> bool {
    if bm.oflag_test(v, VERT_MARK) {
        return false;
    }
    bm.vert_loops(v).into_iter().any(|l_a| {
        let l_b = bm.loop_other_edge_loop(l_a, v);
        !bm.oflag_test(bm.lp(l_a).e, EDGE_TAG) && !bm.oflag_test(bm.lp(l_b).e, EDGE_TAG)
    })
}

/// `bmo_vert_tagged_edges_count_at_most`.
fn vert_tagged_edges_count_at_most(bm: &BMesh, v: VertId, edge_oflag: u16, max: usize) -> usize {
    let mut n = 0;
    for e in bm.vert_edges(v) {
        if bm.oflag_test(e, edge_oflag) {
            n += 1;
        }
        if n == max {
            return n;
        }
    }
    n
}

fn dissolve_edges_exec(bm: &mut BMesh, i: &DissolveEdgesIn) {
    // RAD2DEGF(0.0001f): the epsilon Blender uses for exact 0/90/180 degree angles.
    let angle_epsilon = mods::rad2degf(0.0001);
    let angle_threshold = i.angle_threshold;
    let use_verts = i.use_verts && angle_threshold > angle_epsilon;
    let dissolve_all = angle_threshold > core::f32::consts::PI - angle_epsilon;
    let edges = &i.edges;

    if i.use_face_split || use_verts {
        for &e in edges {
            bm.oflag_enable(e, EDGE_TAG);
        }
    }

    // Tag certain geometry around the selected edges, for later processing.
    for &e in edges {
        let [ev1, ev2] = bm.edge(e).v;
        if mods::vert_is_edge_pair(bm, ev1) || mods::vert_is_edge_pair(bm, ev2) {
            bm.oflag_enable(e, EDGE_CHAIN);
        }
        if let Some((fa, fb)) = mods::edge_face_pair(bm, e) {
            for f in [fa, fb] {
                for l in bm.face_loops(f) {
                    let (lv, le) = (bm.lp(l).v, bm.lp(l).e);
                    bm.oflag_enable(lv, VERT_ISGC);
                    bm.oflag_enable(le, EDGE_ISGC);
                }
            }
            if use_verts && bm.oflag_test(e, EDGE_CHAIN) {
                find_end_of_chain(bm, e, ev1, EDGE_TAG);
                find_end_of_chain(bm, e, ev2, EDGE_TAG);
            }
        }
    }

    if use_verts {
        // Mark all verts that are candidates to be dissolved.
        for &e in edges {
            let Some((fa, fb)) = mods::edge_face_pair(bm, e) else {
                continue;
            };
            for k in 0..2 {
                let mut v_edge = Some(bm.edge(e).v[k]);
                if i.use_preserve_quads
                    && bm.face(fa).len == 3
                    && bm.face(fb).len == 3
                    && vert_tagged_edges_count_at_most(bm, v_edge.unwrap(), EDGE_TAG, 2) == 1
                {
                    continue;
                }
                if mods::vert_is_edge_pair(bm, v_edge.unwrap()) {
                    v_edge = find_end_of_chain(bm, e, v_edge.unwrap(), EDGE_CHAIN);
                }
                let Some(v_edge) = v_edge else {
                    continue;
                };
                if vert_touches_unselected_face(bm, v_edge)
                    && vert_tagged_edges_count_at_most(bm, v_edge, EDGE_TAG, 2) != 1
                {
                    continue;
                }
                bm.oflag_enable(v_edge, VERT_MARK);
            }
        }
    }

    if i.use_face_split {
        for v in bm.vert_ids() {
            let untag_count = bm
                .vert_edges(v)
                .into_iter()
                .filter(|&e| !bm.oflag_test(e, EDGE_TAG))
                .count();
            // Check that we have 2 edges remaining after dissolve.
            if untag_count <= 2 {
                bm.oflag_enable(v, VERT_TAG);
            }
        }
        bm_face_split(bm, VERT_TAG, false);
    }

    // Merge any face pairs that straddle a selected edge.
    for &e in edges {
        frap_math::budget::check();
        if !bm.edges.contains(e.0) {
            continue;
        }
        if let Some((l_a, l_b)) = mods::edge_loop_pair(bm, e) {
            mods::faces_join_pair(bm, l_a, l_b, false);
        }
    }

    // Remove edges that are garbage collectible and became irrelevant (no loops).
    for e in bm.edge_ids() {
        if bm.edges.contains(e.0) && bm.edge(e).l.is_none() && bm.oflag_test(e, EDGE_ISGC) {
            bm.edge_kill(e);
        }
    }
    // Remove verts that are garbage collectible and became isolated (no edges).
    for v in bm.vert_ids() {
        if bm.verts.contains(v.0) && bm.vert(v).e.is_none() && bm.oflag_test(v, VERT_ISGC) {
            bm.vert_kill(v);
        }
    }

    // If dissolving verts, then evaluate each VERT_MARK vert.
    if use_verts {
        for v in bm.vert_ids() {
            if !bm.oflag_test(v, VERT_MARK) {
                continue;
            }
            let Some((e0, e1)) = mods::vert_edge_pair(bm, v) else {
                bm.oflag_disable(v, VERT_MARK);
                continue;
            };
            if dissolve_all {
                continue;
            }
            if bm.oflag_test(e0, EDGE_CHAIN) || bm.oflag_test(e1, EDGE_CHAIN) {
                continue;
            }
            if vert_calc_edge_angle_blended(bm, v) > angle_threshold - angle_epsilon {
                bm.oflag_disable(v, VERT_MARK);
            }
        }
        // Dissolve all verts that remain tagged, in a separate pass (earlier dissolves would
        // alter the angles measured at neighbouring verts).
        for v in bm.vert_ids() {
            if !bm.verts.contains(v.0) || !bm.oflag_test(v, VERT_MARK) {
                continue;
            }
            if !mods::vert_is_edge_pair(bm, v) {
                continue;
            }
            bm_vert_collapse_edge_and_merge(bm, v, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    /// Two quads sharing the edge `v1`–`v4`.
    fn two_quads() -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let co = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ];
        let v: Vec<VertId> = co
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        bm.face_create_verts(&[v[0], v[1], v[4], v[3]], None, CreateFlag::empty(), true);
        bm.face_create_verts(&[v[1], v[2], v[5], v[4]], None, CreateFlag::empty(), true);
        (bm, v)
    }

    #[test]
    fn dissolving_the_shared_edge_joins_the_quads_and_its_ends() {
        let (mut bm, v) = two_quads();
        let e = bm.edge_exists(v[1], v[4]).unwrap();
        dissolve_edges(
            &mut bm,
            &DissolveEdgesIn {
                edges: vec![e],
                use_verts: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.faces_num(), 1);
        assert_eq!(bm.verts_num(), 4);
        assert_eq!(bm.edges_num(), 4);
    }

    #[test]
    fn without_verts_the_ends_stay() {
        let (mut bm, v) = two_quads();
        let e = bm.edge_exists(v[1], v[4]).unwrap();
        dissolve_edges(
            &mut bm,
            &DissolveEdgesIn {
                edges: vec![e],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(bm.faces_num(), 1);
        assert_eq!(bm.verts_num(), 6);
        assert_eq!(bm.edges_num(), 6);
    }
}
