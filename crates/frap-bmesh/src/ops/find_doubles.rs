// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.find_doubles` (Blender 5.2.4 `bmo_find_doubles_exec`, `operators/
//! bmo_removedoubles.cc`), implemented by WP 2.7 (Delete, dissolve, merge), with its companion
//! `bmesh.ops.remove_doubles` (`bmo_remove_doubles_exec`: `find_doubles` then `weld_verts`).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.find_doubles(bmesh, verts=[], keep_verts=[], use_connected=False, dist=0.0)`.
//!
//! **Create/kill order**: `find_doubles` changes nothing; it returns `targetmap.out` (in `verts`
//! order). Ported from blender/source/blender/bmesh/operators/bmo_removedoubles.cc
//! (`bmesh_find_doubles_common`): without `use_connected` the vertices are clustered with
//! Blender's kd-tree (`kdtree_calc_duplicates_cb`, ported in `frap_geom::kdtree`): `keep_verts`
//! first claim every vertex in range (closest keeper wins, lowest index on ties), then the
//! remaining vertices are searched in balanced-tree order and each cluster merges into the
//! vertex closest to its centroid (the lower index of a pair) — clusters are not chained
//! transitively. With `use_connected` only vertices reachable over edges (and across
//! the corners of n-gons) within `dist` of a cluster's first vertex are merged, in input
//! order. `remove_doubles` then welds (see `weld_verts`).
//!
//! Operator type flags: none (`find_doubles`); `NORMALS_CALC | SELECT_FLUSH |
//! SELECT_VALIDATE` (`remove_doubles`).

use rustc_hash::FxHashMap;

use crate::ops::weld_verts::{self, TargetMap};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `find_doubles` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FindDoublesIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `keep_verts`.
    pub keep_verts: Vec<VertId>,
    /// `use_connected`.
    pub use_connected: bool,
    /// `dist`.
    pub dist: f32,
}

/// Output slots of `find_doubles`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct FindDoublesOut {
    /// Every element the op created or kept as its result (always empty: nothing is created).
    pub geom: Vec<BMElem>,
    /// `targetmap.out`: each duplicate vertex and the vertex it merges into, in `verts` order.
    pub targetmap: Vec<(VertId, VertId)>,
}

const VERT_KEEP: u16 = 8;

fn check_alive(bm: &BMesh, op: &str, verts: &[VertId]) -> Result<(), BmError> {
    if let Some(v) = verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!("{op}: dead vertex {v:?}")));
    }
    Ok(())
}

/// Runs `find_doubles` on `bm`.
pub fn find_doubles(bm: &mut BMesh, i: &FindDoublesIn) -> Result<FindDoublesOut, BmError> {
    check_alive(bm, "find_doubles", &i.verts)?;
    check_alive(bm, "find_doubles", &i.keep_verts)?;
    let targetmap = bm.bmo_exec(OpTypeFlag::empty(), |bm| {
        find_doubles_common(bm, &i.verts, &i.keep_verts, i.use_connected, i.dist)
    });
    Ok(FindDoublesOut {
        geom: Vec::new(),
        targetmap,
    })
}

/// Input slots of `remove_doubles`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RemoveDoublesIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `use_connected`.
    pub use_connected: bool,
    /// `dist`.
    pub dist: f32,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_remove_doubles_def`.
pub const REMOVE_DOUBLES_TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// `bmesh.ops.remove_doubles`: finds the doubles of `verts` and welds them; returns the number
/// of vertices removed.
pub fn remove_doubles(bm: &mut BMesh, i: &RemoveDoublesIn) -> Result<usize, BmError> {
    check_alive(bm, "remove_doubles", &i.verts)?;
    let before = bm.verts_num();
    bm.bmo_exec(REMOVE_DOUBLES_TYPE_FLAG, |bm| {
        let pairs = find_doubles_common(bm, &i.verts, &[], i.use_connected, i.dist);
        let map: TargetMap = pairs.into_iter().collect();
        bm.bmo_exec(weld_verts::TYPE_FLAG, |bm| {
            weld_verts::weld_exec(bm, &map, false, false);
        });
    });
    Ok(before - bm.verts_num())
}

/// `bmesh_find_doubles_common`: the target map, in `verts` order.
pub(crate) fn find_doubles_common(
    bm: &mut BMesh,
    verts: &[VertId],
    keep_verts: &[VertId],
    use_connected: bool,
    dist: f32,
) -> Vec<(VertId, VertId)> {
    let has_keep_vert = !keep_verts.is_empty();
    for &v in keep_verts {
        bm.oflag_enable(v, VERT_KEEP);
    }
    let duplicates = if use_connected {
        find_doubles_connected(bm, verts, dist, has_keep_vert)
    } else {
        let coords: Vec<[f32; 3]> = verts.iter().map(|&v| bm.vert(v).co).collect();
        let keep: Option<Vec<bool>> = has_keep_vert.then(|| {
            verts
                .iter()
                .map(|&v| bm.oflag_test(v, VERT_KEEP))
                .collect()
        });
        frap_geom::kdtree::find_doubles_by_distance(&coords, dist, keep.as_deref())
    };
    let Some(duplicates) = duplicates else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (i, &d) in duplicates.iter().enumerate() {
        if d == -1 || d as usize == i {
            continue;
        }
        out.push((verts[i], verts[d as usize]));
    }
    out
}

/// `bmesh_find_doubles_by_distance_connected_impl`: `verts`-aligned targets (`-1`: none,
/// itself: kept), `None` when nothing was found.
fn find_doubles_connected(
    bm: &BMesh,
    verts: &[VertId],
    dist: f32,
    has_keep_vert: bool,
) -> Option<Vec<i32>> {
    let n = verts.len();
    let mut duplicates = vec![-1i32; n];
    let mut found_duplicates = false;
    let mut vert_stack: Vec<usize> = Vec::new();
    let mut index_of: FxHashMap<VertId, usize> = FxHashMap::default();
    for (i, &v) in verts.iter().enumerate() {
        if has_keep_vert && bm.oflag_test(v, VERT_KEEP) {
            duplicates[i] = i as i32;
        }
        // `Map::add` keeps the first index of a vertex given twice.
        index_of.entry(v).or_insert(i);
    }
    let dist_sq = dist * dist;
    for i in 0..n {
        if !(duplicates[i] == -1 || duplicates[i] == i as i32) {
            continue;
        }
        let co_check = bm.vert(verts[i]).co;
        let mut i_check = Some(i);
        while let Some(ic) = i_check {
            let v_check = verts[ic];
            let mut try_merge = |v_other: VertId, stack: &mut Vec<usize>, dup: &mut Vec<i32>| {
                if frap_geom::normals::len_squared_v3v3(bm.vert(v_other).co, co_check) < dist_sq
                    && let Some(&i_other) = index_of.get(&v_other)
                    && dup[i_other] == -1
                {
                    dup[i_other] = i as i32;
                    stack.push(i_other);
                    found_duplicates = true;
                }
            };
            for e_iter in bm.vert_edges(v_check) {
                // Edge stepping.
                let v_other = bm.edge_other_vert(e_iter, v_check);
                try_merge(v_other, &mut vert_stack, &mut duplicates);
                // Face stepping.
                for l_radial in bm.edge_loops(e_iter) {
                    let lr = bm.lp(l_radial);
                    if lr.v != v_check || bm.face(lr.f).len <= 3 {
                        continue;
                    }
                    // Every vertex not connected to an edge of `v_check`.
                    let l_end = lr.prev;
                    let mut l_iter = bm.lp(lr.next).next;
                    loop {
                        try_merge(bm.lp(l_iter).v, &mut vert_stack, &mut duplicates);
                        l_iter = bm.lp(l_iter).next;
                        if l_iter == l_end {
                            break;
                        }
                    }
                }
            }
            i_check = vert_stack.pop();
        }
    }
    found_duplicates.then_some(duplicates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    fn points(co: &[[f32; 3]]) -> (BMesh, Vec<VertId>) {
        let mut bm = BMesh::new();
        let v = co
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        (bm, v)
    }

    #[test]
    fn pairs_merge_as_in_blender() {
        // Verified on 5.2.4 (`bmesh.ops.find_doubles`): the search starts from the kd-tree's
        // balanced order, so `v0` merges into `v2` here.
        let (mut bm, v) = points(&[[0.0; 3], [5.0, 0.0, 0.0], [0.0, 0.0, 0.00001]]);
        let out = find_doubles(
            &mut bm,
            &FindDoublesIn {
                verts: v.clone(),
                dist: 0.001,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.targetmap, vec![(v[0], v[2])]);
        assert_eq!(bm.verts_num(), 3);
    }

    #[test]
    fn keep_verts_are_targets() {
        let (mut bm, v) = points(&[[0.0; 3], [0.0, 0.0, 0.00001]]);
        let out = find_doubles(
            &mut bm,
            &FindDoublesIn {
                verts: v.clone(),
                keep_verts: vec![v[1]],
                dist: 0.001,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.targetmap, vec![(v[0], v[1])]);
    }

    #[test]
    fn connected_only_follows_edges() {
        let (mut bm, v) = points(&[[0.0; 3], [0.0, 0.0, 0.00001], [0.0, 0.00001, 0.0]]);
        bm.edge_create(v[0], v[1], None, CreateFlag::empty());
        let out = find_doubles(
            &mut bm,
            &FindDoublesIn {
                verts: v.clone(),
                use_connected: true,
                dist: 0.001,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.targetmap, vec![(v[1], v[0])]);
    }

    #[test]
    fn remove_doubles_counts_removed_vertices() {
        let (mut bm, v) = points(&[[0.0; 3], [0.0, 0.0, 0.00001], [1.0, 0.0, 0.0]]);
        let n = remove_doubles(
            &mut bm,
            &RemoveDoublesIn {
                verts: v,
                dist: 0.001,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(n, 1);
        assert_eq!(bm.verts_num(), 2);
    }
}
