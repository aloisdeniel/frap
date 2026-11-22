// SPDX-License-Identifier: GPL-3.0-or-later
//! `Vtr::internal::FVarRefinement`: refines one face-varying channel along a [`Refinement`] —
//! the child values (siblings) of every child vertex, their propagated tags and crease ends,
//! and the child face values.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/vtr/fvarRefinement.{h,cpp}` (Pixar, TOST /
//! Apache-2.0 modified).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_level::{CreaseEndPair, FVarLevel};
use super::level::Level;
use super::refinement::Refinement;
use super::sdc;
use super::tags::ValueTag;
use super::{Index, LocalIndex};

/// `FVarRefinement::applyRefinement`: the child channel `c` and its `_childValueParentSource`.
pub(crate) fn apply(
    r: &Refinement,
    parent: &Level,
    child: &Level,
    c: usize,
) -> (FVarLevel, Vec<LocalIndex>) {
    let pf = &parent.fvar[c];
    let mut cf = FVarLevel {
        options: pf.options,
        is_linear: pf.is_linear,
        has_linear_boundaries: pf.has_linear_boundaries,
        has_dependent_sharpness: pf.has_dependent_sharpness,
        ..FVarLevel::default()
    };

    // estimateAndAllocateChildValues
    let mut max_values = r.child_vert_from_face_count as usize;
    let ve0 = r.first_child_vert_from_edge;
    for cv in ve0..ve0 + r.child_vert_from_edge_count {
        let pe = r.child_vertex_parent_index[cv as usize];
        max_values += if pf.edge_topology_matches(pe) {
            1
        } else {
            parent.edge_faces(pe).len()
        };
    }
    let vv0 = r.first_child_vert_from_vert;
    for cv in vv0..vv0 + r.child_vert_from_vert_count {
        let pv = r.child_vertex_parent_index[cv as usize];
        max_values += pf.num_vertex_values(pv) as usize;
    }
    cf.resize_components(child);
    cf.vert_value_tags = vec![ValueTag::default(); max_values];
    let mut src: Vec<LocalIndex> = vec![0; max_values];

    // populateChildValues (vertices first)
    cf.value_count = 0;
    for cv in vv0..vv0 + r.child_vert_from_vert_count {
        let pv = r.child_vertex_parent_index[cv as usize];
        cf.vert_sibling_offsets[cv as usize] = cf.value_count;
        if pf.value_topology_matches(pf.vertex_value_offset(pv, 0)) {
            cf.vert_sibling_counts[cv as usize] = 1;
            cf.value_count += 1;
        } else {
            // populateChildValuesForVertexVertex
            let n = pf.num_vertex_values(pv);
            if n > 1 {
                let o = cf.vert_sibling_offsets[cv as usize] as usize;
                for j in 1..n as usize {
                    src[o + j] = j as LocalIndex;
                }
                let ps = pf.vertex_face_siblings(parent, pv).to_vec();
                let co = child.offset_of_vertex_faces(cv) as usize;
                let cn = child.num_vertex_faces(cv) as usize;
                cf.vert_face_siblings[co..co + cn].copy_from_slice(&ps[..cn]);
            }
            cf.vert_sibling_counts[cv as usize] = n as LocalIndex;
            cf.value_count += n;
        }
    }
    let vf0 = r.first_child_vert_from_face;
    for cv in vf0..vf0 + r.child_vert_from_face_count {
        cf.vert_sibling_offsets[cv as usize] = cf.value_count;
        cf.vert_sibling_counts[cv as usize] = 1;
        cf.value_count += 1;
    }
    for cv in ve0..ve0 + r.child_vert_from_edge_count {
        let pe = r.child_vertex_parent_index[cv as usize];
        cf.vert_sibling_offsets[cv as usize] = cf.value_count;
        if pf.edge_topology_matches(pe) {
            cf.vert_sibling_counts[cv as usize] = 1;
            cf.value_count += 1;
        } else {
            // populateChildValuesForEdgeVertex
            let pef = parent.edge_faces(pe);
            let n = if pef.len() == 1 {
                1
            } else {
                let o = cf.vert_sibling_offsets[cv as usize] as usize;
                for i in 0..pef.len() {
                    src[o + i] = i as LocalIndex;
                }
                let cvf = child.vertex_faces(cv);
                let co = child.offset_of_vertex_faces(cv) as usize;
                for i in 0..cvf.len() {
                    let pfi = r.child_face_parent_face(cvf[i]);
                    if pef.len() == 2 {
                        if pfi == pef[1] {
                            cf.vert_face_siblings[co + i] = 1;
                        }
                    } else {
                        for j in 0..pef.len() {
                            if pfi == pef[j] {
                                cf.vert_face_siblings[co + i] = j as LocalIndex;
                            }
                        }
                    }
                }
                pef.len()
            };
            cf.vert_sibling_counts[cv as usize] = n as LocalIndex;
            cf.value_count += n as i32;
        }
    }

    // trimAndFinalizeChildValues
    let nvals = cf.value_count as usize;
    cf.vert_value_tags.resize(nvals, ValueTag::default());
    if cf.has_smooth_boundaries() {
        cf.vert_value_crease_ends = vec![CreaseEndPair::default(); nvals];
    }
    src.resize(nvals, 0);
    cf.vert_value_indices = (0..nvals as Index).collect();

    // propagateEdgeTags
    let nfe = r.child_edge_from_face_count as usize;
    for e in 0..child.edge_count as usize {
        cf.edge_tags[e] = if e < nfe {
            Default::default()
        } else {
            pf.edge_tags[r.child_edge_parent_index[e] as usize]
        };
    }

    // propagateValueTags
    for cv in vf0..vf0 + r.child_vert_from_face_count {
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        cf.vert_value_tags[o] = ValueTag::default();
    }
    let mut mismatch = ValueTag::default();
    mismatch.set_mismatch(true);
    let mut crease = mismatch;
    crease.set_crease(true);
    let split_edge = if pf.has_smooth_boundaries() {
        crease
    } else {
        mismatch
    };
    for cv in ve0..ve0 + r.child_vert_from_edge_count {
        let pe = r.child_vertex_parent_index[cv as usize];
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        let n = cf.vert_sibling_counts[cv as usize] as usize;
        let pt = pf.edge_tags[pe as usize];
        let t = if pt.mismatch() || pt.linear() {
            split_edge
        } else {
            ValueTag::default()
        };
        for k in 0..n {
            cf.vert_value_tags[o + k] = t;
        }
    }
    for cv in vv0..vv0 + r.child_vert_from_vert_count {
        let pv = r.child_vertex_parent_index[cv as usize];
        let ptags = pf.vertex_value_tags(pv);
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        cf.vert_value_tags[o..o + ptags.len()].copy_from_slice(ptags);
    }

    if cf.has_smooth_boundaries() {
        propagate_value_creases(r, pf, &mut cf);
        reclassify_semisharp_values(r, parent, child, pf, &mut cf);
    }

    if cf.value_count > child.vert_count {
        cf.initialize_face_values_from_vertex_face_siblings(child);
    } else {
        cf.initialize_face_values_from_face_vertices(child);
    }
    (cf, src)
}

/// `propagateValueCreases`
fn propagate_value_creases(r: &Refinement, pf: &FVarLevel, cf: &mut FVarLevel) {
    let inc = 2usize;
    let ve0 = r.first_child_vert_from_edge;
    for cv in ve0..ve0 + r.child_vert_from_edge_count {
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        let n = cf.vert_sibling_counts[cv as usize] as usize;
        if !cf.vert_value_tags[o].mismatch() || !r.is_child_vertex_complete(cv) {
            continue;
        }
        let mut start = 0usize;
        let mut end = start + inc - 1;
        for i in 0..n {
            if !cf.vert_value_tags[o + i].is_inf_sharp() {
                cf.vert_value_crease_ends[o + i] = CreaseEndPair {
                    start_face: start as LocalIndex,
                    end_face: end as LocalIndex,
                };
            }
            start += inc;
            end += inc;
        }
    }
    let vv0 = r.first_child_vert_from_vert;
    for cv in vv0..vv0 + r.child_vert_from_vert_count {
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        let n = cf.vert_sibling_counts[cv as usize] as usize;
        if !cf.vert_value_tags[o].mismatch() || !r.is_child_vertex_complete(cv) {
            continue;
        }
        let pv = r.child_vertex_parent_index[cv as usize];
        let pends = pf.vertex_value_crease_ends(pv);
        for j in 0..n {
            if !cf.vert_value_tags[o + j].is_inf_sharp() {
                cf.vert_value_crease_ends[o + j] = pends[j];
            }
        }
    }
}

/// The child vertex's edges (`cVertEdges`), from the child level or through the parent.
fn child_vertex_edges(
    r: &Refinement,
    parent: &Level,
    child: &Level,
    cv: Index,
    pv: Index,
) -> Vec<Index> {
    if child.num_vertex_edges_total() > 0 {
        child.vertex_edges(cv).to_vec()
    } else {
        let pe = parent.vertex_edges(pv);
        let pin = parent.vertex_edge_local_indices(pv);
        (0..pe.len())
            .map(|i| r.edge_child_edges(pe[i])[pin[i] as usize])
            .collect()
    }
}

/// `reclassifySemisharpValues`
fn reclassify_semisharp_values(
    r: &Refinement,
    parent: &Level,
    child: &Level,
    pf: &FVarLevel,
    cf: &mut FVarLevel,
) {
    let has_dep = pf.has_dependent_sharpness;
    let vv0 = r.first_child_vert_from_vert;
    for cv in vv0..vv0 + r.child_vert_from_vert_count {
        let o = cf.vert_sibling_offsets[cv as usize] as usize;
        let n = cf.vert_sibling_counts[cv as usize] as usize;
        if !cf.vert_value_tags[o].mismatch() || !r.is_child_vertex_complete(cv) {
            continue;
        }
        let pv = r.child_vertex_parent_index[cv as usize];
        let pt = parent.vertex_tag(pv);
        if !pt.semi_sharp() && !pt.semi_sharp_edges() {
            continue;
        }
        let ct = child.vertex_tag(cv);
        if ct.semi_sharp() || ct.inf_sharp() {
            continue;
        }
        if !ct.semi_sharp() && !ct.semi_sharp_edges() {
            for j in 0..n {
                let t = &mut cf.vert_value_tags[o + j];
                if t.semi_sharp() {
                    t.set_semi_sharp(false);
                    t.set_dep_sharp(false);
                    t.set_crease(true);
                }
            }
            continue;
        }
        let edges = child_vertex_edges(r, parent, child, cv, pv);
        for j in 0..n {
            let t = cf.vert_value_tags[o + j];
            if t.semi_sharp() && !t.dep_sharp() {
                let ends = cf.vert_value_crease_ends[o + j];
                let (s, e) = (ends.start_face as usize, ends.end_face as usize);
                let mut still = false;
                if e > s {
                    for k in s + 1..=e {
                        if still {
                            break;
                        }
                        still = child.edge_tag(edges[k]).semi_sharp();
                    }
                } else if s > e {
                    for k in s + 1..edges.len() {
                        if still {
                            break;
                        }
                        still = child.edge_tag(edges[k]).semi_sharp();
                    }
                    for k in 0..=e {
                        if still {
                            break;
                        }
                        still = child.edge_tag(edges[k]).semi_sharp();
                    }
                }
                if !still {
                    let t = &mut cf.vert_value_tags[o + j];
                    t.set_semi_sharp(false);
                    t.set_dep_sharp(false);
                    t.set_crease(true);
                }
            }
        }
        if n == 2 && has_dep {
            let (t0, t1) = (cf.vert_value_tags[o], cf.vert_value_tags[o + 1]);
            if t0.dep_sharp() && !t1.semi_sharp() {
                cf.vert_value_tags[o].set_dep_sharp(false);
            } else if t1.dep_sharp() && !t0.semi_sharp() {
                cf.vert_value_tags[o + 1].set_dep_sharp(false);
            }
        }
    }
}

/// `FVarRefinement::getFractionalWeight`
pub(crate) fn fractional_weight(
    r: &Refinement,
    parent: &Level,
    child: &Level,
    c: usize,
    pv: Index,
    p_sibling: usize,
    cv: Index,
) -> f32 {
    // Uniform creasing ignores the child edge sharpness (`IsUniform()`), so the child edges
    // (`cVertEdges`) are not gathered.
    let _ = r;
    let pf = &parent.fvar[c];
    let pe = parent.vertex_edges(pv);
    let ends = pf.vertex_value_crease_ends(pv)[p_sibling];
    let (s, e) = (ends.start_face as usize, ends.end_face as usize);
    let mut psharp = Vec::new();
    if e > s {
        for i in s + 1..=e {
            psharp.push(parent.edge_sharpness(pe[i]));
        }
    } else if s > e {
        for i in s + 1..pe.len() {
            psharp.push(parent.edge_sharpness(pe[i]));
        }
        for i in 0..=e {
            psharp.push(parent.edge_sharpness(pe[i]));
        }
    }
    sdc::fractional_weight_at_vertex(
        parent.vertex_sharpness(pv),
        child.vertex_sharpness(cv),
        &psharp,
    )
}
