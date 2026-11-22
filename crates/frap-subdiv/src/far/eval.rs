// SPDX-License-Identifier: GPL-3.0-or-later
//! Patch lookup and evaluation: Blender's `PatchMap` (a quadtree per ptex face over the patch
//! parameterisations), `Osd::CpuEvaluator::EvalPatches` for one point, and the patch bases of
//! `osd/patchBasis.h` (`OsdEvaluatePatchBasis` for `REGULAR`, `GREGORY_BASIS` and `QUADS`) in
//! `f32` with clang's fused multiply-adds.
//!
//! Ported from Blender 5.2.4 `intern/opensubdiv/internal/evaluator/patch_map.{h,cc}`
//! (Apache-2.0, originally Pixar) and OpenSubdiv 3.7.0 `opensubdiv/osd/{patchBasis.h,
//! patchBasisTypes.h,cpuEvaluator.cpp}` (Pixar, TOST / Apache-2.0 modified).

use super::patch_builder::PatchParam;
use super::patch_table::{FvarPatch, PatchType};

#[derive(Copy, Clone, Debug, Default)]
struct Child {
    is_set: bool,
    is_leaf: bool,
    index: usize,
}

#[derive(Copy, Clone, Debug, Default)]
struct QuadNode {
    children: [Child; 4],
}

/// Blender's `blender::opensubdiv::PatchMap` (quad patches).
#[derive(Clone, Debug, Default)]
pub(crate) struct PatchMap {
    min_face: i32,
    max_face: i32,
    max_depth: i32,
    tree: Vec<QuadNode>,
}

impl PatchMap {
    pub(crate) fn new(params: &[PatchParam]) -> PatchMap {
        let mut m = PatchMap {
            min_face: -1,
            max_face: -1,
            ..PatchMap::default()
        };
        if params.is_empty() {
            return m;
        }
        m.min_face = params[0].face_id;
        m.max_face = m.min_face;
        for p in params {
            m.min_face = m.min_face.min(p.face_id);
            m.max_face = m.max_face.max(p.face_id);
        }
        let n = (m.max_face - m.min_face + 1) as usize;
        m.tree = vec![QuadNode::default(); n];
        for (handle, p) in params.iter().enumerate() {
            let depth = p.depth;
            let root = p.non_quad_root as i32;
            m.max_depth = m.max_depth.max(depth);
            let mut node = (p.face_id - m.min_face) as usize;
            if depth == root {
                for c in &mut m.tree[node].children {
                    *c = Child {
                        is_set: true,
                        is_leaf: true,
                        index: handle,
                    };
                }
                continue;
            }
            for j in root + 1..=depth {
                let ub = (p.u >> (depth - j)) & 1;
                let vb = (p.v >> (depth - j)) & 1;
                let q = ((vb << 1) | ub) as usize;
                if j == depth {
                    m.tree[node].children[q] = Child {
                        is_set: true,
                        is_leaf: true,
                        index: handle,
                    };
                } else if m.tree[node].children[q].is_set {
                    node = m.tree[node].children[q].index;
                } else {
                    let new = m.tree.len();
                    m.tree.push(QuadNode::default());
                    m.tree[node].children[q] = Child {
                        is_set: true,
                        is_leaf: false,
                        index: new,
                    };
                    node = new;
                }
            }
        }
        m
    }

    /// `FindPatch(faceid, u, v)`: the patch handle (table index).
    pub(crate) fn find(&self, face: i32, u: f64, v: f64) -> Option<usize> {
        if face < self.min_face || face > self.max_face {
            return None;
        }
        let mut node = &self.tree[(face - self.min_face) as usize];
        if !node.children[0].is_set {
            return None;
        }
        let (mut u, mut v) = (u, v);
        let mut median = 0.5f64;
        for _ in 0..=self.max_depth {
            let uh = u >= median;
            if uh {
                u -= median;
            }
            let vh = v >= median;
            if vh {
                v -= median;
            }
            let q = ((vh as usize) << 1) | uh as usize;
            let c = node.children[q];
            if !c.is_set {
                return None;
            }
            if c.is_leaf {
                return Some(c.index);
            }
            node = &self.tree[c.index];
            median *= 0.5;
        }
        None
    }
}

/// `Osd_evalBSplineCurve` (positions only).
fn bspline_curve(t: f32) -> [f32; 4] {
    let one6th = 1.0f32 / 6.0f32;
    let t2 = t * t;
    let t3 = t * t2;
    [
        one6th * ((-3.0f32).mul_add(t - t2, 1.0) - t3),
        one6th * 3.0f32.mul_add(t3, (-6.0f32).mul_add(t2, 4.0)),
        one6th * 3.0f32.mul_add(t + t2 - t3, 1.0),
        one6th * t3,
    ]
}

/// `Osd_adjustBSplineBoundaryWeights`
fn adjust_boundary(boundary: i32, w: &mut [f32; 16]) {
    if boundary & 1 != 0 {
        for i in 0..4 {
            w[i + 8] -= w[i];
            w[i + 4] = w[i].mul_add(2.0, w[i + 4]);
            w[i] = 0.0;
        }
    }
    if boundary & 2 != 0 {
        for i in (0..16).step_by(4) {
            w[i + 1] -= w[i + 3];
            w[i + 2] = w[i + 3].mul_add(2.0, w[i + 2]);
            w[i + 3] = 0.0;
        }
    }
    if boundary & 4 != 0 {
        for i in 0..4 {
            w[i + 4] -= w[i + 12];
            w[i + 8] = w[i + 12].mul_add(2.0, w[i + 8]);
            w[i + 12] = 0.0;
        }
    }
    if boundary & 8 != 0 {
        for i in (0..16).step_by(4) {
            w[i + 2] -= w[i];
            w[i + 1] = w[i].mul_add(2.0, w[i + 1]);
            w[i] = 0.0;
        }
    }
}

/// `Osd_evalBezierCurve` (positions only).
fn bezier_curve(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let tc = 1.0f32 - t;
    let tc2 = tc * tc;
    [tc2 * tc, tc2 * t * 3.0, t2 * tc * 3.0, t2 * t]
}

/// `Osd_EvalBasisGregory` (positions only).
fn gregory_basis(s: f32, t: f32, w: &mut [f32; 20]) {
    const BOUNDARY_G: [usize; 12] = [0, 1, 7, 5, 2, 6, 16, 12, 15, 17, 11, 10];
    const BOUNDARY_S: [usize; 12] = [0, 1, 2, 3, 0, 3, 0, 3, 0, 1, 2, 3];
    const BOUNDARY_T: [usize; 12] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 3, 3];
    const INTERIOR_G: [usize; 8] = [3, 4, 8, 9, 13, 14, 18, 19];
    const INTERIOR_S: [usize; 8] = [1, 1, 2, 2, 2, 2, 1, 1];
    const INTERIOR_T: [usize; 8] = [1, 1, 1, 1, 2, 2, 2, 2];
    let bs = bezier_curve(s);
    let bt = bezier_curve(t);
    let sc = 1.0f32 - s;
    let tc = 1.0f32 - t;
    let inv = |d: f32| if d <= 0.0 { 1.0 } else { 1.0 / d };
    let df0 = inv(s + t);
    let df1 = inv(sc + t);
    let df2 = inv(sc + tc);
    let df3 = inv(s + tc);
    let g = [
        s * df0,
        (-s).mul_add(df0, 1.0),
        t * df1,
        (-t).mul_add(df1, 1.0),
        sc * df2,
        (-sc).mul_add(df2, 1.0),
        tc * df3,
        (-tc).mul_add(df3, 1.0),
    ];
    for i in 0..12 {
        w[BOUNDARY_G[i]] = bs[BOUNDARY_S[i]] * bt[BOUNDARY_T[i]];
    }
    for j in 0..8 {
        w[INTERIOR_G[j]] = bs[INTERIOR_S[j]] * bt[INTERIOR_T[j]] * g[j];
    }
}

/// `OsdPatchParamNormalize`
fn normalize(p: &PatchParam, s: f32, t: f32) -> (f32, f32) {
    let frac = 1.0f32 / (1i32 << (p.depth - p.non_quad_root as i32)) as f32;
    let frac_inv = 1.0f32 / frac;
    (
        s.mul_add(frac_inv, -(p.u as f32)),
        t.mul_add(frac_inv, -(p.v as f32)),
    )
}

/// `CpuEvaluator::EvalPatches` for one point of a two-component buffer.
pub(crate) fn eval_patch(
    patch: &FvarPatch,
    s: f32,
    t: f32,
    data: &[[f32; 2]],
    base: usize,
) -> [f32; 2] {
    let (s, t) = normalize(&patch.param, s, t);
    let mut out = [0.0f32; 2];
    let mut acc = |w: &[f32], pts: &[i32]| {
        for j in 0..w.len() {
            let src = data[base + pts[j] as usize];
            out[0] = src[0].mul_add(w[j], out[0]);
            out[1] = src[1].mul_add(w[j], out[1]);
        }
    };
    let kind = match patch.kind {
        PatchType::Quads => PatchType::Quads,
        _ if patch.param.regular => PatchType::Regular,
        _ => PatchType::GregoryBasis,
    };
    match kind {
        PatchType::Quads => {
            let sc = 1.0f32 - s;
            let tc = 1.0f32 - t;
            acc(&[sc * tc, s * tc, s * t, sc * t], &patch.points);
        }
        PatchType::Regular => {
            let sw = bspline_curve(s);
            let tw = bspline_curve(t);
            let mut w = [0.0f32; 16];
            for i in 0..4 {
                for j in 0..4 {
                    w[4 * i + j] = sw[j] * tw[i];
                }
            }
            if patch.param.boundary != 0 {
                adjust_boundary(patch.param.boundary, &mut w);
            }
            acc(&w, &patch.points);
        }
        PatchType::GregoryBasis => {
            let mut w = [0.0f32; 20];
            gregory_basis(s, t, &mut w);
            acc(&w, &patch.points);
        }
    }
    out
}
