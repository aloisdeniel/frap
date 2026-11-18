// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.grid_fill` (Blender 5.2.4 `bmo_grid_fill_exec`, `operators/bmo_fill_grid.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.grid_fill(bmesh, edges=[], mat_nr=0, use_smooth=False, use_interp_simple=False)`.
//!
//! **Create/kill order**: the two input chains (`BM_mesh_edgeloops_find`) and the two rails
//! joining their ends over wire/boundary edges (`BM_mesh_edgeloops_find_path`, which re-points
//! `v->e` of the visited vertices like Blender) form the grid border; a shorter side is first
//! padded by splitting its edges (`BM_edge_split` at factor 0). Interior vertices are created
//! row by row (`y` = 1.., `x` = 1..), placed by the barycentric transform of the border rows
//! (or bilinear weights with `use_interp_simple`); then the quads column by column (`x`
//! outer, `y` inner, `BM_face_create_verts` with edge creation), wound against the attached
//! boundary faces. The padding edges are collapsed afterwards (`BM_edge_collapse`).
//!
//! Errors (`BMO_ERROR_CANCEL`): `Select two edge loops or a single closed edge loop from which
//! two edge loops can be calculated`, `Closed loops unsupported`, `Loops are not connected by
//! wire/boundary edges`, `Connecting edge loops overlap`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use crate::ops::bridge_loops::edgeloop::{self, EdgeLoopStore};
use crate::ops::contextual_create::bli::{self, V3};
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `grid_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GridFillIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `mat_nr`.
    pub mat_nr: i32,
    /// `use_smooth`.
    pub use_smooth: bool,
    /// `use_interp_simple`.
    pub use_interp_simple: bool,
}

/// Output slots of `grid_fill`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct GridFillOut {
    /// `faces.out` as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the grid faces (mesh order).
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_grid_fill_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const EDGE_MARK: u16 = 4;
const FACE_OUT: u16 = 16;

/// Runs `grid_fill` on `bm`. A `BMO_ERROR_CANCEL` is returned as [`BmError::Cancel`] (nothing
/// was changed).
pub fn grid_fill(bm: &mut BMesh, i: &GridFillIn) -> Result<GridFillOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("grid_fill: dead edge {dead:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

/// `quad_edges_to_normal`.
fn quad_edges_to_normal(co_a1: V3, co_a2: V3, co_b1: V3, co_b2: V3) -> V3 {
    let diff_a = bli::normalize_v3(bli::sub(co_a2, co_a1)).0;
    let diff_b = bli::normalize_v3(bli::sub(co_b2, co_b1)).0;
    bli::normalize_v3(bli::add(diff_a, diff_b)).0
}

/// `quad_verts_to_barycentric_tri`.
#[allow(clippy::too_many_arguments)]
fn quad_verts_to_barycentric_tri(
    co_a: V3,
    co_b: V3,
    co_a_next: V3,
    co_b_next: V3,
    prev: Option<(V3, V3)>,
    is_flip: bool,
) -> [V3; 3] {
    let mut no = quad_edges_to_normal(co_a, co_a_next, co_b, co_b_next);
    if let Some((co_a_prev, co_b_prev)) = prev {
        let no_t = quad_edges_to_normal(co_a_prev, co_a, co_b_prev, co_b);
        no = bli::add(no, no_t);
        no = bli::normalize_v3(no).0;
    }
    if is_flip {
        no = bli::negate(no);
    }
    no = bli::scale(no, bli::len_v3v3(co_a, co_b));
    let t2 = bli::add(bli::mid_v3_v3v3(co_a, co_b), no);
    [co_a, co_b, t2]
}

/// `barycentric_weights_v2`.
fn barycentric_weights_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2], co: [f32; 2]) -> [f32; 3] {
    let cross_tri = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (a[0] - b[0]) * (b[1] - c[1]) + (a[1] - b[1]) * (c[0] - b[0])
    };
    let mut w = [cross_tri(v2, v3, co), cross_tri(v3, v1, co), cross_tri(v1, v2, co)];
    let wtot = w[0] + w[1] + w[2];
    let f = 1.0 / wtot;
    w = [w[0] * f, w[1] * f, w[2] * f];
    if w.iter().all(|x| x.is_finite()) {
        w
    } else {
        [1.0 / 3.0; 3]
    }
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (transform_point_by_tri_v3).
fn transform_point_by_tri_v3(pt_src: V3, tar: &[V3; 3], src: &[V3; 3]) -> V3 {
    let no_tar = bli::normal_tri_v3(tar[0], tar[1], tar[2]).0;
    let no_src = bli::normal_tri_v3(src[0], src[1], src[2]).0;
    let m = bli::axis_dominant_v3_to_m3(no_src);
    let pt_xy = bli::mul_v3_m3v3(&m, pt_src);
    let t0 = bli::mul_v3_m3v3(&m, src[0]);
    let t1 = bli::mul_v3_m3v3(&m, src[1]);
    let t2 = bli::mul_v3_m3v3(&m, src[2]);
    let w = barycentric_weights_v2([t0[0], t0[1]], [t1[0], t1[1]], [t2[0], t2[1]], [pt_xy[0], pt_xy[1]]);
    let mut p = [0.0f32; 3];
    for k in 0..3 {
        p[k] = tar[0][k] * w[0] + tar[1][k] * w[1] + tar[2][k] * w[2];
    }
    let area_tar = bli::area_tri_v3(tar[0], tar[1], tar[2]).sqrt();
    let area_src = bli::area_tri_v2([t0[0], t0[1]], [t1[0], t1[1]], [t2[0], t2[1]]).sqrt();
    let z_ofs_src = pt_xy[2] - t0[2];
    bli::madd_v3_v3v3fl(p, no_tar, (z_ofs_src / area_src) * area_tar)
}

// Ported from blender/source/blender/blenlib/intern/math_geom.cc (barycentric_weights_v2_quad).
fn barycentric_weights_v2_quad(v: [[f32; 2]; 4], co: [f32; 2]) -> [f32; 4] {
    let dirs: Vec<[f32; 2]> = v.iter().map(|p| [p[0] - co[0], p[1] - co[1]]).collect();
    let lens: Vec<f32> = dirs.iter().map(|d| (d[0] * d[0] + d[1] * d[1]).sqrt()).collect();
    for k in 0..4 {
        if lens[k] < f32::EPSILON {
            let mut w = [0.0; 4];
            w[k] = 1.0;
            return w;
        }
    }
    let cross = |a: [f32; 2], b: [f32; 2]| a[0] * b[1] - a[1] * b[0];
    let dotp = |a: [f32; 2], b: [f32; 2]| a[0] * b[0] + a[1] * b[1];
    let areas: Vec<f32> = (0..4).map(|i| cross(dirs[i], dirs[(i + 1) & 3])).collect();
    let dots: Vec<f32> = (0..4).map(|i| dotp(dirs[i], dirs[(i + 1) & 3])).collect();
    let lens_prod: Vec<f32> = (0..4).map(|i| lens[i] * lens[(i + 1) & 3]).collect();
    for i in 0..4 {
        if areas[i].abs() < 1.0e-12 * lens_prod[i] && dots[i] <= 0.0 {
            let j = (i + 1) & 3;
            let sum = lens[i] + lens[j];
            let mut w = [0.0; 4];
            w[i] = lens[j] / sum;
            w[j] = lens[i] / sum;
            return w;
        }
    }
    let t: Vec<f32> = (0..4)
        .map(|i| {
            if areas[i] != 0.0 {
                ((lens_prod[i] - dots[i]) / areas[i]).abs()
            } else {
                0.0
            }
        })
        .collect();
    let mut w = [
        (t[3] + t[0]) / lens[0],
        (t[0] + t[1]) / lens[1],
        (t[1] + t[2]) / lens[2],
        (t[2] + t[3]) / lens[3],
    ];
    let wtot = w[0] + w[1] + w[2] + w[3];
    let f = 1.0 / wtot;
    for x in &mut w {
        *x *= f;
    }
    if w.iter().all(|x| x.is_finite()) { w } else { [0.25; 4] }
}

/// `barycentric_weights_v2_grid_cache`.
fn weights_grid_cache(xtot: usize, ytot: usize) -> Vec<[f32; 4]> {
    let x_step = 1.0 / (xtot - 1) as f32;
    let y_step = 1.0 / (ytot - 1) as f32;
    let mut out = Vec::with_capacity(xtot * ytot);
    for y in 0..ytot {
        let fy = y_step * y as f32;
        for x in 0..xtot {
            let fx = x_step * x as f32;
            let cos = [[fx, 0.0], [0.0, fy], [fx, 1.0], [1.0, fy]];
            out.push(barycentric_weights_v2_quad(cos, [fx, fy]));
        }
    }
    out
}

/// `bm_loop_pair_from_verts`.
fn loop_pair_from_verts(bm: &BMesh, v_a: VertId, v_b: VertId) -> [Option<LoopId>; 2] {
    let e = bm.edge_exists(v_a, v_b).expect("grid border edge");
    match bm.edge(e).l {
        Some(l) => {
            let ll = bm.lp(l);
            if ll.v == v_a { [Some(l), Some(ll.next)] } else { [Some(ll.next), Some(l)] }
        }
        None => [None, None],
    }
}

/// `bm_loop_pair_test_copy`.
fn loop_pair_test_copy(a: &mut [Option<LoopId>; 2], b: &mut [Option<LoopId>; 2]) {
    if a[0].is_some() && b[0].is_none() {
        *b = [a[1], a[0]];
    } else if b[0].is_some() && a[0].is_none() {
        *a = [b[1], b[0]];
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_fill_grid.cc (bm_grid_fill_array).
#[allow(clippy::too_many_arguments)]
fn grid_fill_array(
    bm: &mut BMesh,
    v_grid: &mut [Option<VertId>],
    xtot: usize,
    ytot: usize,
    mat_nr: i16,
    use_smooth: bool,
    use_flip: bool,
    use_interp_simple: bool,
) {
    let use_vert_interp = !bm.vdata.layers.is_empty();
    let use_loop_interp = !bm.ldata.layers.is_empty();
    let xy = |x: usize, y: usize| x + y * xtot;
    let g = |vg: &[Option<VertId>], i: usize| vg[i].expect("grid border vertex");
    let co = |bm: &BMesh, vg: &[Option<VertId>], i: usize| bm.vert(g(vg, i)).co;
    let tri_a = quad_verts_to_barycentric_tri(
        co(bm, v_grid, xy(0, 0)),
        co(bm, v_grid, xy(xtot - 1, 0)),
        co(bm, v_grid, xy(0, 1)),
        co(bm, v_grid, xy(xtot - 1, 1)),
        None,
        false,
    );
    let tri_b = quad_verts_to_barycentric_tri(
        co(bm, v_grid, xy(0, ytot - 1)),
        co(bm, v_grid, xy(xtot - 1, ytot - 1)),
        co(bm, v_grid, xy(0, ytot - 2)),
        co(bm, v_grid, xy(xtot - 1, ytot - 2)),
        None,
        true,
    );
    let weights = if use_interp_simple || use_vert_interp || use_loop_interp {
        weights_grid_cache(xtot, ytot)
    } else {
        Vec::new()
    };
    let (mut lx_a, mut lx_b, mut ly_a, mut ly_b) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    if use_loop_interp {
        for x in 0..xtot - 1 {
            let mut a = loop_pair_from_verts(bm, g(v_grid, xy(x, 0)), g(v_grid, xy(x + 1, 0)));
            let mut b = loop_pair_from_verts(
                bm,
                g(v_grid, xy(x, ytot - 1)),
                g(v_grid, xy(x + 1, ytot - 1)),
            );
            loop_pair_test_copy(&mut a, &mut b);
            lx_a.push(a);
            lx_b.push(b);
        }
        for y in 0..ytot - 1 {
            let mut a = loop_pair_from_verts(bm, g(v_grid, xy(0, y)), g(v_grid, xy(0, y + 1)));
            let mut b = loop_pair_from_verts(
                bm,
                g(v_grid, xy(xtot - 1, y)),
                g(v_grid, xy(xtot - 1, y + 1)),
            );
            loop_pair_test_copy(&mut a, &mut b);
            ly_a.push(a);
            ly_b.push(b);
        }
    }
    // Build verts.
    for y in 1..ytot - 1 {
        let tri_t = quad_verts_to_barycentric_tri(
            co(bm, v_grid, xy(0, y)),
            co(bm, v_grid, xy(xtot - 1, y)),
            co(bm, v_grid, xy(0, y + 1)),
            co(bm, v_grid, xy(xtot - 1, y + 1)),
            Some((co(bm, v_grid, xy(0, y - 1)), co(bm, v_grid, xy(xtot - 1, y - 1)))),
            false,
        );
        for x in 1..xtot - 1 {
            let c = if !use_interp_simple {
                let co_a = transform_point_by_tri_v3(co(bm, v_grid, x), &tri_t, &tri_a);
                let co_b = transform_point_by_tri_v3(
                    co(bm, v_grid, xtot * ytot + x - xtot),
                    &tri_t,
                    &tri_b,
                );
                bli::interp_v3_v3v3(co_a, co_b, y as f32 / (ytot as f32 - 1.0))
            } else {
                let w = weights[xy(x, y)];
                let mut c = [0.0f32; 3];
                bli::madd_v3_v3fl(&mut c, co(bm, v_grid, xy(x, 0)), w[0]);
                bli::madd_v3_v3fl(&mut c, co(bm, v_grid, xy(0, y)), w[1]);
                bli::madd_v3_v3fl(&mut c, co(bm, v_grid, xy(x, ytot - 1)), w[2]);
                bli::madd_v3_v3fl(&mut c, co(bm, v_grid, xy(xtot - 1, y)), w[3]);
                c
            };
            let v = bm.vert_create(c, None, CreateFlag::empty());
            v_grid[xy(x, y)] = Some(v);
            if use_vert_interp {
                let w = weights[xy(x, y)];
                let src = [
                    g(v_grid, xy(x, 0)),
                    g(v_grid, xy(0, y)),
                    g(v_grid, xy(x, ytot - 1)),
                    g(v_grid, xy(xtot - 1, y)),
                ];
                bm.vert_data_interp(&src, &w, v);
            }
        }
    }
    // Build faces.
    for x in 0..xtot - 1 {
        for y in 0..ytot - 1 {
            let bl = g(v_grid, xy(x, y));
            let tl = g(v_grid, xy(x, y + 1));
            let tr = g(v_grid, xy(x + 1, y + 1));
            let br = g(v_grid, xy(x + 1, y));
            let quad = if use_flip { [bl, tl, tr, br] } else { [br, tr, tl, bl] };
            let f = bm
                .face_create_verts(&quad, None, CreateFlag::empty(), true)
                .expect("grid quad");
            if use_loop_interp && (lx_a[x][0].is_some() || ly_a[y][0].is_some()) {
                let (interp_from, l_tmp) = if lx_a[x][0].is_some() && ly_a[y][0].is_some() {
                    ('B', lx_a[x][0])
                } else if lx_a[x][0].is_some() {
                    ('X', lx_a[x][0])
                } else {
                    ('Y', ly_a[y][0])
                };
                let src_f = bm.lp(l_tmp.expect("loop")).f;
                bm.face_attrs_copy(src_f, f);
                let ls = bm.face_loops(f);
                let l_quad = if use_flip {
                    [ls[0], ls[1], ls[3], ls[2]]
                } else {
                    [ls[3], ls[2], ls[0], ls[1]]
                };
                let mut i = 0;
                for x_side in 0..2 {
                    for y_side in 0..2 {
                        let lq = l_quad[i];
                        i += 1;
                        match interp_from {
                            'B' => {
                                let w = weights[xy(x + x_side, y + y_side)];
                                let b = [
                                    lx_a[x][x_side],
                                    ly_a[y][y_side],
                                    lx_b[x][x_side],
                                    ly_b[y][y_side],
                                ];
                                let blocks: Vec<u32> =
                                    b.iter().map(|l| l.expect("boundary loop").0).collect();
                                bm.ldata.interp(&blocks, &w, lq.0);
                            }
                            'X' => {
                                let t = (y + y_side) as f32 / (ytot - 1) as f32;
                                let b = [lx_a[x][x_side], lx_b[x][x_side]];
                                let blocks: Vec<u32> =
                                    b.iter().map(|l| l.expect("boundary loop").0).collect();
                                bm.ldata.interp(&blocks, &[1.0 - t, t], lq.0);
                            }
                            _ => {
                                let t = (x + x_side) as f32 / (xtot - 1) as f32;
                                let b = [ly_a[y][y_side], ly_b[y][y_side]];
                                let blocks: Vec<u32> =
                                    b.iter().map(|l| l.expect("boundary loop").0).collect();
                                bm.ldata.interp(&blocks, &[1.0 - t, t], lq.0);
                            }
                        }
                    }
                }
            }
            bm.oflag_enable(f, FACE_OUT);
            bm.face_mut(f).mat_nr = mat_nr;
            if use_smooth {
                bm.face_mut(f).flag.insert(ElemFlag::SMOOTH);
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/operators/bmo_fill_grid.cc (bm_grid_fill).
#[allow(clippy::too_many_arguments)]
fn grid_fill_loops(
    bm: &mut BMesh,
    a: &EdgeLoopStore,
    b: &EdgeLoopStore,
    rail_a: &EdgeLoopStore,
    rail_b: &EdgeLoopStore,
    mat_nr: i16,
    use_smooth: bool,
    use_interp_simple: bool,
) {
    let xtot = a.len();
    let ytot = rail_a.len();
    let mut v_grid: Vec<Option<VertId>> = vec![None; xtot * ytot];
    for (i, &v) in a.verts.iter().enumerate() {
        v_grid[i] = Some(v);
    }
    for (i, &v) in b.verts.iter().enumerate() {
        v_grid[ytot * xtot + i - xtot] = Some(v);
    }
    for (i, &v) in rail_a.verts.iter().enumerate() {
        v_grid[xtot * i] = Some(v);
    }
    for (i, &v) in rail_b.verts.iter().enumerate() {
        v_grid[xtot * i + xtot - 1] = Some(v);
    }
    // USE_FLIP_DETECT.
    let lists: [(&EdgeLoopStore, i32); 4] = [(a, -1), (b, 1), (rail_a, 1), (rail_b, -1)];
    let mut votes = 0i32;
    for (el, dir) in lists {
        for w in el.verts.windows(2) {
            let e = bm.edge_exists(w[0], w[1]).expect("grid border edge");
            if bm.edge_is_boundary(e) {
                let l = bm.edge(e).l.expect("boundary");
                votes += if bm.lp(l).v == w[0] { dir } else { -dir };
            }
        }
    }
    let use_flip = votes < 0;
    grid_fill_array(
        bm,
        &mut v_grid,
        xtot,
        ytot,
        mat_nr,
        use_smooth,
        use_flip,
        use_interp_simple,
    );
}

/// `bm_edgeloop_flag_set(HIDDEN)` over the edges of an open chain.
fn edgeloop_hide_set(bm: &mut BMesh, el: &EdgeLoopStore, set: bool) {
    for w in el.verts.windows(2) {
        if let Some(e) = bm.edge_exists(w[1], w[0]) {
            bm.edge_mut(e).flag.set(ElemFlag::HIDDEN, set);
        }
    }
}

/// `bm_edge_test_rail_cb`.
fn rail_test(bm: &BMesh, e: EdgeId) -> bool {
    !bm.edge(e).flag.contains(ElemFlag::HIDDEN) && (bm.edge_is_wire(e) || bm.edge_is_boundary(e))
}

// Ported from blender/source/blender/bmesh/operators/bmo_fill_grid.cc (bmo_grid_fill_exec).
fn exec(bm: &mut BMesh, i: &GridFillIn) -> Result<GridFillOut, BmError> {
    let mat_nr = i.mat_nr as i16;
    for &e in &i.edges {
        bm.oflag_enable(e, EDGE_MARK);
    }
    let eloops = edgeloop::mesh_edgeloops_find(bm, &|bm, e| bm.oflag_test(e, EDGE_MARK));
    if eloops.len() != 2 {
        return Err(BmError::Cancel(
            "Select two edge loops or a single closed edge loop from which two edge loops can be \
             calculated"
                .into(),
        ));
    }
    let mut a = eloops[0].clone();
    let mut b = eloops[1].clone();
    let v_a_first = a.verts[0];
    let v_a_last = *a.verts.last().expect("loop");
    let v_b_first = b.verts[0];
    let v_b_last = *b.verts.last().expect("loop");
    if a.closed || b.closed {
        return Err(BmError::Cancel("Closed loops unsupported".into()));
    }
    edgeloop_hide_set(bm, &a, true);
    edgeloop_hide_set(bm, &b, true);
    let mut rails: Option<(EdgeLoopStore, EdgeLoopStore)> = None;
    if let Some(ra) = edgeloop::mesh_edgeloops_find_path(bm, Some(&rail_test), v_a_first, v_b_first)
        && let Some(rb) = edgeloop::mesh_edgeloops_find_path(bm, Some(&rail_test), v_a_last, v_b_last)
    {
        rails = Some((ra, rb));
    }
    if rails.is_none()
        && let Some(ra) =
            edgeloop::mesh_edgeloops_find_path(bm, Some(&rail_test), v_a_first, v_b_last)
        && let Some(rb) =
            edgeloop::mesh_edgeloops_find_path(bm, Some(&rail_test), v_a_last, v_b_first)
    {
        rails = Some((ra, rb));
        b.flip();
    }
    edgeloop_hide_set(bm, &a, false);
    edgeloop_hide_set(bm, &b, false);
    let Some((mut rail_a, mut rail_b)) = rails else {
        return Err(BmError::Cancel(
            "Loops are not connected by wire/boundary edges".into(),
        ));
    };
    if edgeloop::edgeloop_overlap_check(&rail_a, &rail_b) {
        return Err(BmError::Cancel("Connecting edge loops overlap".into()));
    }
    let mut split_edges: Vec<EdgeId> = Vec::new();
    for pair in 0..2 {
        let (x, y) = if pair == 0 {
            (&mut a, &mut b)
        } else {
            (&mut rail_a, &mut rail_b)
        };
        let (la, lb) = (x.len(), y.len());
        if la < lb {
            x.expand_split(bm, lb, &mut split_edges);
        } else if lb < la {
            y.expand_split(bm, la, &mut split_edges);
        }
    }
    grid_fill_loops(
        bm,
        &a,
        &b,
        &rail_a,
        &rail_b,
        mat_nr,
        i.use_smooth,
        i.use_interp_simple,
    );
    for e in split_edges {
        if bm.edges.contains(e.0) {
            let v2 = bm.edge(e).v[1];
            bm.join_vert_kill_edge(e, v2, true, true, true);
        }
    }
    let faces: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.oflag_test(f, FACE_OUT))
        .collect();
    Ok(GridFillOut {
        geom: faces.iter().map(|&f| BMElem::Face(f)).collect(),
        faces,
    })
}
