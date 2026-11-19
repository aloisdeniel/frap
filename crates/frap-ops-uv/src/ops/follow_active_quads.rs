// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.follow_active_quads(mode='LENGTH_AVERAGE')`, tier T1
//! (`tests/golden/cases/uv/follow/*.py`). Port of the Python operator
//! `scripts/startup/bl_operators/uvcalc_follow_active.py` (`extend`, `main`), Blender 5.2.4,
//! GPL-2.0-or-later. Its float arithmetic is kept: lengths and factors are Python floats
//! (`f64`) of single-precision lengths, UV arithmetic is `mathutils.Vector` (`f32`).
//!
//! Poll `context.mode == 'EDIT_MESH'`. On every mesh in Edit Mode (unique data), the UVs of
//! the active face (a selected quad: `bm.faces.active`) are extended to the selected quads
//! connected to it across manifold, non-seam edges, breadth first, each quad continuing its
//! neighbour's strip. The spacing follows `mode`: `EVEN` (as the neighbour), `LENGTH` (the
//! ratio of the edge lengths), `LENGTH_AVERAGE` (the ratio of the average edge lengths of the
//! two quad loops). Called from a 3D View (frap's virtual one, D-10), the UV selection is
//! ignored. When every mesh fails, the operator raises the first applicable error:
//! `Active face must be a quad`, `Active face not selected`, `No selected faces`, `No UV
//! layers`, `No active face` (a mesh without a UV map is not given one).

use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::follow_active_quads::Mode;

use super::unwrap::uvedit::{self, Uv, UvOb};

const STATUS_OK: u32 = 1 << 0;
const STATUS_ERR_ACTIVE_FACE: u32 = 1 << 1;
const STATUS_ERR_NOT_SELECTED: u32 = 1 << 2;
const STATUS_ERR_NOT_QUAD: u32 = 1 << 3;
const STATUS_ERR_MISSING_UV_LAYER: u32 = 1 << 4;
const STATUS_ERR_NO_FACES_SELECTED: u32 = 1 << 5;

/// `BMEdge.calc_length()` as a Python float.
fn edge_len(bm: &BMesh, e: EdgeId) -> f64 {
    let [a, b] = bm.edge(e).v;
    f64::from(frap_uv::param::bli::len_v3v3(bm.vert(a).co, bm.vert(b).co))
}

/// `(a - b).length` of two vertex positions as a Python float.
fn dist(a: [f32; 3], b: [f32; 3]) -> f64 {
    f64::from(frap_uv::param::bli::len_v3v3(a, b))
}

/// `walk_edgeloop_all`: every edge of the quad loops (rings) through `e`, in the generator's
/// order.
fn walk_edgeloop_all(bm: &BMesh, e: EdgeId) -> Vec<EdgeId> {
    let mut out = vec![e];
    let mut visited = vec![false; bm.edges.capacity()];
    visited[e.slot()] = true;
    let mut stack: Vec<LoopId> = Vec::new();
    let quad = |l: LoopId| bm.face(bm.lp(l).f).len == 4;
    let mut step = |l: LoopId, stack: &mut Vec<LoopId>, out: &mut Vec<EdgeId>| {
        let l_other = bm.lp(bm.lp(l).next).next;
        let oe = bm.lp(l_other).e;
        if !visited[oe.slot()] {
            visited[oe.slot()] = true;
            out.push(oe);
            if !bm.edge_is_boundary(oe) {
                stack.push(l_other);
            }
        }
    };
    for l in bm.edge_loops(e) {
        if !quad(l) {
            continue;
        }
        step(l, &mut stack, &mut out);
    }
    while let Some(l_test) = stack.pop() {
        frap_math::budget::check();
        let mut l = bm.lp(l_test).radial_next;
        while l != l_test {
            if quad(l) {
                step(l, &mut stack, &mut out);
            }
            l = bm.lp(l).radial_next;
        }
    }
    out
}

/// `extrapolate_uv`.
fn extrapolate(
    fac: f64,
    uvs: &mut [Uv],
    a_outer: LoopId,
    a_inner: LoopId,
    b_outer: LoopId,
    b_inner: LoopId,
) {
    let inner = uvs[a_inner.slot()];
    let outer = uvs[a_outer.slot()];
    let f = fac as f32;
    uvs[b_inner.slot()] = inner;
    uvs[b_outer.slot()] = [
        inner[0] + (inner[0] - outer[0]) * f,
        inner[1] + (inner[1] - outer[1]) * f,
    ];
}

/// `extend`: the status of one mesh; the UVs are written into `o.uvs`.
fn extend(o: &mut UvOb, mode: Mode) -> u32 {
    let bm = o.bm.clone();
    let Some(f_act) = bm
        .act_face
        .filter(|&f| bm.elem_alive(frap_bmesh::BMElem::Face(f)))
    else {
        return STATUS_ERR_ACTIVE_FACE;
    };
    if !bm.face(f_act).flag.contains(ElemFlag::SELECT) {
        return STATUS_ERR_NOT_SELECTED;
    }
    if bm.face(f_act).len != 4 {
        return STATUS_ERR_NOT_QUAD;
    }
    // Called from a 3D View: the UV selection is ignored.
    let faces: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT) && bm.face(f).len == 4)
        .collect();
    if faces.is_empty() {
        return STATUS_ERR_NO_FACES_SELECTED;
    }

    // Average length per edge loop.
    let mut edge_lengths: Vec<Option<usize>> = vec![None; bm.edges.capacity()];
    let mut stores: Vec<f64> = Vec::new();
    if mode == Mode::LengthAverage {
        for &f in &faces {
            let l_quad = bm.face_loops(f);
            for &l_init in &l_quad[..2] {
                let e_init = bm.lp(l_init).e;
                if edge_lengths[e_init.slot()].is_some() {
                    continue;
                }
                let store = stores.len();
                stores.push(-1.0);
                let mut accum = 0.0f64;
                let mut total = 0u32;
                for e in walk_edgeloop_all(&bm, e_init) {
                    edge_lengths[e.slot()] = Some(store);
                    accum += edge_len(&bm, e);
                    total += 1;
                }
                stores[store] = accum / f64::from(total);
            }
        }
    }
    let length_of = |e: EdgeId| -> f64 { edge_lengths[e.slot()].map_or(-1.0, |s| stores[s]) };

    // `walk_face_init`.
    let mut tag = vec![true; bm.faces.capacity()];
    for &f in &faces {
        tag[f.slot()] = false;
    }
    tag[f_act.slot()] = true;

    // `walk_face(f_act)`, applying `apply_uv` to each step as the generator yields it.
    let co = |l: LoopId| bm.vert(bm.lp(l).v).co;
    let mut faces_a = vec![f_act];
    let mut faces_b = Vec::new();
    while !faces_a.is_empty() {
        for &f in &faces_a {
            for l in bm.face_loops(f) {
                frap_math::budget::check();
                let e = bm.lp(l).e;
                if !bm.edge_is_manifold(e) || bm.edge(e).flag.contains(ElemFlag::SEAM) {
                    continue;
                }
                let l_other = bm.lp(l).radial_next;
                let f_other = bm.lp(l_other).f;
                if tag[f_other.slot()] {
                    continue;
                }
                // `apply_uv(f, l, f_other)`.
                let next = |x: LoopId| bm.lp(x).next;
                let l_a = [l, next(l), next(next(l)), next(next(next(l)))];
                let l_next = l_other;
                let l_b = if bm.lp(l_next).v != bm.lp(l).v {
                    let b1 = l_next;
                    let b0 = next(b1);
                    let b3 = next(b0);
                    let b2 = next(b3);
                    [b0, b1, b2, b3]
                } else {
                    let b0 = l_next;
                    [b0, next(b0), next(next(b0)), next(next(next(b0)))]
                };
                let fac = match mode {
                    Mode::LengthAverage => {
                        let d1 = length_of(bm.lp(l_a[1]).e);
                        let d2 = length_of(bm.lp(l_b[2]).e);
                        if d1 == 0.0 { 1.0 } else { d2 / d1 }
                    }
                    Mode::Length => {
                        let (a0, b0, c0) = (co(l_a[3]), co(l_a[0]), co(l_b[3]));
                        let (a1, b1, c1) = (co(l_a[2]), co(l_a[1]), co(l_b[2]));
                        let d1 = dist(a0, b0) + dist(a1, b1);
                        let d2 = dist(b0, c0) + dist(b1, c1);
                        if d1 == 0.0 { 1.0 } else { d2 / d1 }
                    }
                    Mode::Even => 1.0,
                };
                extrapolate(fac, &mut o.uvs, l_a[3], l_a[0], l_b[3], l_b[0]);
                extrapolate(fac, &mut o.uvs, l_a[2], l_a[1], l_b[2], l_b[1]);
                tag[f_other.slot()] = true;
                faces_b.push(f_other);
            }
        }
        core::mem::swap(&mut faces_a, &mut faces_b);
        faces_b.clear();
    }
    STATUS_OK
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::FollowActiveQuads) -> OpResult {
    let objects = uvedit::edit_objects(cx);
    let mut num_errors = 0usize;
    let mut status = 0u32;
    for &(ob, me) in &objects {
        let Some(bm) = uvedit::edit_bm(cx, me) else {
            continue;
        };
        // A mesh without a UV map (`bm.loops.layers.uv.active is None`) fails after the
        // active-face checks.
        let ret = match UvOb::gather(cx, &[(ob, me)]).pop() {
            Some(mut o) => {
                let r = extend(&mut o, p.mode.value);
                if r == STATUS_OK {
                    o.commit(cx)?;
                }
                r
            }
            None => {
                let mut o = UvOb {
                    ob,
                    me,
                    bm: bm.clone(),
                    uv_name: Default::default(),
                    uvs: Vec::new(),
                    pins: Vec::new(),
                };
                match extend_checks(&mut o) {
                    0 => STATUS_ERR_MISSING_UV_LAYER,
                    r => r,
                }
            }
        };
        if ret != STATUS_OK {
            num_errors += 1;
            status |= ret;
        }
    }
    if num_errors == objects.len() {
        let msg = if status & STATUS_ERR_NOT_QUAD != 0 {
            "Active face must be a quad"
        } else if status & STATUS_ERR_NOT_SELECTED != 0 {
            "Active face not selected"
        } else if status & STATUS_ERR_NO_FACES_SELECTED != 0 {
            "No selected faces"
        } else if status & STATUS_ERR_MISSING_UV_LAYER != 0 {
            "No UV layers"
        } else {
            "No active face"
        };
        return Err(OpError::Runtime(msg.into()));
    }
    Ok(OpStatus::Finished)
}

/// The active-face checks of `extend` that come before the UV map test (0: they pass).
fn extend_checks(o: &mut UvOb) -> u32 {
    let bm = &o.bm;
    let Some(f_act) = bm
        .act_face
        .filter(|&f| bm.elem_alive(frap_bmesh::BMElem::Face(f)))
    else {
        return STATUS_ERR_ACTIVE_FACE;
    };
    if !bm.face(f_act).flag.contains(ElemFlag::SELECT) {
        return STATUS_ERR_NOT_SELECTED;
    }
    if bm.face(f_act).len != 4 {
        return STATUS_ERR_NOT_QUAD;
    }
    0
}
