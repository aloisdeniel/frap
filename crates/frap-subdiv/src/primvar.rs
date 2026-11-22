// SPDX-License-Identifier: GPL-3.0-or-later
//! Primvar refinement and limit evaluation (`Far::PrimvarRefiner`): vertex data (positions)
//! through one refinement step, and limit positions at the last level. Face-varying data (UV
//! maps) is evaluated bit-exactly by `crate::far`.
//!
//! Ported from OpenSubdiv 3.x `opensubdiv/far/primvarRefiner.h` (`interpFromFaces`,
//! `interpFromEdges`, `interpFromVerts`, `limit`). Values are
//! accumulated in `f64` (OpenSubdiv's stencils are `float`; the difference is far below the
//! golden tolerance) in OpenSubdiv's order: face weights, then edge weights, then the vertex.

use smallvec::SmallVec;

use crate::sdc::{self, Mask, Rule};
use crate::vtr::{Level, Refinement};

pub(crate) type V<const N: usize> = [f64; N];

#[inline]
fn madd<const N: usize>(d: &mut V<N>, s: &V<N>, w: f64) {
    for k in 0..N {
        d[k] += s[k] * w;
    }
}

/// The child edges incident to the vertex-child of `v`, in the parent's `vert_edges` order.
fn child_vert_edge_sharpness(
    parent: &Level,
    child: &Level,
    r: &Refinement,
    v: usize,
) -> SmallVec<[f32; 6]> {
    parent.vert_edges[v]
        .iter()
        .enumerate()
        .map(|(i, &e)| {
            let k = parent.vert_edge_local[v][i] as usize;
            let ce = r.edge_child_edges[e as usize][k];
            if ce == u32::MAX {
                0.0
            } else {
                child.edge_sharpness[ce as usize]
            }
        })
        .collect()
}

/// The vertex-vertex mask of `v` (parent and child rules from the tags).
fn vertex_mask(parent: &Level, child: &Level, r: &Refinement, v: usize, m: &mut Mask) {
    let cv = r.vert_child_vert[v] as usize;
    let p_rule = parent.vert_tags[v].rule;
    let c_rule = child.vert_tags[cv].rule;
    let pe = parent.vert_edge_sharpness(v);
    let ce = if p_rule != c_rule {
        child_vert_edge_sharpness(parent, child, r, v)
    } else {
        SmallVec::new()
    };
    sdc::vertex_vertex_mask(
        parent.vert_faces[v].len(),
        p_rule,
        c_rule,
        parent.vert_sharpness[v],
        child.vert_sharpness[cv],
        &pe,
        &ce,
        m,
    );
}

/// `PrimvarRefiner::Interpolate` for vertex data.
pub(crate) fn interp_vertex<const N: usize>(
    parent: &Level,
    child: &Level,
    r: &Refinement,
    src: &[V<N>],
) -> Vec<V<N>> {
    let mut dst = vec![[0.0; N]; child.nverts];
    for f in 0..parent.nfaces() {
        frap_math::budget::check();
        let fv = parent.face_verts(f);
        let w = 1.0 / fv.len() as f64;
        let d = &mut dst[r.face_child_vert[f] as usize];
        for &v in fv {
            madd(d, &src[v as usize], w);
        }
    }
    for e in 0..parent.nedges() {
        frap_math::budget::check();
        let cv = r.edge_child_vert[e] as usize;
        let s = parent.edge_sharpness[e];
        let faces = &parent.edge_faces[e];
        let (vw, fw) = sdc::edge_vertex_mask(s, s > 0.0, child.vert_tags[cv].rule, faces.len());
        let ev = parent.edge_verts[e];
        let mut acc = [0.0; N];
        madd(&mut acc, &src[ev[0] as usize], vw);
        madd(&mut acc, &src[ev[1] as usize], vw);
        if fw != 0.0 {
            for &f in faces {
                let fc = dst[r.face_child_vert[f as usize] as usize];
                madd(&mut acc, &fc, fw);
            }
        }
        dst[cv] = acc;
    }
    let mut m = Mask::default();
    for v in 0..parent.nverts {
        frap_math::budget::check();
        let cv = r.vert_child_vert[v] as usize;
        vertex_mask(parent, child, r, v, &mut m);
        let mut acc = [0.0; N];
        for (i, &f) in parent.vert_faces[v].iter().enumerate() {
            if let Some(&w) = m.faces.get(i) {
                let fc = dst[r.face_child_vert[f as usize] as usize];
                madd(&mut acc, &fc, w);
            }
        }
        for (i, &e) in parent.vert_edges[v].iter().enumerate() {
            if let Some(&w) = m.edges.get(i)
                && w != 0.0
            {
                let o = parent.other_vert(e, v as u32) as usize;
                madd(&mut acc, &src[o], w);
            }
        }
        madd(&mut acc, &src[v], m.vertex);
        dst[cv] = acc;
    }
    dst
}

/// The rule a limit mask uses at a vertex. `inf_only`: the isolation level was reached with
/// semi-sharp features left, which OpenSubdiv's end-cap patches ignore (only infinitely sharp
/// edges and vertices shape the irregular patch, `PatchBuilder::getIrregularFacePoints`).
fn limit_rule(lvl: &Level, v: usize, inf_only: bool) -> (Rule, SmallVec<[f32; 6]>) {
    let mut es = lvl.vert_edge_sharpness(v);
    if !inf_only {
        return (lvl.vert_tags[v].rule, es);
    }
    for s in es.iter_mut() {
        if !sdc::is_infinite(*s) {
            *s = 0.0;
        }
    }
    let rule = if lvl.vert_tags[v].inf_sharp {
        Rule::Corner
    } else {
        Rule::determine(0.0, es.iter().filter(|&&s| s > 0.0).count())
    };
    (rule, es)
}

/// A smooth boundary corner (one face, crease rule): OpenSubdiv's patch tables approximate
/// it with a sharp regular corner patch once isolated (`generateLegacySharpCornerPatches`,
/// on by default; `PatchBuilder::IsPatchRegular`), so its surface point is its refined
/// position at the isolation level rather than its limit.
fn is_smooth_corner(lvl: &Level, v: usize, rule: Rule) -> bool {
    let t = lvl.vert_tags[v];
    rule == Rule::Crease
        && t.boundary
        && !t.inf_sharp
        && !t.non_manifold
        && lvl.vert_faces[v].len() == 1
}

/// The refined position of a crease-rule point after `steps` more levels: along the crease
/// the vertex point is `3/4 v + 1/8 (a + b)` and the new neighbours are the edge midpoints.
fn crease_refined<const N: usize>(mut v: V<N>, mut a: V<N>, mut b: V<N>, steps: usize) -> V<N> {
    for _ in 0..steps {
        let mut nv = [0.0; N];
        for k in 0..N {
            nv[k] = 0.75 * v[k] + 0.125 * (a[k] + b[k]);
            a[k] = 0.5 * (v[k] + a[k]);
            b[k] = 0.5 * (v[k] + b[k]);
        }
        v = nv;
    }
    v
}

/// `PrimvarRefiner::Limit` (positions) at the vertices of `lvl` listed in `which`;
/// `corner_steps` levels remain to the isolation level (smooth corners, see
/// [`is_smooth_corner`]).
pub(crate) fn limit_vertex<const N: usize>(
    lvl: &Level,
    src: &[V<N>],
    which: impl Iterator<Item = usize>,
    inf_only: bool,
    corner_steps: usize,
    dst: &mut [V<N>],
) {
    let mut m = Mask::default();
    for v in which {
        frap_math::budget::check();
        let (rule, es) = limit_rule(lvl, v, inf_only);
        if is_smooth_corner(lvl, v, rule) {
            let ends = sdc::sharp_edge_pair(&es);
            let ve = &lvl.vert_edges[v];
            let a = lvl.other_vert(ve[ends[0]], v as u32) as usize;
            let b = lvl.other_vert(ve[ends[1]], v as u32) as usize;
            dst[v] = crease_refined(src[v], src[a], src[b], corner_steps);
            continue;
        }
        sdc::vertex_limit_mask(lvl.vert_faces[v].len(), rule, &es, &mut m);
        let mut acc = [0.0; N];
        for (i, &f) in lvl.vert_faces[v].iter().enumerate() {
            if let Some(&w) = m.faces.get(i) {
                let fv = lvl.face_verts(f as usize);
                let k = lvl.vert_face_local[v][i] as usize;
                let opp = fv[(k + 2) % fv.len()] as usize;
                madd(&mut acc, &src[opp], w);
            }
        }
        for (i, &e) in lvl.vert_edges[v].iter().enumerate() {
            if let Some(&w) = m.edges.get(i)
                && w != 0.0
            {
                let o = lvl.other_vert(e, v as u32) as usize;
                madd(&mut acc, &src[o], w);
            }
        }
        madd(&mut acc, &src[v], m.vertex);
        dst[v] = acc;
    }
}
