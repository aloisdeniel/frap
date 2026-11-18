// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_degenerate` (Blender 5.2.4 `bmo_dissolve_degenerate_exec`, `operators/
//! bmo_dissolve.cc`), implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_degenerate(bmesh, dist=0.0, edges=[])`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/bmo_dissolve.cc):
//! input edges shorter than `dist` are collapsed first (`bmesh.ops.collapse` with `uvs`, which
//! welds as `weld_verts` does). Then, in edge order and around each input edge's radial cycle,
//! every face corner whose two sides are (nearly) parallel within `dist` is clipped: equal
//! sides collapse the triangle's third edge or split the face across the corner; unequal
//! sides first split the longer side at the length of the shorter (`BM_edge_split`: a new
//! vertex and edge), then split the face across the corner (a new edge and face). The new
//! cross edges are collapsed in a second `collapse`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use crate::ops::collapse::collapse_exec;
use crate::ops::dissolve_verts::mods;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag};

/// Input slots of `dissolve_degenerate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveDegenerateIn {
    /// `dist`.
    pub dist: f32,
    /// `edges`.
    pub edges: Vec<EdgeId>,
}

/// Output slots of `dissolve_degenerate` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveDegenerateOut {
    /// Always empty (`bmesh.ops.dissolve_degenerate` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_dissolve_degenerate_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const EDGE_MARK: u16 = 1;
const EDGE_COLLAPSE: u16 = 2;

/// Runs `dissolve_degenerate` on `bm`.
pub fn dissolve_degenerate(
    bm: &mut BMesh,
    i: &DissolveDegenerateIn,
) -> Result<DissolveDegenerateOut, BmError> {
    if let Some(e) = i.edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "dissolve_degenerate: dead edge {e:?}"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| dissolve_degenerate_exec(bm, &i.edges, i.dist));
    Ok(DissolveDegenerateOut::default())
}

/// `bm_mesh_edge_collapse_flagged`: `collapse edges=%fe uvs=true` as a nested operator.
fn collapse_flagged(bm: &mut BMesh, oflag: u16) {
    let edges: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, oflag))
        .collect();
    bm.bmo_exec(crate::ops::collapse::TYPE_FLAG, |bm| collapse_exec(bm, &edges, true));
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dissolve_degenerate_exec(bm: &mut BMesh, edges: &[EdgeId], dist: f32) {
    let dist_sq = dist * dist;
    for &e in edges {
        bm.oflag_enable(e, EDGE_MARK);
    }

    // Collapse zero length edges, this accounts for zero area faces too.
    let mut found = false;
    for e in bm.edge_ids() {
        if bm.oflag_test(e, EDGE_MARK) && mods::edge_calc_length_squared(bm, e) < dist_sq {
            bm.oflag_enable(e, EDGE_COLLAPSE);
            found = true;
        }
        // Clear all loop tags (checked later).
        for l in bm.edge_loops(e) {
            bm.lp_mut(l).flag.remove(ElemFlag::TAG);
        }
    }
    if found {
        collapse_flagged(bm, EDGE_COLLAPSE);
    }

    // Clip degenerate ears from the face.
    found = false;
    for e in bm.edge_ids() {
        if !bm.edges.contains(e.0) || !bm.oflag_test(e, EDGE_MARK) {
            continue;
        }
        let Some(mut l_first) = bm.edge(e).l else {
            continue;
        };
        frap_math::budget::check();
        let mut l_iter = l_first;
        loop {
            let tagged = bm.lp(l_iter).flag.contains(ElemFlag::TAG);
            let test = !tagged && {
                bm.lp_mut(l_iter).flag.insert(ElemFlag::TAG);
                let li = bm.lp(l_iter);
                let prev_e = bm.lp(li.prev).e;
                bm.oflag_test(prev_e, EDGE_MARK)
                    && !bm.oflag_test(li.e, EDGE_COLLAPSE)
                    && !bm.oflag_test(prev_e, EDGE_COLLAPSE)
            };
            if test {
                // Test if the face's loop (ear) is degenerate.
                let li = bm.lp(l_iter).clone();
                let co = bm.vert(li.v).co;
                let (dir_prev, len_prev) =
                    crate::query::geom::normalize_v3(sub(bm.vert(bm.lp(li.prev).v).co, co));
                let (dir_next, len_next) =
                    crate::query::geom::normalize_v3(sub(bm.vert(bm.lp(li.next).v).co, co));
                if frap_geom::normals::len_v3v3(dir_prev, dir_next) * len_prev.min(len_next)
                    <= dist
                {
                    let mut reset = false;
                    if (len_prev - len_next).abs() <= dist {
                        // Both edges the same length.
                        if bm.face(li.f).len == 3 {
                            // Ideally this would have been discovered with the short edge test.
                            let ne = bm.lp(li.next).e;
                            bm.oflag_enable(ne, EDGE_COLLAPSE);
                            found = true;
                        } else if let Some((_, l_split)) =
                            bm.face_split(li.f, li.prev, li.next, None, true)
                        {
                            // Add a joining edge and tag for removal.
                            let se = bm.lp(l_split).e;
                            bm.oflag_enable(se, EDGE_COLLAPSE);
                            found = true;
                            reset = true;
                        }
                    } else if len_prev < len_next {
                        // Split `l_iter->e`, then join the vert with next.
                        bm.edge_split(li.e, li.v, len_prev / len_next);
                        let li = bm.lp(l_iter).clone();
                        if let Some((_, l_split)) = bm.face_split(li.f, li.prev, li.next, None, true)
                        {
                            let se = bm.lp(l_split).e;
                            bm.oflag_enable(se, EDGE_COLLAPSE);
                            found = true;
                        }
                        reset = true;
                    } else if len_next < len_prev {
                        // Split `l_iter->prev->e`, then join the vert with next.
                        let prev_e = bm.lp(li.prev).e;
                        bm.edge_split(prev_e, li.v, len_next / len_prev);
                        let li = bm.lp(l_iter).clone();
                        if let Some((_, l_split)) = bm.face_split(li.f, li.prev, li.next, None, true)
                        {
                            let se = bm.lp(l_split).e;
                            bm.oflag_enable(se, EDGE_COLLAPSE);
                            found = true;
                        }
                        reset = true;
                    }
                    if reset {
                        // We can't easily track where we are on the radial edge, reset!
                        l_first = l_iter;
                    }
                }
            }
            l_iter = bm.lp(l_iter).radial_next;
            if l_iter == l_first {
                break;
            }
        }
    }
    if found {
        collapse_flagged(bm, EDGE_COLLAPSE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateFlag, VertId};

    #[test]
    fn short_edges_collapse() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.00001, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
        .iter()
        .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
        .collect();
        bm.face_create_verts(&v, None, CreateFlag::empty(), true);
        let edges = bm.edge_ids();
        dissolve_degenerate(&mut bm, &DissolveDegenerateIn { dist: 0.001, edges }).unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.verts_num(), 4);
        assert_eq!(bm.faces_num(), 1);
    }
}
