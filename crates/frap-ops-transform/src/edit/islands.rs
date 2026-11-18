// SPDX-License-Identifier: GPL-3.0-or-later
//! Islands for `INDIVIDUAL_ORIGINS` and connected proportional distances. Ported from
//! `transform_convert_mesh_islands_calc`, `bmesh_test_dist_add`, `bmesh_test_loose_edge` and
//! `transform_convert_mesh_connectivity_distance`
//! (`blender/source/blender/editors/transform/transform_convert_mesh.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Islands are the groups of selected edges (vertex and edge select modes) or of selected faces
//! sharing an edge (face select mode), each with the mean of its elements' centers and an axis
//! matrix from their summed normals and planes. Connected distances are geodesic distances
//! from the selection, propagated along edges and across faces (in world-scaled object space).

use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, SelectMode, VertId};
use glam::{Mat3, Vec3};

use super::bmq::{
    co, edge_groups_selected, editselection_center, editselection_normal, editselection_plane,
    face_groups_selected, has, vno,
};
use super::mathx::{
    axis_dominant_v3_to_m3, create_space_normal_tangent_or_fallback,
    geodesic_distance_propagate_across_triangle, invert_m3, is_zero, len, normalized,
};

/// `TransIslandData`.
#[derive(Clone, Debug, Default)]
pub struct Islands {
    /// Island center by island (object space), when requested.
    pub center: Option<Vec<Vec3>>,
    /// Island axis matrix by island, when requested.
    pub axismtx: Option<Vec<Mat3>>,
    /// Island of each vertex by dense vertex index (-1 for none); empty when not computed.
    pub vert_map: Vec<i32>,
}

/// `transform_convert_mesh_islands_calc`. `vidx` maps vertex slots to dense indices.
pub fn islands_calc(
    bm: &BMesh,
    vidx: &[i32],
    calc_single_islands: bool,
    calc_center: bool,
    calc_axismtx: bool,
) -> Islands {
    let (totvertsel, totedgesel, totfacesel) = bm.selected_counts();
    let only_single = totedgesel == 0 && totfacesel == 0;
    if only_single && !calc_single_islands {
        return Islands::default();
    }
    let mut data = Islands {
        center: None,
        axismtx: None,
        vert_map: vec![-1; bm.verts_num()],
    };
    let _ = totvertsel;
    let mut centers = Vec::new();
    let mut axes = Vec::new();
    let mut island_tot = 0i32;
    if !only_single {
        let groups: Vec<Vec<BMElem>> = if bm
            .select_mode
            .intersects(SelectMode::VERT | SelectMode::EDGE)
        {
            edge_groups_selected(bm)
                .into_iter()
                .map(|g| g.into_iter().map(BMElem::Edge).collect())
                .collect()
        } else {
            face_groups_selected(bm)
                .into_iter()
                .map(|g| g.into_iter().map(BMElem::Face).collect())
                .collect()
        };
        island_tot = groups.len() as i32;
        for (i, g) in groups.iter().enumerate() {
            let mut c = Vec3::ZERO;
            let mut no = Vec3::ZERO;
            let mut tangent = Vec3::ZERO;
            for ese in g {
                if calc_center {
                    c += editselection_center(bm, *ese);
                }
                if calc_axismtx {
                    no += editselection_normal(bm, *ese);
                    tangent += editselection_plane(bm, *ese, None);
                }
                let verts: Vec<VertId> = match ese {
                    BMElem::Edge(e) => bm.edge(*e).v.to_vec(),
                    BMElem::Face(f) => bm.face_verts(*f),
                    BMElem::Vert(v) => vec![*v],
                };
                for v in verts {
                    data.vert_map[vidx[v.slot()] as usize] = i as i32;
                }
            }
            if calc_center {
                centers.push(c * (1.0 / g.len() as f32));
            }
            if calc_axismtx {
                axes.push(create_space_normal_tangent_or_fallback(
                    normalized(no),
                    normalized(tangent),
                ));
            }
        }
    }
    if calc_single_islands {
        for v in bm.vert_ids() {
            let i = vidx[v.slot()] as usize;
            if has(bm, v, ElemFlag::SELECT) && data.vert_map[i] == -1 {
                data.vert_map[i] = island_tot;
                island_tot += 1;
                if calc_center {
                    centers.push(co(bm, v));
                }
                if calc_axismtx {
                    let n = vno(bm, v);
                    axes.push(if !is_zero(n) {
                        invert_m3(&axis_dominant_v3_to_m3(n)).unwrap_or(Mat3::ZERO)
                    } else {
                        Mat3::IDENTITY
                    });
                }
            }
        }
    }
    if calc_center {
        data.center = Some(centers);
    }
    if calc_axismtx {
        data.axismtx = Some(axes);
    }
    data
}

fn hidden(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    has(bm, e, ElemFlag::HIDDEN)
}

/// `bmesh_test_loose_edge`.
fn test_loose_edge(bm: &BMesh, e: EdgeId) -> bool {
    if bm.edge(e).l.is_none() {
        return true;
    }
    bm.edge_faces(e).iter().all(|f| hidden(bm, *f))
}

struct Dist<'a> {
    bm: &'a BMesh,
    vidx: &'a [i32],
    mtx: Mat3,
    dists: Vec<f32>,
    index: Option<Vec<i32>>,
}

impl Dist<'_> {
    fn i(&self, v: VertId) -> usize {
        self.vidx[v.slot()] as usize
    }
    /// `bmesh_test_dist_add`.
    fn test_add(&mut self, v0: VertId, v1: VertId, v2: Option<VertId>) -> bool {
        let bm = self.bm;
        if has(bm, v0, ElemFlag::SELECT) || hidden(bm, v0) {
            return false;
        }
        let (i0, i1) = (self.i(v0), self.i(v1));
        if self.dists[i0] <= self.dists[i1] {
            return false;
        }
        let dist0 = if let Some(v2) = v2 {
            let i2 = self.i(v2);
            if self.dists[i0] <= self.dists[i2] {
                return false;
            }
            let vm0 = self.mtx * co(bm, v0);
            let vm1 = self.mtx * co(bm, v1);
            let vm2 = self.mtx * co(bm, v2);
            geodesic_distance_propagate_across_triangle(
                vm0,
                vm1,
                vm2,
                self.dists[i1],
                self.dists[i2],
            )
        } else {
            let vec = self.mtx * (co(bm, v1) - co(bm, v0));
            self.dists[i1] + len(vec)
        };
        if dist0 < self.dists[i0] {
            self.dists[i0] = dist0;
            if let Some(index) = &mut self.index {
                index[i0] = index[i1];
            }
            return true;
        }
        false
    }
}

/// `transform_convert_mesh_connectivity_distance`: per dense vertex index, the geodesic
/// distance to the selection (`f32::MAX` when unreachable) and, with `want_index`, the index
/// of the selected vertex it was reached from.
pub fn connectivity_distance(
    bm: &BMesh,
    vidx: &[i32],
    mtx: &Mat3,
    want_index: bool,
) -> (Vec<f32>, Option<Vec<i32>>) {
    let verts = bm.vert_ids();
    let mut d = Dist {
        bm,
        vidx,
        mtx: *mtx,
        dists: vec![f32::MAX; verts.len()],
        index: want_index.then(|| (0..verts.len() as i32).collect()),
    };
    for (i, v) in verts.iter().enumerate() {
        d.dists[i] = if !has(bm, *v, ElemFlag::SELECT) || hidden(bm, *v) {
            f32::MAX
        } else {
            0.0
        };
    }
    let nedge = bm.edges.slots.len();
    let mut queued = vec![false; nedge];
    let mut loose = vec![false; nedge];
    // `BLI_LINKSTACK`: push and pop at the front, i.e. a LIFO stack.
    let mut queue: Vec<EdgeId> = Vec::new();
    for e in bm.edge_ids() {
        queued[e.slot()] = false;
        if hidden(bm, e) {
            continue;
        }
        let ed = bm.edge(e);
        let (i1, i2) = (d.i(ed.v[0]), d.i(ed.v[1]));
        if d.dists[i1] != f32::MAX || d.dists[i2] != f32::MAX {
            queue.push(e);
        }
        loose[e.slot()] = test_loose_edge(bm, e);
    }
    let mut queue_next: Vec<EdgeId> = Vec::new();
    loop {
        while let Some(e) = queue.pop() {
            frap_math::budget::check();
            let ed = bm.edge(e);
            let (mut v1, mut v2) = (ed.v[0], ed.v[1]);
            let (mut i1, mut i2) = (d.i(v1), d.i(v2));
            if loose[e.slot()] || d.dists[i1] == f32::MAX || d.dists[i2] == f32::MAX {
                if d.dists[i1] > d.dists[i2] {
                    std::mem::swap(&mut i1, &mut i2);
                    std::mem::swap(&mut v1, &mut v2);
                }
                let _ = (i1, i2);
                if d.test_add(v2, v1, None) {
                    let need_direct = loose[e.slot()]
                        || has(bm, v1, ElemFlag::SELECT)
                        || has(bm, v2, ElemFlag::SELECT);
                    for o in bm.vert_edges(v2) {
                        if o != e
                            && !queued[o.slot()]
                            && !hidden(bm, o)
                            && (need_direct
                                || loose[o.slot()]
                                || d.dists[d.i(bm.edge_other_vert(o, v2))] != f32::MAX)
                        {
                            queued[o.slot()] = true;
                            queue_next.push(o);
                        }
                    }
                }
            }
            if !loose[e.slot()] {
                for l in bm.edge_loops(e) {
                    let ll = bm.lp(l);
                    if hidden(bm, ll.f) {
                        continue;
                    }
                    let mut lo = bm.lp(ll.next).next;
                    while lo != l {
                        let v_other = bm.lp(lo).v;
                        if d.test_add(v_other, v1, Some(v2)) {
                            for o in bm.vert_edges(v_other) {
                                if o != e
                                    && !queued[o.slot()]
                                    && !hidden(bm, o)
                                    && (loose[o.slot()]
                                        || d.dists[d.i(bm.edge_other_vert(o, v_other))] != f32::MAX)
                                {
                                    queued[o.slot()] = true;
                                    queue_next.push(o);
                                }
                            }
                        }
                        lo = bm.lp(lo).next;
                    }
                }
            }
        }
        for e in &queue_next {
            queued[e.slot()] = false;
        }
        // `BLI_LINKSTACK_SWAP`: the next round pops the most recently pushed first.
        std::mem::swap(&mut queue, &mut queue_next);
        queue_next.clear();
        if queue.is_empty() {
            break;
        }
    }
    (d.dists, d.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_bmesh::{BMElem, FromMeshParams};

    fn cube() -> BMesh {
        let mut bm = BMesh::from_mesh(
            &frap_mesh::primitives::cube(2.0, false, [1.0; 3]),
            &FromMeshParams::default(),
        );
        bm.deselect_all();
        bm
    }

    #[test]
    fn connected_distance_along_the_cube() {
        let mut bm = cube();
        let v0 = bm.vert_ids()[0];
        bm.select_set(BMElem::Vert(v0), true);
        let (_, vidx) = crate::edit::bmq::vert_index(&bm);
        let (d, idx) = connectivity_distance(&bm, &vidx, &Mat3::IDENTITY, true);
        assert_eq!(d[0], 0.0);
        // The three edge neighbors at 2, the face diagonals at 2√2, the far corner at √20 (the
        // geodesic over two unfolded faces).
        assert!((d[7] - 20f32.sqrt()).abs() < 1e-5, "{d:?}");
        assert_eq!(d.iter().filter(|x| (**x - 2.0).abs() < 1e-6).count(), 3);
        assert!(idx.expect("index").iter().all(|i| *i == 0));
        // Distances are measured through the matrix.
        let (d2, _) = connectivity_distance(&bm, &vidx, &(Mat3::IDENTITY * 2.0), false);
        assert_eq!(d2.iter().filter(|x| (**x - 4.0).abs() < 1e-5).count(), 3);
    }

    #[test]
    fn islands_of_two_faces() {
        let mut bm = cube();
        bm.select_mode = frap_bmesh::SelectMode::FACE;
        let f = bm.face_ids();
        bm.select_set(BMElem::Face(f[0]), true);
        bm.select_set(BMElem::Face(f[2]), true);
        let (_, vidx) = crate::edit::bmq::vert_index(&bm);
        let isl = islands_calc(&bm, &vidx, false, true, true);
        let n = isl.center.as_ref().expect("centers").len();
        assert!(n == 1 || n == 2);
        assert_eq!(isl.axismtx.expect("axes").len(), n);
        assert!(isl.vert_map.iter().filter(|i| **i != -1).count() >= 6);
    }
}
