// SPDX-License-Identifier: GPL-3.0-or-later
//! `Vtr::internal::Refinement` and `QuadRefinement`: uniform or sparse (feature-adaptive)
//! Catmull-Clark splitting of a parent [`Level`] into a child level — the parent/child index
//! maps, the child tags, the six child relations, the subdivided sharpness and the refined
//! face-varying channels — plus `SparseSelector`.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/vtr/{refinement,quadRefinement,sparseSelector}.
//! {h,cpp}` (Pixar, TOST / Apache-2.0 modified). Child vertices are ordered from vertices, then
//! faces, then edges (`orderVerticesFromFacesFirst = false`, Blender's default).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::fvar_refinement;
use super::level::{Level, smooth_vtag};
use super::sdc;
use super::tags::{ETag, RULE_CREASE, RULE_SMOOTH, VTag};
use super::{INDEX_INVALID, Index, LocalIndex};

/// `Refinement::SparseTag`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct SparseTag {
    pub selected: bool,
    pub transitional: u8,
}

/// `Refinement::ChildTag`
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct ChildTag {
    pub incomplete: bool,
    pub index_in_parent: u8,
}

/// The refinement between level `i` and `i + 1`.
#[derive(Clone, Debug, Default)]
pub(crate) struct Refinement {
    pub(crate) uniform: bool,

    pub(crate) child_face_from_face_count: i32,
    pub(crate) child_edge_from_face_count: i32,
    pub(crate) child_edge_from_edge_count: i32,
    pub(crate) child_vert_from_face_count: i32,
    pub(crate) child_vert_from_edge_count: i32,
    pub(crate) child_vert_from_vert_count: i32,
    pub(crate) first_child_face_from_face: i32,
    pub(crate) first_child_edge_from_face: i32,
    pub(crate) first_child_edge_from_edge: i32,
    pub(crate) first_child_vert_from_face: i32,
    pub(crate) first_child_vert_from_edge: i32,
    pub(crate) first_child_vert_from_vert: i32,

    /// Shared with the parent's `faceVertCountsAndOffsets`.
    pub(crate) face_child_co: Vec<Index>,
    pub(crate) face_child_face_indices: Vec<Index>,
    pub(crate) face_child_edge_indices: Vec<Index>,
    pub(crate) face_child_vert_index: Vec<Index>,
    pub(crate) edge_child_edge_indices: Vec<Index>,
    pub(crate) edge_child_vert_index: Vec<Index>,
    pub(crate) vert_child_vert_index: Vec<Index>,

    pub(crate) child_face_parent_index: Vec<Index>,
    pub(crate) child_edge_parent_index: Vec<Index>,
    pub(crate) child_vertex_parent_index: Vec<Index>,
    pub(crate) child_face_tag: Vec<ChildTag>,
    pub(crate) child_edge_tag: Vec<ChildTag>,
    pub(crate) child_vertex_tag: Vec<ChildTag>,

    pub(crate) parent_face_tag: Vec<SparseTag>,
    pub(crate) parent_edge_tag: Vec<SparseTag>,
    pub(crate) parent_vertex_tag: Vec<SparseTag>,

    /// `_childValueParentSource` of each face-varying channel (`FVarRefinement`).
    pub(crate) fvar_child_value_parent_source: Vec<Vec<LocalIndex>>,
}

const SPARSE_NEIGHBOR: Index = 1;
const SPARSE_SELECTED: Index = 2;

#[inline]
fn valid(i: Index) -> bool {
    i != INDEX_INVALID
}

impl Refinement {
    pub(crate) fn face_child_faces(&self, f: Index) -> &[Index] {
        let n = self.face_child_co[2 * f as usize] as usize;
        let o = self.face_child_co[2 * f as usize + 1] as usize;
        &self.face_child_face_indices[o..o + n]
    }
    pub(crate) fn face_child_edges(&self, f: Index) -> &[Index] {
        let n = self.face_child_co[2 * f as usize] as usize;
        let o = self.face_child_co[2 * f as usize + 1] as usize;
        &self.face_child_edge_indices[o..o + n]
    }
    pub(crate) fn edge_child_edges(&self, e: Index) -> [Index; 2] {
        [
            self.edge_child_edge_indices[2 * e as usize],
            self.edge_child_edge_indices[2 * e as usize + 1],
        ]
    }
    fn face_child_range(&self, f: Index) -> std::ops::Range<usize> {
        let n = self.face_child_co[2 * f as usize] as usize;
        let o = self.face_child_co[2 * f as usize + 1] as usize;
        o..o + n
    }
    pub(crate) fn is_child_vertex_complete(&self, v: Index) -> bool {
        !self.child_vertex_tag[v as usize].incomplete
    }

    // --- SparseSelector ----------------------------------------------------------------

    fn initialize_selection(&mut self, parent: &Level) {
        if self.parent_vertex_tag.is_empty() {
            self.parent_face_tag = vec![SparseTag::default(); parent.face_count as usize];
            self.parent_edge_tag = vec![SparseTag::default(); parent.edge_count as usize];
            self.parent_vertex_tag = vec![SparseTag::default(); parent.vert_count as usize];
        }
    }

    /// `SparseSelector::selectFace`
    pub(crate) fn select_face(&mut self, parent: &Level, f: Index) {
        self.initialize_selection(parent);
        if !self.parent_face_tag[f as usize].selected {
            self.parent_face_tag[f as usize].selected = true;
            let fe = parent.face_edges(f);
            let fv = parent.face_vertices(f);
            for i in 0..fv.len() {
                self.parent_edge_tag[fe[i] as usize].selected = true;
                self.parent_vertex_tag[fv[i] as usize].selected = true;
            }
        }
    }

    /// `SparseSelector::isSelectionEmpty`
    pub(crate) fn is_selection_empty(&self) -> bool {
        self.parent_vertex_tag.is_empty()
    }

    // --- refine ------------------------------------------------------------------------

    /// `Refinement::refine` (full topology in the child, as `RefineAdaptive` and Blender's
    /// `RefineUniform` with `fullTopologyInLastLevel`-independent face-varying needs use).
    pub(crate) fn refine(
        &mut self,
        parent: &Level,
        child: &mut Level,
        sparse: bool,
        minimal: bool,
    ) {
        child.depth = parent.depth + 1;
        self.uniform = !sparse;
        let refine_fvar = parent.num_fvar_channels() > 0;

        self.populate_parent_to_child_mapping(parent);
        child.face_count = self.child_face_from_face_count;
        child.edge_count = self.child_edge_from_face_count + self.child_edge_from_edge_count;
        child.vert_count = self.child_vert_from_face_count
            + self.child_vert_from_edge_count
            + self.child_vert_from_vert_count;
        self.populate_child_to_parent_mapping(parent, child);
        self.propagate_component_tags(parent, child);

        let all = !minimal;
        self.populate_face_vertex_relation(parent, child);
        if all {
            self.populate_face_edge_relation(parent, child);
            self.populate_edge_vertex_relation(parent, child);
            self.populate_edge_face_relation(parent, child);
        }
        if all || refine_fvar {
            self.populate_vertex_face_relation(parent, child);
        }
        if all {
            self.populate_vertex_edge_relation(parent, child);
        }
        child.max_valence = parent.max_valence.max(4).max(2 + parent.max_edge_faces);

        self.subdivide_edge_sharpness(parent, child);
        self.subdivide_vertex_sharpness(parent, child);
        self.reclassify_semisharp_vertices(parent, child);

        if refine_fvar {
            self.fvar_child_value_parent_source.clear();
            for c in 0..parent.num_fvar_channels() {
                let (fvar, src) = fvar_refinement::apply(self, parent, child, c);
                child.fvar.push(fvar);
                self.fvar_child_value_parent_source.push(src);
            }
        }
    }

    // --- parent to child -----------------------------------------------------------------

    fn populate_parent_to_child_mapping(&mut self, parent: &Level) {
        // QuadRefinement::allocateParentChildIndices
        self.face_child_co = parent.face_vert_co.clone();
        self.face_child_face_indices = vec![0; parent.face_vert_indices.len()];
        self.face_child_edge_indices = vec![0; parent.face_edge_indices.len()];
        self.edge_child_edge_indices = vec![0; parent.edge_vert_indices.len()];
        self.face_child_vert_index = vec![0; parent.face_count as usize];
        self.edge_child_vert_index = vec![0; parent.edge_count as usize];
        self.vert_child_vert_index = vec![0; parent.vert_count as usize];
        if !self.uniform {
            self.mark_sparse_vertex_children(parent);
            self.mark_sparse_edge_children(parent);
            self.mark_sparse_face_children(parent);
        }
        self.populate_parent_child_indices();
    }

    fn populate_parent_child_indices(&mut self) {
        let seq = |v: &mut Vec<Index>, base: i32, sparse: bool| -> i32 {
            let mut n = 0;
            for x in v.iter_mut() {
                if !sparse || *x != 0 {
                    *x = base + n;
                    n += 1;
                } else {
                    *x = INDEX_INVALID;
                }
            }
            n
        };
        let sparse = !self.uniform;
        self.first_child_face_from_face = 0;
        self.child_face_from_face_count = seq(&mut self.face_child_face_indices, 0, sparse);
        self.first_child_edge_from_face = 0;
        self.child_edge_from_face_count = seq(&mut self.face_child_edge_indices, 0, sparse);
        self.first_child_edge_from_edge = self.child_edge_from_face_count;
        self.child_edge_from_edge_count = seq(
            &mut self.edge_child_edge_indices,
            self.first_child_edge_from_edge,
            sparse,
        );
        self.first_child_vert_from_vert = 0;
        self.child_vert_from_vert_count = seq(&mut self.vert_child_vert_index, 0, sparse);
        self.first_child_vert_from_face = self.child_vert_from_vert_count;
        self.child_vert_from_face_count = seq(
            &mut self.face_child_vert_index,
            self.first_child_vert_from_face,
            sparse,
        );
        self.first_child_vert_from_edge =
            self.first_child_vert_from_face + self.child_vert_from_face_count;
        self.child_vert_from_edge_count = seq(
            &mut self.edge_child_vert_index,
            self.first_child_vert_from_edge,
            sparse,
        );
    }

    fn mark_sparse_vertex_children(&mut self, parent: &Level) {
        for v in 0..parent.vert_count as usize {
            if self.parent_vertex_tag[v].selected {
                self.vert_child_vert_index[v] = SPARSE_SELECTED;
            }
        }
    }

    fn mark_sparse_edge_children(&mut self, parent: &Level) {
        for e in 0..parent.edge_count {
            let eu = e as usize;
            let ev = parent.edge_vertices(e);
            if self.parent_edge_tag[eu].selected {
                self.edge_child_edge_indices[2 * eu] = SPARSE_SELECTED;
                self.edge_child_edge_indices[2 * eu + 1] = SPARSE_SELECTED;
                self.edge_child_vert_index[eu] = SPARSE_SELECTED;
            } else {
                if self.parent_vertex_tag[ev[0] as usize].selected {
                    self.edge_child_edge_indices[2 * eu] = SPARSE_NEIGHBOR;
                    self.edge_child_vert_index[eu] = SPARSE_NEIGHBOR;
                }
                if self.parent_vertex_tag[ev[1] as usize].selected {
                    self.edge_child_edge_indices[2 * eu + 1] = SPARSE_NEIGHBOR;
                    self.edge_child_vert_index[eu] = SPARSE_NEIGHBOR;
                }
            }
            let ef = parent.edge_faces(e);
            let t = if ef.len() == 2 {
                self.parent_face_tag[ef[0] as usize].selected
                    != self.parent_face_tag[ef[1] as usize].selected
            } else if ef.len() < 2 {
                false
            } else {
                let s0 = self.parent_face_tag[ef[0] as usize].selected;
                ef[1..]
                    .iter()
                    .any(|&f| self.parent_face_tag[f as usize].selected != s0)
            };
            self.parent_edge_tag[eu].transitional = t as u8;
        }
    }

    fn mark_sparse_face_children(&mut self, parent: &Level) {
        for f in 0..parent.face_count {
            let fu = f as usize;
            let r = self.face_child_range(f);
            let fv = parent.face_vertices(f);
            if self.parent_face_tag[fu].selected {
                for i in 0..fv.len() {
                    self.face_child_face_indices[r.start + i] = SPARSE_SELECTED;
                    self.face_child_edge_indices[r.start + i] = SPARSE_SELECTED;
                }
                self.face_child_vert_index[fu] = SPARSE_SELECTED;
                self.parent_face_tag[fu].transitional = 0;
            } else {
                let mut marked = false;
                for i in 0..fv.len() {
                    if self.parent_vertex_tag[fv[i] as usize].selected {
                        let prev = if i > 0 { i - 1 } else { fv.len() - 1 };
                        self.face_child_face_indices[r.start + i] = SPARSE_NEIGHBOR;
                        self.face_child_edge_indices[r.start + i] = SPARSE_NEIGHBOR;
                        self.face_child_edge_indices[r.start + prev] = SPARSE_NEIGHBOR;
                        marked = true;
                    }
                }
                if marked {
                    self.face_child_vert_index[fu] = SPARSE_NEIGHBOR;
                    let fe = parent.face_edges(f);
                    let tr = |e: Index| self.parent_edge_tag[e as usize].transitional;
                    self.parent_face_tag[fu].transitional = if fe.len() == 4 {
                        tr(fe[0]) | (tr(fe[1]) << 1) | (tr(fe[2]) << 2) | (tr(fe[3]) << 3)
                    } else if fe.len() == 3 {
                        tr(fe[0]) | (tr(fe[1]) << 1) | (tr(fe[2]) << 2)
                    } else {
                        fe.iter().fold(0, |a, &e| a | tr(e))
                    };
                }
            }
        }
    }

    // --- child to parent -----------------------------------------------------------------

    fn populate_child_to_parent_mapping(&mut self, parent: &Level, child: &Level) {
        let tag = |incomplete: bool, i: usize| ChildTag {
            incomplete,
            index_in_parent: i as u8,
        };
        // faces
        self.child_face_tag = vec![ChildTag::default(); child.face_count as usize];
        self.child_face_parent_index = vec![0; child.face_count as usize];
        for f in 0..parent.face_count {
            let incomplete = !self.uniform && !self.parent_face_tag[f as usize].selected;
            let r = self.face_child_range(f);
            let n = r.len();
            let too_large = n > 4;
            for i in 0..n {
                let c = self.face_child_face_indices[r.start + i];
                if valid(c) {
                    self.child_face_tag[c as usize] =
                        tag(incomplete, if too_large { 0 } else { i });
                    self.child_face_parent_index[c as usize] = f;
                }
            }
        }
        // edges
        self.child_edge_tag = vec![ChildTag::default(); child.edge_count as usize];
        self.child_edge_parent_index = vec![0; child.edge_count as usize];
        for f in 0..parent.face_count {
            let incomplete = !self.uniform && !self.parent_face_tag[f as usize].selected;
            let r = self.face_child_range(f);
            let n = r.len();
            let too_large = n > 4;
            for i in 0..n {
                let c = self.face_child_edge_indices[r.start + i];
                if valid(c) {
                    self.child_edge_tag[c as usize] =
                        tag(incomplete, if too_large { 0 } else { i });
                    self.child_edge_parent_index[c as usize] = f;
                }
            }
        }
        for e in 0..parent.edge_count {
            let incomplete = !self.uniform && !self.parent_edge_tag[e as usize].selected;
            let ce = self.edge_child_edges(e);
            for i in 0..2 {
                if valid(ce[i]) {
                    self.child_edge_tag[ce[i] as usize] = tag(incomplete, i);
                    self.child_edge_parent_index[ce[i] as usize] = e;
                }
            }
        }
        // vertices
        self.child_vertex_tag = vec![tag(!self.uniform, 0); child.vert_count as usize];
        self.child_vertex_parent_index = vec![0; child.vert_count as usize];
        for f in 0..parent.face_count {
            let c = self.face_child_vert_index[f as usize];
            if valid(c) {
                if self.uniform || self.parent_face_tag[f as usize].selected {
                    self.child_vertex_tag[c as usize] = tag(false, 0);
                }
                self.child_vertex_parent_index[c as usize] = f;
            }
        }
        for e in 0..parent.edge_count {
            let c = self.edge_child_vert_index[e as usize];
            if valid(c) {
                if self.uniform || self.parent_edge_tag[e as usize].selected {
                    self.child_vertex_tag[c as usize] = tag(false, 0);
                }
                self.child_vertex_parent_index[c as usize] = e;
            }
        }
        for v in 0..parent.vert_count {
            let c = self.vert_child_vert_index[v as usize];
            if valid(c) {
                if self.uniform || self.parent_vertex_tag[v as usize].selected {
                    self.child_vertex_tag[c as usize] = tag(false, 0);
                }
                self.child_vertex_parent_index[c as usize] = v;
            }
        }
    }

    // --- tags ----------------------------------------------------------------------------

    fn propagate_component_tags(&self, parent: &Level, child: &mut Level) {
        child.face_holes = (0..child.face_count as usize)
            .map(|f| parent.face_holes[self.child_face_parent_index[f] as usize])
            .collect();

        child.edge_tags = vec![ETag::default(); child.edge_count as usize];
        let e0 = self.first_child_edge_from_edge as usize;
        for c in e0..e0 + self.child_edge_from_edge_count as usize {
            child.edge_tags[c] = parent.edge_tags[self.child_edge_parent_index[c] as usize];
        }

        child.vert_tags = vec![VTag::default(); child.vert_count as usize];
        // from faces
        let fv0 = self.first_child_vert_from_face as usize;
        for c in fv0..fv0 + self.child_vert_from_face_count as usize {
            let mut t = smooth_vtag();
            if parent.depth == 0 && parent.num_face_vertices(self.child_vertex_parent_index[c]) != 4
            {
                t.set_xordinary(true);
            }
            child.vert_tags[c] = t;
        }
        // from edges
        for e in 0..parent.edge_count {
            let c = self.edge_child_vert_index[e as usize];
            if !valid(c) {
                continue;
            }
            let pt = parent.edge_tags[e as usize];
            let mut t = VTag::default();
            t.set_non_manifold(pt.non_manifold());
            t.set_boundary(pt.boundary());
            t.set_semi_sharp_edges(pt.semi_sharp());
            t.set_inf_sharp_edges(pt.inf_sharp());
            t.set_inf_sharp_crease(pt.inf_sharp());
            t.set_inf_irregular(pt.inf_sharp() && pt.non_manifold());
            t.set_rule(if pt.semi_sharp() || pt.inf_sharp() {
                RULE_CREASE
            } else {
                RULE_SMOOTH
            });
            child.vert_tags[c as usize] = t;
        }
        // from vertices
        let vv0 = self.first_child_vert_from_vert as usize;
        for c in vv0..vv0 + self.child_vert_from_vert_count as usize {
            let mut t = parent.vert_tags[self.child_vertex_parent_index[c] as usize];
            t.set_incid_irreg_face(false);
            child.vert_tags[c] = t;
        }
        if !self.uniform {
            for c in 0..child.vert_count as usize {
                if self.child_vertex_tag[c].incomplete {
                    child.vert_tags[c].set_incomplete(true);
                }
            }
        }
    }

    // --- relations (QuadRefinement) --------------------------------------------------------

    fn populate_face_vertex_relation(&self, parent: &Level, child: &mut Level) {
        let nf = child.face_count as usize;
        child.face_vert_co = (0..nf).flat_map(|i| [4, (i << 2) as Index]).collect();
        child.face_vert_indices = vec![0; nf * 4];
        for f in 0..parent.face_count {
            let pfv = parent.face_vertices(f);
            let pfe = parent.face_edges(f);
            let children = self.face_child_faces(f);
            let n = pfv.len();
            for j in 0..n {
                let c = children[j];
                if !valid(c) {
                    continue;
                }
                let j_prev = if j > 0 { j - 1 } else { n - 1 };
                let v_face = self.face_child_vert_index[f as usize];
                let v_eprev = self.edge_child_vert_index[pfe[j_prev] as usize];
                let v_vert = self.vert_child_vert_index[pfv[j] as usize];
                let v_enext = self.edge_child_vert_index[pfe[j] as usize];
                let o = 4 * c as usize;
                let cfv = &mut child.face_vert_indices[o..o + 4];
                if n == 4 {
                    let j_opp = if j_prev > 0 { j_prev - 1 } else { 3 };
                    let j_next = if j_opp > 0 { j_opp - 1 } else { 3 };
                    cfv[j] = v_vert;
                    cfv[j_next] = v_enext;
                    cfv[j_opp] = v_face;
                    cfv[j_prev] = v_eprev;
                } else {
                    cfv[0] = v_vert;
                    cfv[1] = v_enext;
                    cfv[2] = v_face;
                    cfv[3] = v_eprev;
                }
            }
        }
    }

    fn populate_face_edge_relation(&self, parent: &Level, child: &mut Level) {
        child.face_edge_indices = vec![0; child.face_count as usize * 4];
        for f in 0..parent.face_count {
            let pfv = parent.face_vertices(f);
            let pfe = parent.face_edges(f);
            let cfaces = self.face_child_faces(f);
            let cedges = self.face_child_edges(f);
            let n = pfv.len();
            for j in 0..n {
                let c = cfaces[j];
                if !valid(c) {
                    continue;
                }
                let j_prev = if j > 0 { j - 1 } else { n - 1 };
                let p_prev = pfe[j_prev];
                let pv_prev = parent.edge_vertices(p_prev);
                let p_next = pfe[j];
                let pv_next = parent.edge_vertices(p_next);
                let corner = pfv[j];
                let in_prev = if pv_prev[0] != pv_prev[1] {
                    (pv_prev[0] != corner) as usize
                } else {
                    1
                };
                let in_next = if pv_next[0] != pv_next[1] {
                    (pv_next[0] != corner) as usize
                } else {
                    0
                };
                let e_of_prev = self.edge_child_edges(p_prev)[in_prev];
                let e_of_next = self.edge_child_edges(p_next)[in_next];
                let perp_prev = cedges[j_prev];
                let perp_next = cedges[j];
                let o = 4 * c as usize;
                let cfe = &mut child.face_edge_indices[o..o + 4];
                if n == 4 {
                    let j_opp = if j_prev > 0 { j_prev - 1 } else { 3 };
                    let j_next = if j_opp > 0 { j_opp - 1 } else { 3 };
                    cfe[j] = e_of_next;
                    cfe[j_next] = perp_next;
                    cfe[j_opp] = perp_prev;
                    cfe[j_prev] = e_of_prev;
                } else {
                    cfe[0] = e_of_next;
                    cfe[1] = perp_next;
                    cfe[2] = perp_prev;
                    cfe[3] = e_of_prev;
                }
            }
        }
    }

    fn populate_edge_vertex_relation(&self, parent: &Level, child: &mut Level) {
        child.edge_vert_indices = vec![0; child.edge_count as usize * 2];
        for f in 0..parent.face_count {
            let pfe = parent.face_edges(f);
            let cedges = self.face_child_edges(f);
            for j in 0..pfe.len() {
                let c = cedges[j];
                if valid(c) {
                    child.edge_vert_indices[2 * c as usize] =
                        self.face_child_vert_index[f as usize];
                    child.edge_vert_indices[2 * c as usize + 1] =
                        self.edge_child_vert_index[pfe[j] as usize];
                }
            }
        }
        for e in 0..parent.edge_count {
            let pev = parent.edge_vertices(e);
            let ce = self.edge_child_edges(e);
            for j in 0..2 {
                if valid(ce[j]) {
                    child.edge_vert_indices[2 * ce[j] as usize] =
                        self.edge_child_vert_index[e as usize];
                    child.edge_vert_indices[2 * ce[j] as usize + 1] =
                        self.vert_child_vert_index[pev[j] as usize];
                }
            }
        }
    }

    fn populate_edge_face_relation(&self, parent: &Level, child: &mut Level) {
        let estimate = parent.face_vert_indices.len() * 2 + parent.edge_face_indices.len() * 2;
        child.edge_face_co = vec![0; child.edge_count as usize * 2];
        child.edge_face_indices = vec![0; estimate];
        child.edge_face_local = vec![0; estimate];
        child.max_edge_faces = parent.max_edge_faces;
        // from parent faces
        for f in 0..parent.face_count {
            let cfaces = self.face_child_faces(f);
            let cedges = self.face_child_edges(f);
            let n = cfaces.len();
            for j in 0..n {
                let ce = cedges[j];
                if !valid(ce) {
                    continue;
                }
                child.resize_edge_faces_of(ce, 2);
                let o = child.offset_of_edge_faces(ce) as usize;
                let j_next = if j + 1 < n { j + 1 } else { 0 };
                let mut count = 0;
                if valid(cfaces[j]) {
                    child.edge_face_indices[o + count] = cfaces[j];
                    child.edge_face_local[o + count] =
                        if n == 4 { j_next } else { 1 } as LocalIndex;
                    count += 1;
                }
                if valid(cfaces[j_next]) {
                    child.edge_face_indices[o + count] = cfaces[j_next];
                    child.edge_face_local[o + count] =
                        if n == 4 { (j_next + 2) & 3 } else { 2 } as LocalIndex;
                    count += 1;
                }
                child.trim_edge_faces(ce, count as i32);
            }
        }
        // from parent edges
        for e in 0..parent.edge_count {
            let cedges = self.edge_child_edges(e);
            if !valid(cedges[0]) && !valid(cedges[1]) {
                continue;
            }
            let pef = parent.edge_faces(e);
            let pein = parent.edge_face_local_indices(e);
            let pev = parent.edge_vertices(e);
            for j in 0..2 {
                let ce = cedges[j];
                if !valid(ce) {
                    continue;
                }
                child.resize_edge_faces_of(ce, pef.len() as i32);
                let o = child.offset_of_edge_faces(ce) as usize;
                let mut count = 0;
                for i in 0..pef.len() {
                    let pf = pef[i];
                    let edge_in_face = pein[i] as usize;
                    let pfv = parent.face_vertices(pf);
                    let pchildren = self.face_child_faces(pf);
                    let child_of_edge = if pev[0] == pev[1] {
                        j
                    } else {
                        (pfv[edge_in_face] != pev[j]) as usize
                    };
                    let mut child_in_face = edge_in_face + child_of_edge;
                    if child_in_face == pchildren.len() {
                        child_in_face = 0;
                    }
                    if valid(pchildren[child_in_face]) {
                        child.edge_face_indices[o + count] = pchildren[child_in_face];
                        child.edge_face_local[o + count] = if pfv.len() == 4 {
                            edge_in_face
                        } else if child_of_edge != 0 {
                            3
                        } else {
                            0
                        } as LocalIndex;
                        count += 1;
                    }
                }
                child.trim_edge_faces(ce, count as i32);
            }
        }
        let last = child.edge_count - 1;
        let total = if last >= 0 {
            (child.num_edge_faces(last) + child.offset_of_edge_faces(last)) as usize
        } else {
            0
        };
        child.edge_face_indices.truncate(total);
        child.edge_face_local.truncate(total);
    }

    fn populate_vertex_face_relation(&self, parent: &Level, child: &mut Level) {
        let estimate = parent.face_vert_indices.len()
            + parent.edge_face_indices.len() * 2
            + parent.vert_face_indices.len();
        child.vert_face_co = vec![0; child.vert_count as usize * 2];
        child.vert_face_indices = vec![0; estimate];
        child.vert_face_local = vec![0; estimate];
        // vertices first (child vertices from vertices have the first indices)
        for v in 0..parent.vert_count {
            let cv = self.vert_child_vert_index[v as usize];
            if !valid(cv) {
                continue;
            }
            let pvf = parent.vertex_faces(v);
            let pvin = parent.vertex_face_local_indices(v);
            child.resize_vertex_faces_of(cv, pvf.len() as i32);
            let o = child.offset_of_vertex_faces(cv) as usize;
            let mut count = 0;
            for i in 0..pvf.len() {
                let pf = pvf[i];
                let vin = pvin[i] as usize;
                let pchildren = self.face_child_faces(pf);
                if valid(pchildren[vin]) {
                    child.vert_face_indices[o + count] = pchildren[vin];
                    child.vert_face_local[o + count] =
                        if pchildren.len() == 4 { vin } else { 0 } as LocalIndex;
                    count += 1;
                }
            }
            child.trim_vertex_faces(cv, count as i32);
        }
        for f in 0..parent.face_count {
            let cv = self.face_child_vert_index[f as usize];
            if !valid(cv) {
                continue;
            }
            let pchildren = self.face_child_faces(f);
            let n = pchildren.len();
            child.resize_vertex_faces_of(cv, n as i32);
            let o = child.offset_of_vertex_faces(cv) as usize;
            let mut count = 0;
            for j in 0..n {
                if valid(pchildren[j]) {
                    child.vert_face_indices[o + count] = pchildren[j];
                    child.vert_face_local[o + count] =
                        if n == 4 { (j + 2) & 3 } else { 2 } as LocalIndex;
                    count += 1;
                }
            }
            child.trim_vertex_faces(cv, count as i32);
        }
        for e in 0..parent.edge_count {
            let cv = self.edge_child_vert_index[e as usize];
            if !valid(cv) {
                continue;
            }
            let pef = parent.edge_faces(e);
            let pein = parent.edge_face_local_indices(e);
            child.resize_vertex_faces_of(cv, 2 * pef.len() as i32);
            let o = child.offset_of_vertex_faces(cv) as usize;
            let mut count = 0;
            for i in 0..pef.len() {
                let pf = pef[i];
                let edge_in_face = pein[i] as usize;
                let pchildren = self.face_child_faces(pf);
                let n = pchildren.len();
                let c0 = edge_in_face;
                let mut c1 = edge_in_face + 1;
                if c1 == n {
                    c1 = 0;
                }
                if valid(pchildren[c1]) {
                    child.vert_face_indices[o + count] = pchildren[c1];
                    child.vert_face_local[o + count] = if n == 4 { c0 } else { 3 } as LocalIndex;
                    count += 1;
                }
                if valid(pchildren[c0]) {
                    child.vert_face_indices[o + count] = pchildren[c0];
                    child.vert_face_local[o + count] = if n == 4 { c1 } else { 1 } as LocalIndex;
                    count += 1;
                }
            }
            child.trim_vertex_faces(cv, count as i32);
        }
        let last = child.vert_count - 1;
        let total = if last >= 0 {
            (child.num_vertex_faces(last) + child.offset_of_vertex_faces(last)) as usize
        } else {
            0
        };
        child.vert_face_indices.truncate(total);
        child.vert_face_local.truncate(total);
    }

    fn populate_vertex_edge_relation(&self, parent: &Level, child: &mut Level) {
        let estimate = parent.face_vert_indices.len()
            + parent.edge_face_indices.len()
            + parent.edge_count as usize * 2
            + parent.vert_edge_indices.len();
        child.vert_edge_co = vec![0; child.vert_count as usize * 2];
        child.vert_edge_indices = vec![0; estimate];
        child.vert_edge_local = vec![0; estimate];
        for v in 0..parent.vert_count {
            let cv = self.vert_child_vert_index[v as usize];
            if !valid(cv) {
                continue;
            }
            let pve = parent.vertex_edges(v);
            let pvin = parent.vertex_edge_local_indices(v);
            child.resize_vertex_edges_of(cv, pve.len() as i32);
            let o = child.offset_of_vertex_edges(cv) as usize;
            let mut count = 0;
            for i in 0..pve.len() {
                let ce = self.edge_child_edges(pve[i])[pvin[i] as usize];
                if valid(ce) {
                    child.vert_edge_indices[o + count] = ce;
                    child.vert_edge_local[o + count] = 1;
                    count += 1;
                }
            }
            child.trim_vertex_edges(cv, count as i32);
        }
        for f in 0..parent.face_count {
            let cv = self.face_child_vert_index[f as usize];
            if !valid(cv) {
                continue;
            }
            let pfv = parent.face_vertices(f);
            let cedges = self.face_child_edges(f);
            let n = pfv.len();
            child.resize_vertex_edges_of(cv, n as i32);
            let o = child.offset_of_vertex_edges(cv) as usize;
            let mut count = 0;
            for j in 0..n {
                let lead = if j > 0 { j - 1 } else { n - 1 };
                if valid(cedges[lead]) {
                    child.vert_edge_indices[o + count] = cedges[lead];
                    child.vert_edge_local[o + count] = 0;
                    count += 1;
                }
            }
            child.trim_vertex_edges(cv, count as i32);
        }
        for e in 0..parent.edge_count {
            let cv = self.edge_child_vert_index[e as usize];
            if !valid(cv) {
                continue;
            }
            let pef = parent.edge_faces(e);
            let pein = parent.edge_face_local_indices(e);
            let pev = parent.edge_vertices(e);
            let pce = self.edge_child_edges(e);
            child.resize_vertex_edges_of(cv, pef.len() as i32 + 2);
            let o = child.offset_of_vertex_edges(cv) as usize;
            let mut count = 0;
            for k in 0..2 {
                if valid(pce[k]) {
                    child.vert_edge_indices[o + count] = pce[k];
                    child.vert_edge_local[o + count] = 0;
                    count += 1;
                }
            }
            for i in 0..pef.len() {
                let pf = pef[i];
                let edge_in_face = pein[i] as usize;
                let ce_face = self.face_child_edges(pf)[edge_in_face];
                if valid(ce_face) {
                    child.vert_edge_indices[o + count] = ce_face;
                    child.vert_edge_local[o + count] = 1;
                    count += 1;
                    if i == 0 && count == 3 {
                        if pev[0] != pev[1] && parent.face_vertices(pf)[edge_in_face] == pev[0] {
                            child.vert_edge_indices.swap(o, o + 1);
                            child.vert_edge_local.swap(o, o + 1);
                        }
                        child.vert_edge_indices.swap(o + 1, o + 2);
                        child.vert_edge_local.swap(o + 1, o + 2);
                    }
                }
            }
            child.trim_vertex_edges(cv, count as i32);
        }
        let last = child.vert_count - 1;
        let total = if last >= 0 {
            (child.num_vertex_edges(last) + child.offset_of_vertex_edges(last)) as usize
        } else {
            0
        };
        child.vert_edge_indices.truncate(total);
        child.vert_edge_local.truncate(total);
    }

    // --- sharpness -------------------------------------------------------------------------

    fn subdivide_edge_sharpness(&self, parent: &Level, child: &mut Level) {
        child.edge_sharpness = vec![sdc::SHARPNESS_SMOOTH; child.edge_count as usize];
        let e0 = self.first_child_edge_from_edge as usize;
        for c in e0..e0 + self.child_edge_from_edge_count as usize {
            let t = child.edge_tags[c];
            if t.inf_sharp() {
                child.edge_sharpness[c] = sdc::SHARPNESS_INFINITE;
            } else if t.semi_sharp() {
                let p = parent.edge_sharpness[self.child_edge_parent_index[c] as usize];
                let s = sdc::decrement_sharpness(p);
                child.edge_sharpness[c] = s;
                if !sdc::is_sharp(s) {
                    child.edge_tags[c].set_semi_sharp(false);
                }
            }
        }
    }

    fn subdivide_vertex_sharpness(&self, parent: &Level, child: &mut Level) {
        child.vert_sharpness = vec![sdc::SHARPNESS_SMOOTH; child.vert_count as usize];
        let v0 = self.first_child_vert_from_vert as usize;
        for c in v0..v0 + self.child_vert_from_vert_count as usize {
            let t = child.vert_tags[c];
            if t.inf_sharp() {
                child.vert_sharpness[c] = sdc::SHARPNESS_INFINITE;
            } else if t.semi_sharp() {
                let p = parent.vert_sharpness[self.child_vertex_parent_index[c] as usize];
                let s = sdc::decrement_sharpness(p);
                child.vert_sharpness[c] = s;
                if !sdc::is_sharp(s) {
                    child.vert_tags[c].set_semi_sharp(false);
                }
            }
        }
    }

    fn reclassify_semisharp_vertices(&self, parent: &Level, child: &mut Level) {
        let ve0 = self.first_child_vert_from_edge as usize;
        for c in ve0..ve0 + self.child_vert_from_edge_count as usize {
            if !child.vert_tags[c].semi_sharp_edges() {
                continue;
            }
            let pe = self.child_vertex_parent_index[c];
            let ce = self.edge_child_edges(pe);
            if self.child_vertex_tag[c].incomplete {
                let s = (valid(ce[0]) && child.edge_tags[ce[0] as usize].semi_sharp())
                    || (valid(ce[1]) && child.edge_tags[ce[1] as usize].semi_sharp());
                child.vert_tags[c].set_semi_sharp_edges(s);
                child.vert_tags[c].set_rule(if s { RULE_CREASE } else { RULE_SMOOTH });
            } else {
                let n = child.edge_tags[ce[0] as usize].semi_sharp() as i32
                    + child.edge_tags[ce[1] as usize].semi_sharp() as i32;
                child.vert_tags[c].set_semi_sharp_edges(n > 0);
                child.vert_tags[c].set_rule(sdc::rule_from_count(0.0, n));
            }
        }
        let vv0 = self.first_child_vert_from_vert as usize;
        for c in vv0..vv0 + self.child_vert_from_vert_count as usize {
            let pv = self.child_vertex_parent_index[c];
            let pt = parent.vert_tags[pv as usize];
            if !pt.semi_sharp() && !pt.semi_sharp_edges() {
                continue;
            }
            let ct = child.vert_tags[c];
            let decayed = pt.semi_sharp() && !ct.semi_sharp();
            if pt.semi_sharp_edges() || decayed {
                let mut inf = 0;
                let mut semi = 0;
                if child.num_vertex_edges_total() > 0 {
                    for &e in child.vertex_edges(c as Index) {
                        let t = child.edge_tags[e as usize];
                        inf += t.inf_sharp() as i32;
                        semi += t.semi_sharp() as i32;
                    }
                } else {
                    let pe = parent.vertex_edges(pv);
                    let pin = parent.vertex_edge_local_indices(pv);
                    for i in 0..pe.len() {
                        let e = self.edge_child_edges(pe[i])[pin[i] as usize];
                        let t = child.edge_tags[e as usize];
                        inf += t.inf_sharp() as i32;
                        semi += t.semi_sharp() as i32;
                    }
                }
                let t = &mut child.vert_tags[c];
                t.set_semi_sharp_edges(semi > 0);
                if !t.semi_sharp() && !t.inf_sharp() {
                    t.set_rule(sdc::rule_from_count(0.0, inf + semi));
                }
            }
        }
    }

    /// `getChildFaceParentFace`
    pub(crate) fn child_face_parent_face(&self, f: Index) -> Index {
        self.child_face_parent_index[f as usize]
    }

    /// `getChildFaceInParentFace`
    pub(crate) fn child_face_in_parent_face(&self, f: Index) -> i32 {
        self.child_face_tag[f as usize].index_in_parent as i32
    }
}
