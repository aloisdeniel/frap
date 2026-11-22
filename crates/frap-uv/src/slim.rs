// SPDX-License-Identifier: GPL-3.0-or-later
//! SLIM (`method='MINIMUM_STRETCH'`): the parametrizer side of `uv_parametrizer.cc`'s SLIM
//! integration (`slim_convert_blender`, `slim_transfer_*`, `slim_flush_uvs`,
//! `uv_parametrizer_slim_solve`) and Blender's `intern/slim` library ([`solver`], [`init`]),
//! built on `faer` instead of Eigen.
//!
//! Per chart: degenerate triangles are corrected in the parametrizer's copy of the positions,
//! the outer boundary gets SLIM ids `0..#boundary` and the other vertices the following ones,
//! the initial map puts the boundary on a circle and the interior at mean value coordinates
//! (fitted to the pins when there are some), and `iterations` SLIM iterations minimise the
//! symmetric Dirichlet energy, optionally weighted per face by a vertex group (`use_weights`,
//! `weight_factor`). A chart with an edge shorter than 1e-5 (SLIM does not support doubled
//! vertices) or whose solve fails gets zero UVs and counts as failed, as in Blender.
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc and
//! blender/intern/slim (Blender 5.2.4).

mod degenerate;
pub(crate) mod init;
pub(crate) mod solver;
pub(crate) mod sparse;
pub(crate) mod svd;

use crate::param::{NIL, OrigBounds, PVERT_PIN, PVERT_SELECT, ParamMesh, PhState, SolveCount};

/// `SlimFailedException`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlimFailed;

/// `ParamSlimOptions`.
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SlimOptions {
    /// `weight_influence` (`weight_factor`; 0 without `use_weights` or a weight group).
    pub weight_influence: f32,
    /// `iterations`.
    pub iterations: i32,
    /// `no_flip` (SLIM's `reflection_mode`).
    pub no_flip: bool,
    /// `skip_init`: start from the current UVs when they have no flipped face (live unwrap).
    pub skip_init: bool,
}

impl SlimOptions {
    /// The options of `uv.unwrap`: `iterations`, `no_flip`, `weight_influence`.
    pub fn new(iterations: i32, no_flip: bool, weight_influence: f32) -> SlimOptions {
        SlimOptions {
            weight_influence,
            iterations,
            no_flip,
            skip_init: false,
        }
    }
}

/// `MatrixTransferChart`: one chart's data in SLIM's layout (column-major matrices).
#[derive(Clone, Debug, Default)]
pub(crate) struct MatrixTransferChart {
    pub(crate) verts_num: usize,
    pub(crate) faces_num: usize,
    pub(crate) pinned_vertices_num: usize,
    pub(crate) boundary_vertices_num: usize,
    pub(crate) edges_num: usize,
    pub(crate) succeeded: bool,
    pub(crate) v_matrices: Vec<f64>,
    pub(crate) uv_matrices: Vec<f64>,
    pub(crate) pp_matrices: Vec<f64>,
    pub(crate) el_vectors: Vec<f64>,
    pub(crate) w_vectors: Vec<f32>,
    pub(crate) f_matrices: Vec<i32>,
    pub(crate) p_matrices: Vec<i32>,
    pub(crate) e_matrices: Vec<i32>,
    pub(crate) b_vectors: Vec<i32>,
}

/// `MatrixTransfer`'s per-solve options.
#[derive(Copy, Clone, Debug)]
struct Transfer {
    use_weights: bool,
    is_minimize_stretch: bool,
}

/// `SLIM_CORR_MIN_AREA`.
const SLIM_CORR_MIN_AREA: f32 = 1.0e-8;
/// `DOUBLED_VERT_THRESHOLD`.
const DOUBLED_VERT_THRESHOLD: f32 = 1.0e-5;

impl ParamMesh {
    /// `slim_transfer_boundary_vertices`.
    fn slim_transfer_boundary_vertices(
        &mut self,
        c: usize,
        mt: &mut MatrixTransferChart,
        tr: Transfer,
    ) -> bool {
        for v in self.chart_vert_list(c) {
            self.verts[v as usize].on_boundary_flag = false;
        }
        let outer = self.chart_boundaries(c);
        if outer == NIL {
            return false;
        }
        let mut be = outer;
        let mut vid = 0usize;
        loop {
            frap_math::budget::check();
            let v = self.edges[be as usize].vert;
            if tr.use_weights {
                mt.w_vectors[vid] = self.verts[v as usize].weight;
            }
            mt.boundary_vertices_num += 1;
            self.verts[v as usize].slim_id = vid as i32;
            self.verts[v as usize].on_boundary_flag = true;
            mt.b_vectors[vid] = vid as i32;
            vid += 1;
            be = self.boundary_edge_next(be);
            if be == outer {
                break;
            }
        }
        true
    }

    /// `slim_transfer_vertices`.
    fn slim_transfer_vertices(&mut self, c: usize, mt: &mut MatrixTransferChart, tr: Transfer) {
        let r = mt.verts_num;
        let mut p_vid = 0usize;
        let mut vid = mt.boundary_vertices_num;
        for v in self.chart_vert_list(c) {
            let pv = &mut self.verts[v as usize];
            if !pv.on_boundary_flag {
                if tr.use_weights {
                    mt.w_vectors[vid] = pv.weight;
                }
                pv.slim_id = vid as i32;
                vid += 1;
            }
            let id = pv.slim_id as usize;
            mt.v_matrices[id] = f64::from(pv.co[0]);
            mt.v_matrices[r + id] = f64::from(pv.co[1]);
            mt.v_matrices[2 * r + id] = f64::from(pv.co[2]);
            mt.uv_matrices[id] = f64::from(pv.uv[0]);
            mt.uv_matrices[r + id] = f64::from(pv.uv[1]);
            if pv.flag & PVERT_PIN != 0 || (tr.is_minimize_stretch && pv.flag & PVERT_SELECT == 0) {
                mt.pinned_vertices_num += 1;
                mt.p_matrices[p_vid] = id as i32;
                mt.pp_matrices[2 * p_vid] = f64::from(pv.uv[0]);
                mt.pp_matrices[2 * p_vid + 1] = f64::from(pv.uv[1]);
                p_vid += 1;
            }
        }
    }

    /// `slim_transfer_edges`.
    fn slim_transfer_edges(&mut self, c: usize, mt: &mut MatrixTransferChart) {
        let outer = self.chart_boundaries(c);
        let off = mt.edges_num + mt.boundary_vertices_num;
        let mut eid = 0usize;
        let mut be = outer;
        loop {
            let v = self.edges[be as usize].vert;
            mt.e_matrices[eid] = self.verts[v as usize].slim_id;
            let edge_len = self.edge_length(be);
            mt.el_vectors[eid] = f64::from(edge_len);
            // SLIM does not support doubled vertices.
            if edge_len < DOUBLED_VERT_THRESHOLD {
                mt.succeeded = false;
            }
            be = self.boundary_edge_next(be);
            let v = self.edges[be as usize].vert;
            mt.e_matrices[eid + off] = self.verts[v as usize].slim_id;
            eid += 1;
            if be == outer {
                break;
            }
        }
        for e in self.chart_edge_list(c) {
            let e1 = self.edges[e as usize].next;
            let v = self.edges[e as usize].vert;
            mt.e_matrices[eid] = self.verts[v as usize].slim_id;
            let edge_len = self.edge_length(e);
            mt.el_vectors[eid] = f64::from(edge_len);
            if edge_len < DOUBLED_VERT_THRESHOLD {
                mt.succeeded = false;
            }
            let v1 = self.edges[e1 as usize].vert;
            mt.e_matrices[eid + off] = self.verts[v1 as usize].slim_id;
            eid += 1;
        }
    }

    /// `slim_transfer_faces`.
    fn slim_transfer_faces(&self, c: usize, mt: &mut MatrixTransferChart) {
        let r = mt.faces_num;
        for (fid, f) in self.chart_face_list(c).into_iter().enumerate() {
            let [v1, v2, v3] = self.face_verts(f);
            mt.f_matrices[fid] = self.verts[v1 as usize].slim_id;
            mt.f_matrices[r + fid] = self.verts[v2 as usize].slim_id;
            mt.f_matrices[2 * r + fid] = self.verts[v3 as usize].slim_id;
        }
    }

    /// One chart of `slim_convert_blender` (`slim_allocate_matrices` and the transfers).
    fn slim_convert_chart(
        &mut self,
        c: usize,
        tr: Transfer,
        use_original_bounds: bool,
    ) -> MatrixTransferChart {
        if use_original_bounds {
            // `p_chart_orig_bounds_init`.
            let angle = self.chart_minimum_area_angle(c);
            let (min, max) = self.chart_uv_bbox(c);
            self.charts[c].orig_bounds = Some(OrigBounds { min, max, angle });
        }
        self.chart_correct_degenerate_triangles(c, SLIM_CORR_MIN_AREA, 1.0f32.to_radians());

        let chart = &self.charts[c];
        let (nv, nf, ne) = (
            chart.nverts.max(0) as usize,
            chart.nfaces.max(0) as usize,
            chart.nedges.max(0) as usize,
        );
        let mut mt = MatrixTransferChart {
            verts_num: nv,
            faces_num: nf,
            edges_num: ne,
            succeeded: true,
            v_matrices: vec![0.0; nv * 3],
            uv_matrices: vec![0.0; nv * 2],
            pp_matrices: vec![0.0; nv * 2],
            f_matrices: vec![0; nf * 3],
            p_matrices: vec![0; nv],
            b_vectors: vec![0; nv],
            w_vectors: vec![0.0; nv],
            e_matrices: vec![0; ne * 2 * 2],
            el_vectors: vec![0.0; ne * 2],
            ..MatrixTransferChart::default()
        };
        if !self.slim_transfer_boundary_vertices(c, &mut mt, tr) {
            // No boundary (non-finite coordinates): Blender has no outer edge either.
            mt.succeeded = false;
            return mt;
        }
        self.slim_transfer_vertices(c, &mut mt, tr);
        self.slim_transfer_edges(c, &mut mt);
        self.slim_transfer_faces(c, &mut mt);
        mt.pp_matrices.truncate(mt.pinned_vertices_num * 2);
        mt.p_matrices.truncate(mt.pinned_vertices_num);
        mt.b_vectors.truncate(mt.boundary_vertices_num);
        mt.e_matrices
            .resize((mt.edges_num + mt.boundary_vertices_num) * 2, 0);
        mt
    }

    /// `slim_flush_uvs` for one chart (not live).
    fn slim_flush_chart(&mut self, c: usize, mt: &MatrixTransferChart, count: &mut SolveCount) {
        let verts = self.chart_vert_list(c);
        if mt.succeeded {
            count.changed += 1;
            for v in verts {
                let pv = &mut self.verts[v as usize];
                if pv.flag & PVERT_PIN != 0 {
                    continue;
                }
                let vid = pv.slim_id as usize;
                pv.uv = [
                    mt.uv_matrices[vid] as f32,
                    mt.uv_matrices[mt.verts_num + vid] as f32,
                ];
            }
        } else {
            count.failed += 1;
            for v in verts {
                self.verts[v as usize].uv = [0.0, 0.0];
            }
        }
    }

    /// `uv_parametrizer_slim_solve`: SLIM on every chart, then the flattened UVs are in the
    /// parametrizer (flush with [`ParamMesh::flush`] after averaging/packing).
    pub fn slim_solve(&mut self, options: &SlimOptions, use_original_bounds: bool) -> SolveCount {
        assert_eq!(self.state, PhState::Constructed);
        // `slim_matrix_transfer`.
        let tr = Transfer {
            use_weights: options.weight_influence > 0.0,
            is_minimize_stretch: false,
        };
        let topts = init::TransferOptions {
            use_weights: tr.use_weights,
            weight_influence: f64::from(options.weight_influence),
            reflection_mode: i32::from(options.no_flip),
            skip_initialization: options.skip_init,
        };
        // `slim_convert_blender`.
        let mut mts = Vec::with_capacity(self.charts.len());
        for c in 0..self.charts.len() {
            mts.push(self.slim_convert_chart(c, tr, use_original_bounds));
        }
        // `MatrixTransfer::parametrize`.
        for mt in &mut mts {
            let mut data = init::setup_slim_data(mt, &topts);
            if !data.valid {
                mt.succeeded = false;
            }
            if mt.succeeded && solver::slim_solve(&mut data, options.iterations).is_err() {
                mt.succeeded = false;
            }
            init::correct_map_surface_area_if_necessary(&mut data);
            if mt.succeeded {
                let n = mt.verts_num;
                for (i, p) in data.v_o.iter().enumerate().take(n) {
                    mt.uv_matrices[i] = p[0];
                    mt.uv_matrices[n + i] = p[1];
                }
            }
        }
        let mut count = SolveCount::default();
        for (c, mt) in mts.iter().enumerate() {
            self.slim_flush_chart(c, mt, &mut count);
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::param::{ConstructOpts, ParamFace};

    /// A 4 × 4 grid of quads on a cylinder section (developable).
    fn curved_grid(pm: &mut ParamMesh) {
        let n = 5usize;
        let co = |i: usize, j: usize| -> [f32; 3] {
            let a = i as f32 * 0.3;
            [libm::sinf(a), j as f32 * 0.3, 1.0 - libm::cosf(a)]
        };
        let mut slot = 0u32;
        let mut key = 0u64;
        for i in 0..n - 1 {
            for j in 0..n - 1 {
                let ids = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)];
                let vkeys: Vec<u64> = ids.iter().map(|&(a, b)| (a * n + b) as u64).collect();
                let cos: Vec<[f32; 3]> = ids.iter().map(|&(a, b)| co(a, b)).collect();
                let uvi: Vec<u32> = (0..4).map(|k| slot + k).collect();
                slot += 4;
                let uv = vec![[0.0f32; 2]; 4];
                let pin = vec![false; 4];
                let sel = vec![true; 4];
                pm.face_add(&ParamFace::new(key, &vkeys, &cos, &uvi, &uv, &pin, &sel));
                key += 1;
            }
        }
    }

    #[test]
    fn slim_flattens_a_developable_patch_isometrically() {
        let mut pm = ParamMesh::new();
        curved_grid(&mut pm);
        pm.construct_end(ConstructOpts::new(true, false));
        let count = pm.slim_solve(&SlimOptions::new(30, false, 0.0), false);
        assert_eq!((count.changed, count.failed), (1, 0));
        for e in pm.chart_edge_list(0) {
            let a = pm.edges[e as usize].vert;
            let b = pm.edges[pm.edges[e as usize].next as usize].vert;
            let l3 = crate::param::bli::len_v3v3(pm.verts[a as usize].co, pm.verts[b as usize].co);
            let l2 = crate::param::bli::len_v2v2(pm.verts[a as usize].uv, pm.verts[b as usize].uv);
            assert!((l2 / l3 - 1.0).abs() < 2e-2, "{l2} vs {l3}");
        }
    }

    #[test]
    fn needle_triangle_is_corrected_before_slim() {
        // An angle far below 1° is corrected in the parametrizer's positions, then solved.
        let mut pm = ParamMesh::new();
        let co = [[0.0f32, 0.0, 0.0], [1e-3, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let uv = [[0.0f32; 2]; 3];
        let f = ParamFace::new(0, &[0, 1, 2], &co, &[0, 1, 2], &uv, &[false; 3], &[true; 3]);
        pm.face_add(&f);
        pm.construct_end(ConstructOpts::new(true, false));
        let count = pm.slim_solve(&SlimOptions::new(10, false, 0.0), false);
        assert_eq!((count.changed, count.failed), (1, 0));
        let moved = pm.chart_vert_list(0).iter().any(|&v| {
            let c = pm.verts[v as usize].co;
            !co.contains(&c)
        });
        assert!(moved);
        for v in pm.chart_vert_list(0) {
            assert!(pm.verts[v as usize].uv.iter().all(|x| x.is_finite()));
        }
    }
}
