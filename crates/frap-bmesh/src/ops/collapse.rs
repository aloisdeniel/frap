// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.collapse` (Blender 5.2.4 `bmo_collapse_exec`, `operators/bmo_removedoubles.cc`),
//! implemented by WP 2.7 (Delete, dissolve, merge), with its companion
//! `bmesh.ops.collapse_uvs` (`bmo_collapse_uvs_exec`).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.collapse(bmesh, edges=[], uvs=False)`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/
//! bmo_removedoubles.cc): with `uvs`, `collapse_uvs` first snaps the UV/colour data of each
//! connected island of corners around the input edges to the centre of its bounding box. Then
//! the input edges are walked in edge order with one `BMW_VERT_SHELL` walker (breadth-first,
//! its visited set shared by every walk, so each connected group of input edges is walked
//! once, from the first vertex of its lowest edge); all vertices of a group move to the mean
//! of its edges' endpoints, and every vertex except the first vertex of the **last** walked
//! edge maps to it. `weld_verts` then runs as a nested operator (edges and faces rebuilt in
//! place, merged vertices killed last).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use rustc_hash::FxHashSet;
use std::collections::VecDeque;

use crate::ops::dissolve_verts::mods::{self, MinMax};
use crate::ops::weld_verts::{self, TargetMap};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, LoopId};

/// Input slots of `collapse` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollapseIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `uvs`.
    pub uvs: bool,
}

/// Output slots of `collapse` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct CollapseOut {
    /// Always empty (`bmesh.ops.collapse` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_collapse_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const EDGE_MARK: u16 = 1;

fn check_alive(bm: &BMesh, edges: &[EdgeId]) -> Result<(), BmError> {
    if let Some(e) = edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("collapse: dead edge {e:?}")));
    }
    Ok(())
}

/// Runs `collapse` on `bm`.
pub fn collapse(bm: &mut BMesh, i: &CollapseIn) -> Result<CollapseOut, BmError> {
    check_alive(bm, &i.edges)?;
    bm.bmo_exec(TYPE_FLAG, |bm| collapse_exec(bm, &i.edges, i.uvs));
    Ok(CollapseOut::default())
}

/// The body of `bmo_collapse_exec` (inside the operator's flag layer).
pub(crate) fn collapse_exec(bm: &mut BMesh, edges: &[EdgeId], uvs: bool) {
    if uvs {
        bm.bmo_exec(OpTypeFlag::empty(), |bm| collapse_uvs_exec(bm, edges));
    }
    let mut map = TargetMap::default();
    for &e in edges {
        bm.oflag_enable(e, EDGE_MARK);
    }
    // One `BMW_VERT_SHELL` walker: the visited set persists across `BMW_begin` calls.
    let mut visit: FxHashSet<EdgeId> = FxHashSet::default();
    let mut edge_stack: Vec<EdgeId> = Vec::new();
    for e in bm.edge_ids() {
        if !bm.oflag_test(e, EDGE_MARK) {
            continue;
        }
        frap_math::budget::check();
        let mut center = [0.0f32; 3];
        let mut count = 0usize;
        // BMW_begin(e->v1): every unvisited marked edge around it, then breadth-first.
        let mut queue: VecDeque<EdgeId> = VecDeque::new();
        let mut visit_edge = |bm: &BMesh, queue: &mut VecDeque<EdgeId>, x: EdgeId| {
            if !visit.contains(&x) && bm.oflag_test(x, EDGE_MARK) {
                queue.push_back(x);
                visit.insert(x);
            }
        };
        let v1 = bm.edge(e).v[0];
        for x in bm.vert_edges(v1) {
            visit_edge(bm, &mut queue, x);
        }
        while let Some(w) = queue.pop_front() {
            for v in bm.edge(w).v {
                for x in bm.vert_edges(v) {
                    visit_edge(bm, &mut queue, x);
                }
            }
            edge_stack.push(w);
            let [a, b] = bm.edge(w).v;
            for c in [bm.vert(a).co, bm.vert(b).co] {
                center = [center[0] + c[0], center[1] + c[1], center[2] + c[2]];
            }
            count += 2;
            // Prevent adding to `slot_targetmap` multiple times.
            bm.vert_mut(a).flag.remove(ElemFlag::TAG);
            bm.vert_mut(b).flag.remove(ElemFlag::TAG);
        }
        let Some(&e_top) = edge_stack.last() else {
            continue;
        };
        let s = 1.0 / count as f32;
        center = [center[0] * s, center[1] * s, center[2] * s];
        // Snap edges to a point.
        let v_tar = bm.edge(e_top).v[0];
        while let Some(w) = edge_stack.pop() {
            for v_src in bm.edge(w).v {
                bm.vert_mut(v_src).co = center;
                if v_src != v_tar && !bm.vert(v_src).flag.contains(ElemFlag::TAG) {
                    bm.vert_mut(v_src).flag.insert(ElemFlag::TAG);
                    map.insert(v_src, v_tar);
                }
            }
        }
    }
    bm.bmo_exec(weld_verts::TYPE_FLAG, |bm| {
        weld_verts::weld_exec(bm, &map, false, false);
    });
}

/// Input slots of `collapse_uvs`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollapseUvsIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
}

/// `bmesh.ops.collapse_uvs`: per UV/colour corner layer, each island of corners connected
/// over the input edges with equal values snaps to the centre of its bounding box.
pub fn collapse_uvs(bm: &mut BMesh, i: &CollapseUvsIn) -> Result<(), BmError> {
    check_alive(bm, &i.edges)?;
    bm.bmo_exec(OpTypeFlag::empty(), |bm| collapse_uvs_exec(bm, &i.edges));
    Ok(())
}

fn collapse_uvs_exec(bm: &mut BMesh, edges: &[EdgeId]) {
    let layers = mods::math_layers(bm);
    if layers.is_empty() {
        return;
    }
    for &e in edges {
        bm.oflag_enable(e, EDGE_MARK);
    }
    for layer in layers {
        collapsecon_do_layer(bm, layer, EDGE_MARK);
    }
}

/// `bmw_mask_check_edge` of a walker with `mask_edge = oflag` and no hidden test.
fn mask_edge(bm: &BMesh, e: EdgeId, oflag: u16) -> bool {
    bm.oflag_test(e, oflag)
}

/// `bmo_collapsecon_do_layer`: one `BMW_LOOPDATA_ISLAND` walker (depth-first, visited set
/// shared by every walk) over the loops of the marked edges.
fn collapsecon_do_layer(bm: &mut BMesh, layer: usize, oflag: u16) {
    let mut visit: FxHashSet<LoopId> = FxHashSet::default();
    for f in bm.face_ids() {
        for l in bm.face_loops(f) {
            if !bm.oflag_test(bm.lp(l).e, oflag) {
                continue;
            }
            // Walk (BMW_begin: the start loop unless visited).
            if !visit.insert(l) {
                continue;
            }
            let mut stack: Vec<LoopId> = vec![l];
            let mut blocks: Vec<LoopId> = Vec::new();
            let mut mm = MinMax::new(&mods::math_get(bm, layer, l));
            while let Some(cur) = stack.pop() {
                uv_edge_walker_step(bm, layer, oflag, cur, &mut visit, &mut stack);
                mm.add(&mods::math_get(bm, layer, cur));
                blocks.push(cur);
            }
            // Snap custom-data (UV, vertex-colors) points to their centroid.
            let c = mm.center();
            while let Some(b) = blocks.pop() {
                mods::math_set(bm, layer, b, c);
            }
        }
    }
}

/// `bmw_UVEdgeWalker_step` for the loop `l` (its yield is `l`): pushes the loops around both
/// ends of `l`'s edge that share the value, depth-first (each push goes on top).
fn uv_edge_walker_step(
    bm: &BMesh,
    layer: usize,
    oflag: u16,
    l: LoopId,
    visit: &mut FxHashSet<LoopId>,
    stack: &mut Vec<LoopId>,
) {
    if !mask_edge(bm, bm.lp(l).e, oflag) {
        return;
    }
    for i in 0..2 {
        let l_pivot = if i == 1 { bm.lp(l).next } else { l };
        let pv = bm.lp(l_pivot).v;
        let data_pivot = mods::math_get(bm, layer, l_pivot);
        for l_radial_first in bm.vert_loops(pv) {
            let mut l_radial = l_radial_first;
            loop {
                let lr = bm.lp(l_radial);
                let skip = visit.contains(&l_radial)
                    || (lr.v != pv && !mask_edge(bm, lr.e, oflag));
                if !skip {
                    let l_other = if lr.v != pv { lr.next } else { l_radial };
                    let data_other = mods::math_get(bm, layer, l_other);
                    if mods::math_equal(&data_pivot, &data_other) {
                        stack.push(l_radial);
                        visit.insert(l_radial);
                    }
                }
                l_radial = bm.lp(l_radial).radial_next;
                if l_radial == l_radial_first {
                    break;
                }
            }
            // Also traverse the previous loop's edge for masked walks (#152249).
            let l_boundary = bm.lp(l_radial_first).prev;
            let be = bm.lp(l_boundary).e;
            if bm.edge_is_boundary(be) && mask_edge(bm, be, oflag) && visit.insert(l_boundary) {
                stack.push(l_boundary);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateFlag, VertId};

    #[test]
    fn collapsing_one_edge_of_a_quad_leaves_a_triangle() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        bm.face_create_verts(&v, None, CreateFlag::empty(), true);
        let e = bm.edge_exists(v[0], v[1]).unwrap();
        collapse(
            &mut bm,
            &CollapseIn {
                edges: vec![e],
                uvs: true,
            },
        )
        .unwrap();
        assert_eq!(bm.verts_num(), 3);
        assert_eq!(bm.vert(v[0]).co, [1.0, 0.0, 0.0]);
        assert_eq!(bm.face(crate::FaceId(0)).len, 3);
        assert!(bm.validate().is_empty());
    }
}
