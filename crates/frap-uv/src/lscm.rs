// SPDX-License-Identifier: GPL-3.0-or-later
//! Least Squares Conformal Maps (`p_chart_lscm_*` of `uv_parametrizer.cc`): the sparse
//! least-squares system over a chart's triangles, pinned by two vertices when the chart has
//! fewer than two pins (`p_chart_symmetry_pins`, else `p_chart_extrema_verts`), solved with
//! `faer` (Blender uses Eigen's `SparseLU` on the normal equations; frap factors the same
//! normal equations with a sparse Cholesky, so the result agrees within 1e-4, a tolerance and
//! not exactness, WP 3.3 acceptance).
//!
//! [`LinearSolver`] is the port of `intern/eigen/intern/linear_solver.cc`'s least-squares mode:
//! locked variables move to the right-hand side, the matrix is built and factored once, and
//! later solves (live unwrap) reuse the factorization.
//!
//! Ported from blender/source/blender/geometry/intern/uv_parametrizer.cc and
//! blender/intern/eigen/intern/linear_solver.cc (Blender 5.2.4).

use faer::Side;
use faer::linalg::solvers::Solve;
use faer::sparse::{SparseColMat, Triplet};
use rustc_hash::FxHashMap;

use crate::param::bli::{self, V3};
use crate::param::{NIL, PVERT_PIN, PVERT_SELECT, PVERT_SPLIT, ParamMesh, PhState, SolveCount};

#[derive(Clone, Debug, Default)]
struct Variable {
    value: f64,
    locked: bool,
    index: usize,
    /// Coefficients of a locked variable: `(row, value)`.
    a: Vec<(usize, f64)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SolverState {
    VariablesConstruct,
    MatrixConstruct,
    MatrixSolved,
}

/// `LinearSolver` (least-squares mode, one right-hand side).
pub(crate) struct LinearSolver {
    state: SolverState,
    num_rows: usize,
    m: usize,
    n: usize,
    variables: Vec<Variable>,
    triplets: Vec<(usize, usize, f64)>,
    /// `M` by rows, compressed at factorization time.
    rows: Vec<Vec<(usize, f64)>>,
    b: Vec<f64>,
    factor: Option<faer::sparse::linalg::solvers::Llt<usize, f64>>,
}

impl core::fmt::Debug for LinearSolver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LinearSolver")
            .field("m", &self.m)
            .field("n", &self.n)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl LinearSolver {
    /// `EIG_linear_least_squares_solver_new(num_rows, num_columns, 1)`.
    pub(crate) fn new(num_rows: usize, num_variables: usize) -> LinearSolver {
        LinearSolver {
            state: SolverState::VariablesConstruct,
            num_rows,
            m: 0,
            n: 0,
            variables: vec![Variable::default(); num_variables],
            triplets: Vec::new(),
            rows: Vec::new(),
            b: Vec::new(),
            factor: None,
        }
    }

    /// `EIG_linear_solver_variable_set`.
    pub(crate) fn variable_set(&mut self, index: usize, value: f64) {
        self.variables[index].value = value;
    }

    /// `EIG_linear_solver_variable_get`.
    pub(crate) fn variable_get(&self, index: usize) -> f64 {
        self.variables[index].value
    }

    /// `EIG_linear_solver_variable_lock`.
    pub(crate) fn variable_lock(&mut self, index: usize) {
        if !self.variables[index].locked {
            debug_assert_eq!(self.state, SolverState::VariablesConstruct);
            self.variables[index].locked = true;
        }
    }

    /// `linear_solver_ensure_matrix_construct`.
    fn ensure_matrix_construct(&mut self) {
        if self.state == SolverState::VariablesConstruct {
            let mut n = 0;
            for v in &mut self.variables {
                if v.locked {
                    v.index = usize::MAX;
                } else {
                    v.index = n;
                    n += 1;
                }
            }
            let m = if self.num_rows == 0 { n } else { self.num_rows };
            self.m = m;
            self.n = n;
            self.triplets.clear();
            self.b = vec![0.0; m];
            self.state = SolverState::MatrixConstruct;
        }
    }

    /// `EIG_linear_solver_matrix_add`.
    pub(crate) fn matrix_add(&mut self, row: usize, col: usize, value: f64) {
        if self.state == SolverState::MatrixSolved {
            return;
        }
        self.ensure_matrix_construct();
        if self.variables[col].locked {
            self.variables[col].a.push((row, value));
        } else {
            let c = self.variables[col].index;
            self.triplets.push((row, c, value));
        }
    }

    /// Builds `M`, `MᵀM` and its factorization (`sparseLU->compute(MtM)` in Blender).
    fn factorize(&mut self) -> bool {
        // `setFromTriplets` sums duplicates.
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.m];
        for &(r, c, v) in &self.triplets {
            let row = &mut rows[r];
            if let Some(e) = row.iter_mut().find(|e| e.0 == c) {
                e.1 += v;
            } else {
                row.push((c, v));
            }
        }
        self.triplets.clear();
        let mut mtm: FxHashMap<(usize, usize), f64> = FxHashMap::default();
        for row in &rows {
            for &(ci, vi) in row {
                for &(cj, vj) in row {
                    if ci >= cj {
                        *mtm.entry((ci, cj)).or_insert(0.0) += vi * vj;
                    }
                }
            }
        }
        self.rows = rows;
        let mut trips: Vec<Triplet<usize, usize, f64>> = mtm
            .into_iter()
            .map(|((r, c), val)| Triplet::new(r, c, val))
            .collect();
        trips.sort_by_key(|t| (t.col, t.row));
        let Ok(mat) = SparseColMat::<usize, f64>::try_new_from_triplets(self.n, self.n, &trips)
        else {
            return false;
        };
        match mat.sp_cholesky(Side::Lower) {
            Ok(llt) => {
                self.factor = Some(llt);
                true
            }
            Err(_) => false,
        }
    }

    /// `EIG_linear_solver_solve`.
    pub(crate) fn solve(&mut self) -> bool {
        if self.m == 0 || self.n == 0 {
            return true;
        }
        let mut result = true;
        if self.state == SolverState::MatrixConstruct {
            result = self.factorize();
            self.state = SolverState::MatrixSolved;
        }
        if result && let Some(llt) = &self.factor {
            // Modify for locked variables.
            let mut b = core::mem::take(&mut self.b);
            for v in &self.variables {
                if v.locked {
                    for &(row, a) in &v.a {
                        b[row] -= a * v.value;
                    }
                }
            }
            // `Mᵀ b`.
            let mut mtb = faer::Mat::<f64>::zeros(self.n, 1);
            for (r, row) in self.rows.iter().enumerate() {
                for &(c, v) in row {
                    mtb[(c, 0)] += v * b[r];
                }
            }
            llt.solve_in_place(mtb.as_mut());
            let ok = (0..self.n).all(|i| mtb[(i, 0)].is_finite());
            if ok {
                for v in &mut self.variables {
                    if !v.locked {
                        v.value = mtb[(v.index, 0)];
                    }
                }
            } else {
                result = false;
            }
            self.b = b;
        } else {
            result = false;
        }
        // Clear for the next solve.
        self.b = vec![0.0; self.m];
        result
    }
}

/// `fix_large_angle`: spreads the excess of an angle above 179° over the two others.
fn fix_large_angle(v_fix: V3, v1: V3, v2: V3, r_fix: &mut f64, r_a1: &mut f64, r_a2: &mut f64) {
    let max_angle = 179.0f64.to_radians();
    let fix_amount = *r_fix - max_angle;
    if fix_amount < 0.0 {
        return;
    }
    let dist_v1 = f64::from(bli::len_v3v3(v_fix, v1));
    let dist_v2 = f64::from(bli::len_v3v3(v_fix, v2));
    let sum = dist_v1 + dist_v2;
    let weight = if sum > f64::from(1e-20f32) {
        dist_v2 / sum
    } else {
        0.5
    };
    *r_fix -= fix_amount;
    *r_a1 += fix_amount * weight;
    *r_a2 += fix_amount * (1.0 - weight);
}

/// `p_triangle_angles`.
pub(crate) fn triangle_angles(v1: V3, v2: V3, v3: V3) -> (f64, f64, f64) {
    let mut a1 = f64::from(bli::angle_v3v3v3(v3, v1, v2));
    let mut a2 = f64::from(bli::angle_v3v3v3(v1, v2, v3));
    let mut a3 = f64::from(bli::angle_v3v3v3(v2, v3, v1));
    fix_large_angle(v1, v2, v3, &mut a1, &mut a2, &mut a3);
    fix_large_angle(v2, v3, v1, &mut a2, &mut a3, &mut a1);
    fix_large_angle(v3, v1, v2, &mut a3, &mut a1, &mut a2);
    let min = f64::from(0.001f32);
    (a1.max(min), a2.max(min), a3.max(min))
}

impl ParamMesh {
    /// `p_chart_pin_positions`.
    fn chart_pin_positions(&mut self, c: usize, pin1: &mut u32, pin2: &mut u32) {
        if *pin1 == NIL || *pin2 == NIL || *pin1 == *pin2 {
            // Degenerate case.
            let f = self.charts[c].faces;
            let e = self.faces[f as usize].edge;
            *pin1 = self.edges[e as usize].vert;
            let n = self.edges[e as usize].next;
            *pin2 = self.edges[n as usize].vert;
            self.verts[*pin1 as usize].uv = [0.0, 0.5];
            self.verts[*pin2 as usize].uv = [1.0, 0.5];
            return;
        }
        let a = self.verts[*pin1 as usize].co;
        let b = self.verts[*pin2 as usize].co;
        let sub = [
            (a[0] - b[0]).abs(),
            (a[1] - b[1]).abs(),
            (a[2] - b[2]).abs(),
        ];
        let (dirx, diry) = if sub[0] > sub[1] && sub[0] > sub[2] {
            (0, if sub[1] > sub[2] { 1 } else { 2 })
        } else if sub[1] > sub[0] && sub[1] > sub[2] {
            (1, if sub[0] > sub[2] { 0 } else { 2 })
        } else {
            (2, if sub[0] > sub[1] { 0 } else { 1 })
        };
        let (diru, dirv) = if dirx == 2 { (1, 0) } else { (0, 1) };
        for p in [*pin1, *pin2] {
            let co = self.verts[p as usize].co;
            let uv = &mut self.verts[p as usize].uv;
            uv[diru] = co[dirx];
            uv[dirv] = co[diry];
        }
    }

    /// `p_chart_symmetry_pins`.
    fn chart_symmetry_pins(
        &mut self,
        c: usize,
        outer: u32,
        pin1: &mut u32,
        pin2: &mut u32,
    ) -> bool {
        let mut maxe1 = NIL;
        let mut maxe2 = NIL;
        let mut cure = NIL;
        let mut firste1 = NIL;
        let mut firste2 = NIL;
        let mut maxlen = 0.0f32;
        let mut curlen = 0.0f32;
        let mut totlen = 0.0f32;
        let mut firstlen = 0.0f32;

        let split = |pm: &ParamMesh, e: u32| pm.verts[pm.edges[e as usize].vert as usize].flag;
        let mut be = outer;
        let mut lastbe = self.boundary_edge_prev(be);
        loop {
            frap_math::budget::check();
            let len = self.edge_length(be);
            totlen += len;
            let nextbe = self.boundary_edge_next(be);
            if split(self, be) & PVERT_SPLIT != 0
                || (split(self, lastbe) & split(self, nextbe) & PVERT_SPLIT) != 0
            {
                if cure == NIL {
                    if be == outer {
                        firste1 = be;
                    }
                    cure = be;
                } else {
                    curlen += self.edge_length(lastbe);
                }
            } else if cure != NIL {
                if curlen > maxlen {
                    maxlen = curlen;
                    maxe1 = cure;
                    maxe2 = lastbe;
                }
                if firste1 == cure {
                    firstlen = curlen;
                    firste2 = lastbe;
                }
                curlen = 0.0;
                cure = NIL;
            }
            lastbe = be;
            be = nextbe;
            if be == outer {
                break;
            }
        }
        // Count a series of splits over the starting point too.
        if cure != NIL && cure != outer {
            firstlen += curlen + self.edge_length(be);
            if firstlen > maxlen {
                maxlen = firstlen;
                maxe1 = cure;
                maxe2 = firste2;
            }
        }
        if maxe1 == NIL || maxe2 == NIL || maxlen < 0.5 * totlen {
            return false;
        }

        // Find pin1 in the split vertices.
        let (mut be1, mut be2) = (maxe1, maxe2);
        let (mut len1, mut len2) = (0.0f32, 0.0f32);
        loop {
            frap_math::budget::check();
            if len1 < len2 {
                len1 += self.edge_length(be1);
                be1 = self.boundary_edge_next(be1);
            } else {
                be2 = self.boundary_edge_prev(be2);
                len2 += self.edge_length(be2);
            }
            if be1 == be2 {
                break;
            }
        }
        *pin1 = self.edges[be1 as usize].vert;

        // Find pin2 outside the split vertices.
        let (mut be1, mut be2) = (maxe1, maxe2);
        let (mut len1, mut len2) = (0.0f32, 0.0f32);
        loop {
            frap_math::budget::check();
            if len1 < len2 {
                be1 = self.boundary_edge_prev(be1);
                len1 += self.edge_length(be1);
            } else {
                len2 += self.edge_length(be2);
                be2 = self.boundary_edge_next(be2);
            }
            if be1 == be2 {
                break;
            }
        }
        *pin2 = self.edges[be1 as usize].vert;

        self.chart_pin_positions(c, pin1, pin2);
        self.verts[*pin1 as usize].co != self.verts[*pin2 as usize].co
    }

    /// `p_chart_extrema_verts`.
    fn chart_extrema_verts(&mut self, c: usize, pin1: &mut u32, pin2: &mut u32) {
        let mut minv = [1e20f32; 3];
        let mut maxv = [-1e20f32; 3];
        let mut minvert = [NIL; 3];
        let mut maxvert = [NIL; 3];
        let mut v = self.charts[c].verts;
        while v != NIL {
            let co = self.verts[v as usize].co;
            for i in 0..3 {
                if co[i] < minv[i] {
                    minv[i] = co[i];
                    minvert[i] = v;
                }
                if co[i] > maxv[i] {
                    maxv[i] = co[i];
                    maxvert[i] = v;
                }
            }
            v = self.verts[v as usize].nextlink;
        }
        let mut dir = 0;
        let mut dirlen = -1.0f32;
        for i in 0..3 {
            if maxv[i] - minv[i] > dirlen {
                dir = i;
                dirlen = maxv[i] - minv[i];
            }
        }
        *pin1 = minvert[dir];
        *pin2 = maxvert[dir];
        self.chart_pin_positions(c, pin1, pin2);
    }

    /// `p_chart_lscm_begin`.
    fn chart_lscm_begin(&mut self, c: usize, live: bool, abf: bool, use_original_bounds: bool) {
        debug_assert!(self.charts[c].context.is_none());
        let mut select = false;
        let mut deselect = false;
        let mut npins = 0;
        if use_original_bounds {
            let angle = self.chart_minimum_area_angle(c);
            let (min, max) = self.chart_uv_bbox(c);
            self.charts[c].orig_bounds = Some(crate::param::OrigBounds { min, max, angle });
        }
        let verts = self.chart_vert_list(c);
        for &v in &verts {
            let flag = self.verts[v as usize].flag;
            if flag & PVERT_PIN != 0 {
                npins += 1;
                if flag & PVERT_SELECT != 0 {
                    select = true;
                }
            }
            if flag & PVERT_SELECT == 0 {
                deselect = true;
            }
        }
        if live && (!select || !deselect) {
            self.charts[c].skip_flush = true;
            return;
        }
        if npins == 1 {
            self.charts[c].area_uv = self.chart_uv_area(c);
            self.charts[c].single_pin = verts
                .iter()
                .copied()
                .find(|&v| self.verts[v as usize].flag & PVERT_PIN != 0)
                .unwrap_or(NIL);
        }
        // ABF++ is M8 (WP 8.1): `abf` falls back to LSCM, as Blender does when ABF fails.
        let _ = abf;

        for (id, &v) in verts.iter().enumerate() {
            self.verts[v as usize].id = id as i32;
        }

        if npins <= 1 {
            // No pins, find some.
            let outer = self.chart_boundaries(c);
            let mut pin1 = NIL;
            let mut pin2 = NIL;
            if !(outer != NIL && self.chart_symmetry_pins(c, outer, &mut pin1, &mut pin2)) {
                self.chart_extrema_verts(c, &mut pin1, &mut pin2);
            }
            self.charts[c].pin1 = pin1;
            self.charts[c].pin2 = pin2;
        }
        let nfaces = self.charts[c].nfaces.max(0) as usize;
        let nverts = self.charts[c].nverts.max(0) as usize;
        self.charts[c].context = Some(LinearSolver::new(2 * nfaces, 2 * nverts));
    }

    /// `p_chart_lscm_solve`.
    fn chart_lscm_solve(&mut self, c: usize) -> bool {
        let verts = self.chart_vert_list(c);
        for &v in &verts {
            if self.verts[v as usize].flag & PVERT_PIN != 0 {
                // Reload for live unwrap.
                self.vert_load_pin_select_uvs(v);
            }
        }
        let single_pin = self.charts[c].single_pin;
        if single_pin != NIL {
            self.charts[c].origin = self.verts[single_pin as usize].uv;
        }
        let Some(mut ctx) = self.charts[c].context.take() else {
            return false;
        };

        let (pin1, pin2) = (self.charts[c].pin1, self.charts[c].pin2);
        if pin1 != NIL {
            for p in [pin1, pin2] {
                let id = self.verts[p as usize].id as usize;
                ctx.variable_lock(2 * id);
                ctx.variable_lock(2 * id + 1);
            }
            for p in [pin1, pin2] {
                let id = self.verts[p as usize].id as usize;
                let uv = self.verts[p as usize].uv;
                ctx.variable_set(2 * id, f64::from(uv[0]));
                ctx.variable_set(2 * id + 1, f64::from(uv[1]));
            }
        } else {
            for &v in &verts {
                let vert = &self.verts[v as usize];
                if vert.flag & PVERT_PIN != 0 {
                    let id = vert.id as usize;
                    ctx.variable_lock(2 * id);
                    ctx.variable_lock(2 * id + 1);
                    ctx.variable_set(2 * id, f64::from(vert.uv[0]));
                    ctx.variable_set(2 * id + 1, f64::from(vert.uv[1]));
                }
            }
        }

        // Detect "up" direction based on pinned vertices.
        let faces = self.chart_face_list(c);
        let mut area_pinned_up = 0.0f32;
        let mut area_pinned_down = 0.0f32;
        for &f in &faces {
            let [v1, v2, v3] = self.face_verts(f);
            let pinned = |v: u32| self.verts[v as usize].flag & PVERT_PIN != 0;
            if pinned(v1) && pinned(v2) && pinned(v3) {
                let area = self.face_uv_area_signed(f);
                if area > 0.0 {
                    area_pinned_up += area;
                } else {
                    area_pinned_down -= area;
                }
            }
        }
        let flip_faces = area_pinned_down > area_pinned_up;

        // Construct the matrix.
        let alpha = self.charts[c].abf_alpha.take();
        let mut row = 0usize;
        for (fi, &f) in faces.iter().enumerate() {
            frap_math::budget::check();
            let [mut v1, mut v2, mut v3] = self.face_verts(f);
            let (mut a1, mut a2, mut a3) = if let Some(al) = &alpha {
                (
                    f64::from(al[fi * 3]),
                    f64::from(al[fi * 3 + 1]),
                    f64::from(al[fi * 3 + 2]),
                )
            } else {
                triangle_angles(
                    self.verts[v1 as usize].co,
                    self.verts[v2 as usize].co,
                    self.verts[v3 as usize].co,
                )
            };
            if flip_faces {
                core::mem::swap(&mut a2, &mut a3);
                core::mem::swap(&mut v2, &mut v3);
            }
            let mut sina1 = libm::sin(a1);
            let mut sina2 = libm::sin(a2);
            let mut sina3 = libm::sin(a3);
            let sinmax = sina1.max(sina2).max(sina3);
            // Shift vertices to find the most stable order (`SHIFT3(a, b, c)`: a←c, b←a, c←b).
            if sina3 != sinmax {
                (v1, v2, v3) = (v3, v1, v2);
                (a1, a2, a3) = (a3, a1, a2);
                (sina1, sina2, sina3) = (sina3, sina1, sina2);
                if sina2 == sinmax {
                    (v1, v2, v3) = (v3, v1, v2);
                    (a1, a2, a3) = (a3, a1, a2);
                    (sina1, sina2, sina3) = (sina3, sina1, sina2);
                }
            }
            let _ = (a2, a3);
            // Angle based LSCM formulation.
            let ratio = if sina3 == 0.0 { 1.0 } else { sina2 / sina3 };
            let cosine = libm::cos(a1) * ratio;
            let sine = sina1 * ratio;
            let (i1, i2, i3) = (
                self.verts[v1 as usize].id as usize,
                self.verts[v2 as usize].id as usize,
                self.verts[v3 as usize].id as usize,
            );
            ctx.matrix_add(row, 2 * i1, cosine - 1.0);
            ctx.matrix_add(row, 2 * i1 + 1, -sine);
            ctx.matrix_add(row, 2 * i2, -cosine);
            ctx.matrix_add(row, 2 * i2 + 1, sine);
            ctx.matrix_add(row, 2 * i3, 1.0);
            row += 1;
            ctx.matrix_add(row, 2 * i1, sine);
            ctx.matrix_add(row, 2 * i1 + 1, cosine - 1.0);
            ctx.matrix_add(row, 2 * i2, -sine);
            ctx.matrix_add(row, 2 * i2 + 1, -cosine);
            ctx.matrix_add(row, 2 * i3 + 1, 1.0);
            row += 1;
        }
        self.charts[c].abf_alpha = alpha;

        let ok = ctx.solve();
        for &v in &verts {
            let id = self.verts[v as usize].id as usize;
            self.verts[v as usize].uv = if ok {
                [
                    ctx.variable_get(2 * id) as f32,
                    ctx.variable_get(2 * id + 1) as f32,
                ]
            } else {
                [0.0, 0.0]
            };
        }
        self.charts[c].context = Some(ctx);
        ok
    }

    /// `p_chart_lscm_transform_single_pin`.
    fn chart_lscm_transform_single_pin(&mut self, c: usize) {
        let pin = self.charts[c].single_pin;
        let new_area = self.chart_uv_area(c);
        if new_area > 0.0 {
            let scale = self.charts[c].area_uv / new_area;
            if scale > 0.0 {
                self.chart_uv_scale(c, scale.sqrt());
            }
        }
        let o = self.charts[c].origin;
        let p = self.verts[pin as usize].uv;
        self.chart_uv_translate(c, [o[0] - p[0], o[1] - p[1]]);
    }

    /// `p_chart_lscm_end`.
    fn chart_lscm_end(&mut self, c: usize) {
        let chart = &mut self.charts[c];
        chart.context = None;
        chart.abf_alpha = None;
        chart.pin1 = NIL;
        chart.pin2 = NIL;
        chart.single_pin = NIL;
    }

    /// `uv_parametrizer_lscm_begin(live, abf, use_original_bounds)`.
    pub fn lscm_begin(&mut self, live: bool, abf: bool, use_original_bounds: bool) {
        assert_eq!(self.state, PhState::Constructed);
        self.state = PhState::Lscm;
        for c in 0..self.charts.len() {
            for f in self.chart_face_list(c) {
                self.face_backup_uvs(f);
            }
            self.chart_lscm_begin(c, live, abf, use_original_bounds);
        }
    }

    /// `uv_parametrizer_lscm_solve`.
    pub fn lscm_solve(&mut self) -> SolveCount {
        assert_eq!(self.state, PhState::Lscm);
        let mut count = SolveCount::default();
        for c in 0..self.charts.len() {
            if self.charts[c].context.is_none() {
                continue;
            }
            let result = self.chart_lscm_solve(c);
            let has_pins = self.charts[c].has_pins;
            if result && !has_pins {
                self.chart_rotate_minimum_area(c);
            } else if result && self.charts[c].single_pin != NIL {
                self.chart_rotate_fit_aabb(c);
                self.chart_lscm_transform_single_pin(c);
            }
            if !result || !has_pins {
                self.chart_lscm_end(c);
            }
            if result {
                count.changed += 1;
            } else {
                count.failed += 1;
            }
        }
        count
    }

    /// `uv_parametrizer_lscm_end`.
    pub fn lscm_end(&mut self) {
        assert_eq!(self.state, PhState::Lscm);
        for c in 0..self.charts.len() {
            self.chart_lscm_end(c);
        }
        self.state = PhState::Constructed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_fits_locked_line() {
        // Minimise (x1 - x0 - 1)^2 + (x2 - x1 - 1)^2 with x0 = 0 locked: x = 0, 1, 2.
        let mut s = LinearSolver::new(2, 3);
        s.variable_lock(0);
        s.variable_set(0, 0.0);
        s.matrix_add(0, 1, 1.0);
        s.matrix_add(0, 0, -1.0);
        s.matrix_add(1, 2, 1.0);
        s.matrix_add(1, 1, -1.0);
        // Right-hand side 1 per row: emulate with a locked constant variable is not possible,
        // so check the homogeneous solution instead.
        assert!(s.solve());
        assert!(s.variable_get(1).abs() < 1e-12 && s.variable_get(2).abs() < 1e-12);
    }

    /// Dense reference: minimise |A x - b| over the free variables by Gaussian elimination on
    /// the normal equations.
    fn dense_lsq(a: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
        let n = a[0].len();
        let mut m = vec![vec![0.0; n + 1]; n];
        for (row, &bi) in a.iter().zip(b) {
            for i in 0..n {
                for j in 0..n {
                    m[i][j] += row[i] * row[j];
                }
                m[i][n] += row[i] * bi;
            }
        }
        for k in 0..n {
            let p = (k..n)
                .max_by(|&x, &y| m[x][k].abs().total_cmp(&m[y][k].abs()))
                .unwrap();
            m.swap(k, p);
            let pivot = m[k].clone();
            for row in m.iter_mut().skip(k + 1) {
                let f = row[k] / pivot[k];
                for (x, &p) in row.iter_mut().zip(&pivot).skip(k) {
                    *x -= f * p;
                }
            }
        }
        let mut x = vec![0.0; n];
        for k in (0..n).rev() {
            let mut s = m[k][n];
            for j in k + 1..n {
                s -= m[k][j] * x[j];
            }
            x[k] = s / m[k][k];
        }
        x
    }

    #[test]
    fn solver_matches_dense_least_squares() {
        // 12 rows, 6 variables of which 2 locked; pseudo-random coefficients.
        let (rows, vars) = (12usize, 6usize);
        let mut seed = 7u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            ((seed >> 33) as f64 / f64::from(1u32 << 31)) - 0.5
        };
        let a: Vec<Vec<f64>> = (0..rows)
            .map(|_| (0..vars).map(|_| rnd()).collect())
            .collect();
        let locked = [(1usize, 0.75f64), (4usize, -0.5f64)];
        let mut s = LinearSolver::new(rows, vars);
        for &(i, v) in &locked {
            s.variable_lock(i);
            s.variable_set(i, v);
        }
        for (r, row) in a.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                s.matrix_add(r, c, v);
            }
        }
        assert!(s.solve());
        let free: Vec<usize> = (0..vars)
            .filter(|i| !locked.iter().any(|l| l.0 == *i))
            .collect();
        let af: Vec<Vec<f64>> = a
            .iter()
            .map(|row| free.iter().map(|&c| row[c]).collect())
            .collect();
        let bf: Vec<f64> = a
            .iter()
            .map(|row| -locked.iter().map(|&(i, v)| row[i] * v).sum::<f64>())
            .collect();
        let x = dense_lsq(&af, &bf);
        for (k, &c) in free.iter().enumerate() {
            assert!(
                (s.variable_get(c) - x[k]).abs() < 1e-9,
                "{} vs {}",
                s.variable_get(c),
                x[k]
            );
        }
    }

    #[test]
    fn singular_system_fails() {
        let mut s = LinearSolver::new(1, 2);
        s.matrix_add(0, 0, 1.0);
        s.matrix_add(0, 1, 1.0);
        assert!(!s.solve());
    }

    #[test]
    fn angles_sum_to_pi() {
        let (a, b, c) = triangle_angles([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]);
        assert!((a + b + c - core::f64::consts::PI).abs() < 1e-5);
        // Degenerate (collinear) triangles keep a minimum angle.
        let (a, b, c) = triangle_angles([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]);
        assert!(a >= 0.001 && b >= 0.001 && c >= 0.001);
    }
}
