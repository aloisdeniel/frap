// SPDX-License-Identifier: GPL-3.0-or-later
//! `Far::PrimvarRefinerReal<float>::InterpolateFaceVarying` for Catmull-Clark, applied to a
//! stencil builder (the only destination `StencilTableFactory` uses): face, edge and vertex
//! points of one face-varying channel, in OpenSubdiv's order of contributions.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/primvarRefiner.h` (`interpFVarFromFaces`,
//! `interpFVarFromEdges`, `interpFVarFromVerts`; Pixar, TOST / Apache-2.0 modified).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_refinement;
use super::refiner::TopologyRefiner;
use super::sdc::{self, Mask, VertexHood};
use super::tags::{RULE_CREASE, RULE_SMOOTH};
use super::{INDEX_INVALID, Index};

/// The destination of `AddWithWeight` calls (`StencilBuilder<float>::Index`): `dst` and `src`
/// are indices in the builder's combined index space.
pub(crate) trait FvarSink {
    fn add_with_weight(&mut self, dst: i32, src: i32, weight: f32);
}

/// `InterpolateFaceVarying(level, src, dst, channel)` with `src` and `dst` the builder offsets
/// of the parent and child level values.
pub(crate) fn interpolate_face_varying(
    refiner: &TopologyRefiner,
    level: usize,
    src: i32,
    dst: i32,
    channel: usize,
    sink: &mut dyn FvarSink,
) {
    from_faces(refiner, level, src, dst, channel, sink);
    from_edges(refiner, level, src, dst, channel, sink);
    from_verts(refiner, level, src, dst, channel, sink);
}

fn from_faces(
    refiner: &TopologyRefiner,
    level: usize,
    src: i32,
    dst: i32,
    channel: usize,
    sink: &mut dyn FvarSink,
) {
    let r = &refiner.refinements[level - 1];
    if r.child_vert_from_face_count == 0 {
        return;
    }
    let parent = &refiner.levels[level - 1];
    let child = &refiner.levels[level];
    let pf = &parent.fvar[channel];
    let cf = &child.fvar[channel];
    let mut m = Mask::default();
    for f in 0..parent.face_count {
        let cv = r.face_child_vert_index[f as usize];
        if cv == INDEX_INVALID {
            continue;
        }
        let cvv = cf.vertex_value_offset(cv, 0);
        let fvals = pf.face_values(parent, f);
        sdc::face_vertex_mask(fvals.len(), &mut m);
        for i in 0..fvals.len() {
            add(sink, dst + cvv, src + fvals[i], m.v[i]);
        }
    }
}

#[inline]
fn add(sink: &mut dyn FvarSink, dst: i32, src: i32, w: f32) {
    // `StencilBuilder::Index::AddWithWeight` skips zero weights.
    if w == 0.0 {
        return;
    }
    sink.add_with_weight(dst, src, w);
}

fn from_edges(
    refiner: &TopologyRefiner,
    level: usize,
    src: i32,
    dst: i32,
    channel: usize,
    sink: &mut dyn FvarSink,
) {
    let r = &refiner.refinements[level - 1];
    let parent = &refiner.levels[level - 1];
    let child = &refiner.levels[level];
    let pf = &parent.fvar[channel];
    let cf = &child.fvar[channel];
    let src_of = &r.fvar_child_value_parent_source[channel];
    let linear = pf.is_linear;
    let mut m = Mask::default();
    if linear {
        m.v = vec![0.5, 0.5];
        m.nv = 2;
    }
    for e in 0..parent.edge_count {
        let cv = r.edge_child_vert_index[e as usize];
        if cv == INDEX_INVALID {
            continue;
        }
        let cvals = cf.vertex_values(cv);
        let matches = cf.value_topology_matches(cvals[0]);
        if matches {
            if !linear {
                let p_rule = if parent.edge_sharpness(e) > 0.0 {
                    RULE_CREASE
                } else {
                    RULE_SMOOTH
                };
                let c_rule = child.vertex_rule(cv);
                sdc::edge_vertex_mask(
                    parent.edge_faces(e).len(),
                    parent.edge_sharpness(e),
                    p_rule,
                    c_rule,
                    &mut m,
                );
            }
            let ev = pf.edge_face_values(parent, e, 0);
            let cvv = cvals[0];
            add(sink, dst + cvv, src + ev[0], m.v[0]);
            add(sink, dst + cvv, src + ev[1], m.v[1]);
            if !linear && m.nf > 0 {
                let ef = parent.edge_faces(e);
                for i in 0..ef.len() {
                    if m.face_centers {
                        let cvf = r.face_child_vert_index[ef[i] as usize];
                        let cvalf = cf.vertex_value_offset(cvf, 0);
                        add(sink, dst + cvv, dst + cvalf, m.f[i]);
                    } else {
                        let pface = ef[i];
                        let pfe = parent.face_edges(pface);
                        let n = pfe.len();
                        let e_in_face = pfe.iter().position(|&x| x == e).unwrap_or(0);
                        let mut v_in_face = e_in_face + 2;
                        if v_in_face >= n {
                            v_in_face -= n;
                        }
                        let pnext = pf.face_values(parent, pface)[v_in_face];
                        add(sink, dst + cvv, src + pnext, m.f[i]);
                    }
                }
            }
        } else {
            let o = cf.vertex_value_offset(cv, 0) as usize;
            for i in 0..cvals.len() {
                let face_index = src_of[o + i] as usize;
                let ev = pf.edge_face_values(parent, e, face_index);
                let cvv = cvals[i];
                add(sink, dst + cvv, src + ev[0], 0.5);
                add(sink, dst + cvv, src + ev[1], 0.5);
            }
        }
    }
}

fn from_verts(
    refiner: &TopologyRefiner,
    level: usize,
    src: i32,
    dst: i32,
    channel: usize,
    sink: &mut dyn FvarSink,
) {
    let r = &refiner.refinements[level - 1];
    let parent = &refiner.levels[level - 1];
    let child = &refiner.levels[level];
    let pf = &parent.fvar[channel];
    let cf = &child.fvar[channel];
    let src_of = &r.fvar_child_value_parent_source[channel];
    let linear = pf.is_linear;
    let mut m = Mask::default();
    let mut edge_values: Vec<Index> = Vec::new();
    let mut edge_sharpness: Vec<f32> = Vec::new();
    for v in 0..parent.vert_count {
        let cv = r.vert_child_vert_index[v as usize];
        if cv == INDEX_INVALID {
            continue;
        }
        let pvals = pf.vertex_values(v);
        let cvals = cf.vertex_values(cv);
        let matches = cf.value_topology_matches(cvals[0]);
        if linear && matches {
            add(sink, dst + cvals[0], src + pvals[0], 1.0);
            continue;
        }
        if matches {
            let v_edges = parent.vertex_edges(v);
            edge_sharpness.clear();
            edge_sharpness.extend(v_edges.iter().map(|&e| parent.edge_sharpness(e)));
            let hood = VertexHood {
                num_edges: v_edges.len(),
                num_faces: parent.vertex_faces(v).len(),
                sharpness: parent.vertex_sharpness(v),
                edge_sharpness: &edge_sharpness,
                child_sharpness: child.vertex_sharpness(cv),
            };
            sdc::vertex_vertex_mask(&hood, &mut m, parent.vertex_rule(v), child.vertex_rule(cv));
            let pvv = pvals[0];
            let cvv = cvals[0];
            if m.nf > 0 {
                let vf = parent.vertex_faces(v);
                for i in 0..vf.len() {
                    let cvf = r.face_child_vert_index[vf[i] as usize];
                    let cvalf = cf.vertex_value_offset(cvf, 0);
                    add(sink, dst + cvv, dst + cvalf, m.f[i]);
                }
            }
            if m.ne > 0 {
                pf.vertex_edge_values(parent, v, &mut edge_values);
                for i in 0..v_edges.len() {
                    add(sink, dst + cvv, src + edge_values[i], m.e[i]);
                }
            }
            add(sink, dst + cvv, src + pvv, m.v[0]);
        } else {
            let ptags = pf.vertex_value_tags(v);
            let ctags = cf.vertex_value_tags(cv);
            let o = cf.vertex_value_offset(cv, 0) as usize;
            for cs in 0..cvals.len() {
                let ps = src_of[o + cs] as usize;
                let pvv = pvals[ps];
                let cvv = cvals[cs];
                if linear || ctags[cs].is_corner() {
                    add(sink, dst + cvv, src + pvv, 1.0);
                } else {
                    let ends = pf.vertex_crease_end_values(parent, v, ps);
                    let mut vw = 0.75f32;
                    let mut ew = 0.125f32;
                    if ptags[ps].semi_sharp() {
                        let w_corner = if ptags[ps].dep_sharp() {
                            fvar_refinement::fractional_weight(
                                r,
                                parent,
                                child,
                                channel,
                                v,
                                (ps == 0) as usize,
                                cv,
                            )
                        } else {
                            fvar_refinement::fractional_weight(r, parent, child, channel, v, ps, cv)
                        };
                        let w_crease = 1.0f32 - w_corner;
                        vw = w_crease.mul_add(0.75, w_corner);
                        ew = w_crease * 0.125;
                    }
                    add(sink, dst + cvv, src + ends[0], ew);
                    add(sink, dst + cvv, src + ends[1], ew);
                    add(sink, dst + cvv, src + pvv, vw);
                }
            }
        }
    }
}
