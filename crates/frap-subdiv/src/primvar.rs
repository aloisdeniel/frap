// SPDX-License-Identifier: GPL-3.0-or-later
//! Primvar refinement and limit evaluation (`Far::PrimvarRefiner`): vertex data (positions)
//! and face-varying data (UV maps) through one refinement step, and limit positions at the last
//! level.
//!
//! Ported from OpenSubdiv 3.x `opensubdiv/far/primvarRefiner.h` (`interpFromFaces`,
//! `interpFromEdges`, `interpFromVerts`, `interpFVarFrom*`, `limit`, `limitFVar`). Values are
//! accumulated in `f64` (OpenSubdiv's stencils are `float`; the difference is far below the
//! golden tolerance) in OpenSubdiv's order: face weights, then edge weights, then the vertex.

use smallvec::SmallVec;

use crate::fvar::FVarLevel;
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
        let fv = parent.face_verts(f);
        let w = 1.0 / fv.len() as f64;
        let d = &mut dst[r.face_child_vert[f] as usize];
        for &v in fv {
            madd(d, &src[v as usize], w);
        }
    }
    for e in 0..parent.nedges() {
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

/// The value slots of a face-varying refinement: where the children of each parent component
/// put their values.
#[derive(Clone, Debug, Default)]
pub(crate) struct FVarChildSlots {
    pub face: Vec<u32>,
    pub edge_offset: Vec<u32>,
    pub vert_offset: Vec<u32>,
    pub nslots: usize,
}

/// Child face-values of one refinement step (`FVarRefinement::populateChildValues*`): a face
/// child has one value; an edge child one per incident face when the edge is discontinuous;
/// a vertex child one per parent value when the vertex does not match.
pub(crate) fn refine_fvar_topology(
    parent: &Level,
    r: &Refinement,
    pf: &FVarLevel,
) -> (Vec<u32>, FVarChildSlots) {
    let mut slots = FVarChildSlots::default();
    let mut n = 0u32;
    for _ in 0..parent.nfaces() {
        slots.face.push(n);
        n += 1;
    }
    for e in 0..parent.nedges() {
        slots.edge_offset.push(n);
        n += if pf.edge_tags[e].mismatch {
            parent.edge_faces[e].len() as u32
        } else {
            1
        };
    }
    for v in 0..parent.nverts {
        slots.vert_offset.push(n);
        n += if pf.vertex_matches(v) {
            1
        } else {
            pf.vert_values[v].len() as u32
        };
    }
    slots.nslots = n as usize;
    let mut fvals = Vec::with_capacity(parent.face_verts.len() * 4);
    for f in 0..parent.nfaces() {
        let fv = parent.face_verts(f);
        let fe = parent.face_edges(f);
        let k_n = fv.len();
        let edge_slot = |e: u32| -> u32 {
            let e = e as usize;
            let off = slots.edge_offset[e];
            if !pf.edge_tags[e].mismatch {
                return off;
            }
            let faces = &parent.edge_faces[e];
            let mut j = 0;
            if faces.len() == 2 {
                j = (faces[1] as usize == f) as u32;
            } else {
                for (i, &x) in faces.iter().enumerate() {
                    if x as usize == f {
                        j = i as u32;
                    }
                }
            }
            off + j
        };
        for k in 0..k_n {
            let v = fv[k] as usize;
            let vslot = if pf.vertex_matches(v) || pf.vert_values[v].len() == 1 {
                slots.vert_offset[v]
            } else {
                let i = parent.vert_faces[v]
                    .iter()
                    .enumerate()
                    .position(|(i, &x)| {
                        x as usize == f && parent.vert_face_local[v][i] as usize == k
                    })
                    .unwrap_or(0);
                slots.vert_offset[v] + pf.vert_face_siblings[v][i] as u32
            };
            fvals.push(vslot);
            fvals.push(edge_slot(fe[k]));
            fvals.push(slots.face[f]);
            fvals.push(edge_slot(fe[(k + k_n - 1) % k_n]));
        }
    }
    let _ = r;
    (fvals, slots)
}

/// `PrimvarRefiner::InterpolateFaceVarying` for one channel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn interp_fvar<const N: usize>(
    parent: &Level,
    child: &Level,
    r: &Refinement,
    pf: &FVarLevel,
    cf: &FVarLevel,
    slots: &FVarChildSlots,
    src: &[V<N>],
) -> Vec<V<N>> {
    let mut dst = vec![[0.0; N]; slots.nslots];
    let linear = pf.is_linear;
    for f in 0..parent.nfaces() {
        let fvals = pf.face_values(parent, f);
        let w = 1.0 / fvals.len() as f64;
        let d = &mut dst[slots.face[f] as usize];
        for &s in fvals {
            madd(d, &src[s as usize], w);
        }
    }
    for e in 0..parent.nedges() {
        let off = slots.edge_offset[e] as usize;
        let cv = r.edge_child_vert[e] as usize;
        let matches = !cf.value_tags[off].mismatch;
        if matches {
            let (vw, fw) = if linear {
                (0.5, 0.0)
            } else {
                let s = parent.edge_sharpness[e];
                sdc::edge_vertex_mask(
                    s,
                    s > 0.0,
                    child.vert_tags[cv].rule,
                    parent.edge_faces[e].len(),
                )
            };
            let [a, b] = pf.edge_face_values(parent, e, 0);
            let mut acc = [0.0; N];
            madd(&mut acc, &src[a as usize], vw);
            madd(&mut acc, &src[b as usize], vw);
            if fw != 0.0 {
                for &f in &parent.edge_faces[e] {
                    let fc = dst[slots.face[f as usize] as usize];
                    madd(&mut acc, &fc, fw);
                }
            }
            dst[off] = acc;
        } else {
            for j in 0..parent.edge_faces[e].len() {
                let [a, b] = pf.edge_face_values(parent, e, j);
                let mut acc = [0.0; N];
                madd(&mut acc, &src[a as usize], 0.5);
                madd(&mut acc, &src[b as usize], 0.5);
                dst[off + j] = acc;
            }
        }
    }
    let mut m = Mask::default();
    for v in 0..parent.nverts {
        let off = slots.vert_offset[v] as usize;
        let matches = !cf.value_tags[off].mismatch;
        let pvals = &pf.vert_values[v];
        if linear && matches {
            dst[off] = src[pvals[0] as usize];
            continue;
        }
        if matches {
            vertex_mask(parent, child, r, v, &mut m);
            let mut acc = [0.0; N];
            for (i, &f) in parent.vert_faces[v].iter().enumerate() {
                if let Some(&w) = m.faces.get(i) {
                    let fc = dst[slots.face[f as usize] as usize];
                    madd(&mut acc, &fc, w);
                }
            }
            if !m.edges.is_empty() {
                let ev = pf.vertex_edge_values(parent, v);
                for (i, &val) in ev.iter().enumerate() {
                    let w = m.edges[i];
                    if w != 0.0 {
                        madd(&mut acc, &src[val as usize], w);
                    }
                }
            }
            madd(&mut acc, &src[pvals[0] as usize], m.vertex);
            dst[off] = acc;
            continue;
        }
        let cv = r.vert_child_vert[v] as usize;
        for (s, &pslot) in pvals.iter().enumerate() {
            let cslot = off + s;
            if linear || cf.value_tags[cslot].is_corner() {
                dst[cslot] = src[pslot as usize];
                continue;
            }
            let [a, b] = pf.crease_end_values(parent, v, pslot);
            let (mut vw, mut ew) = (0.75, 0.125);
            let ptag = pf.value_tags[pslot as usize];
            if ptag.semi_sharp {
                let wslot = if ptag.dep_sharp && pvals.len() == 2 {
                    pvals[1 - s]
                } else {
                    pslot
                };
                let interior = pf.crease_interior_edges(parent, v, wslot);
                let pe: SmallVec<[f32; 6]> = interior
                    .iter()
                    .map(|&i| parent.edge_sharpness[parent.vert_edges[v][i] as usize])
                    .collect();
                let w_corner = sdc::fractional_weight_at_vertex(
                    parent.vert_sharpness[v],
                    child.vert_sharpness[cv],
                    &pe,
                ) as f64;
                let w_crease = 1.0 - w_corner;
                vw = w_crease * 0.75 + w_corner;
                ew = w_crease * 0.125;
            }
            let mut acc = [0.0; N];
            madd(&mut acc, &src[a as usize], ew);
            madd(&mut acc, &src[b as usize], ew);
            madd(&mut acc, &src[pslot as usize], vw);
            dst[cslot] = acc;
        }
    }
    dst
}

/// `PrimvarRefiner::LimitFaceVarying` for the values of the vertices listed in `which`
/// (smooth corners in face-varying space as in [`limit_vertex`]).
pub(crate) fn limit_fvar<const N: usize>(
    lvl: &Level,
    fl: &FVarLevel,
    src: &[V<N>],
    which: impl Iterator<Item = usize>,
    inf_only: bool,
    corner_steps: usize,
    dst: &mut [V<N>],
) {
    let mut m = Mask::default();
    for v in which {
        let vals = &fl.vert_values[v];
        if fl.is_linear || lvl.vert_edges[v].is_empty() {
            for &s in vals {
                dst[s as usize] = src[s as usize];
            }
            continue;
        }
        if fl.vertex_matches(v) {
            let (rule, es) = limit_rule(lvl, v, inf_only);
            if is_smooth_corner(lvl, v, rule) {
                let ends = sdc::sharp_edge_pair(&es);
                let ev = fl.vertex_edge_values(lvl, v);
                let s = vals[0] as usize;
                dst[s] = crease_refined(
                    src[s],
                    src[ev[ends[0]] as usize],
                    src[ev[ends[1]] as usize],
                    corner_steps,
                );
                continue;
            }
            sdc::vertex_limit_mask(lvl.vert_faces[v].len(), rule, &es, &mut m);
            let mut acc = [0.0; N];
            for (i, &f) in lvl.vert_faces[v].iter().enumerate() {
                if let Some(&w) = m.faces.get(i) {
                    let fvals = fl.face_values(lvl, f as usize);
                    let k = lvl.vert_face_local[v][i] as usize;
                    let opp = fvals[(k + 2) % fvals.len()] as usize;
                    madd(&mut acc, &src[opp], w);
                }
            }
            if !m.edges.is_empty() {
                let ev = fl.vertex_edge_values(lvl, v);
                for (i, &val) in ev.iter().enumerate() {
                    let w = m.edges[i];
                    if w != 0.0 {
                        madd(&mut acc, &src[val as usize], w);
                    }
                }
            }
            madd(&mut acc, &src[vals[0] as usize], m.vertex);
            dst[vals[0] as usize] = acc;
        } else {
            for &s in vals {
                if fl.value_tags[s as usize].is_corner() {
                    dst[s as usize] = src[s as usize];
                } else if fl.crease_ends[s as usize][0] == fl.crease_ends[s as usize][1] {
                    // A one-face crease value: a smooth corner in face-varying space.
                    let [a, b] = fl.crease_end_values(lvl, v, s);
                    dst[s as usize] = crease_refined(
                        src[s as usize],
                        src[a as usize],
                        src[b as usize],
                        corner_steps,
                    );
                } else {
                    let [a, b] = fl.crease_end_values(lvl, v, s);
                    let mut acc = [0.0; N];
                    madd(&mut acc, &src[a as usize], 1.0 / 6.0);
                    madd(&mut acc, &src[b as usize], 1.0 / 6.0);
                    madd(&mut acc, &src[s as usize], 2.0 / 3.0);
                    dst[s as usize] = acc;
                }
            }
        }
    }
}
