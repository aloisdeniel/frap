// SPDX-License-Identifier: GPL-3.0-or-later
//! The stitch state of one mesh (`StitchState`) and of the operator (`StitchStateContainer`),
//! and their construction (`stitch_init`): unique UVs, UV edges with their boundary flags and
//! links between coincident edges, 2D boundary normals and the selection stack.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_smart_stitch.cc
//! (`stitch_init`, `stitch_uv_edge_generate_linked_edges`, `stitch_calculate_edge_normal`,
//! `stitch_select_uv`, `stitch_select_edge`, `stitch_set_selection_mode`, `uv_edge_get`,
//! `goto_next_island`), Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::{LoopId, SelectMode};

use super::element_map::UvElementMap;
use super::ghash::EdgeHash;
use crate::select::{Uv, UvSel};

/// `STITCH_SELECTED`
pub(crate) const STITCH_SELECTED: u8 = 1;
/// `STITCH_STITCHABLE`
pub(crate) const STITCH_STITCHABLE: u8 = 2;
/// `STITCH_PROCESSED`
pub(crate) const STITCH_PROCESSED: u8 = 4;
/// `STITCH_BOUNDARY`
pub(crate) const STITCH_BOUNDARY: u8 = 8;
/// `STITCH_STITCHABLE_CANDIDATE`
pub(crate) const STITCH_STITCHABLE_CANDIDATE: u8 = 16;

/// `StitchModes`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `STITCH_VERT`
    Vert,
    /// `STITCH_EDGE`
    Edge,
}

/// `UvEdge`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct UvEdge {
    /// Unique UV index of the first end (the smaller).
    pub uv1: u32,
    /// Unique UV index of the second end.
    pub uv2: u32,
    /// `STITCH_*` flags.
    pub flag: u8,
    /// The element whose corner starts this edge.
    pub element: u32,
    /// Next edge with the same vertices.
    pub next: Option<u32>,
    /// First of the coincident edges.
    pub first: Option<u32>,
}

/// `StitchStateContainer` (the operator's settings).
#[derive(Clone, Debug)]
pub(crate) struct Ssc {
    /// `clear_seams`
    pub clear_seams: bool,
    /// `use_limit`
    pub use_limit: bool,
    /// `limit`
    pub limit_dist: f32,
    /// `snap_islands`
    pub snap_islands: bool,
    /// `midpoint_snap`
    pub midpoints: bool,
    /// `mode`
    pub mode: Mode,
    /// `static_island`
    pub static_island: i32,
    /// `active_object_index` (into the initialised states)
    pub active_object_index: i32,
}

/// `StitchState` of one mesh.
#[derive(Clone, Debug)]
pub(crate) struct StitchState {
    /// `aspect[0] / aspect[1]` of the mesh's active material image.
    pub aspect: f32,
    /// The element map (seams, winding, islands; every UV of the visible faces).
    pub map: UvElementMap,
    /// The first element of each unique UV.
    pub uvs: Vec<u32>,
    /// Element index → unique UV index.
    pub umap: Vec<u32>,
    /// 2D boundary normals by unique UV.
    pub normals: Vec<Uv>,
    /// The UV edges.
    pub edges: Vec<UvEdge>,
    /// `(uv1, uv2)` → edge index.
    pub edge_hash: EdgeHash,
    /// Islands the "next island" key stops at.
    pub island_is_stitchable: Vec<bool>,
    /// The selection: element indices (vertex mode) or edge indices (edge mode).
    pub selection: Vec<u32>,
    /// `bm->uv_select_sync_valid` after `uvedit_select_prepare`.
    pub sync_valid: bool,
}

/// `UvElementID` of the `selection` argument: `(face_index, element_index)`.
pub(crate) type UvElementId = (i32, i32);

/// `stitch_calculate_edge_normal`.
fn edge_normal(s: &UvSel, map: &UvElementMap, edge: &UvEdge, aspect: f32) -> Uv {
    let l1 = map.storage[edge.element as usize].l;
    let luv1 = s.uv(l1);
    let luv2 = s.uv(s.next(l1));
    let mut tangent = [luv2[0] - luv1[0], luv2[1] - luv1[1]];
    tangent[1] /= aspect;
    let (n, _) = frap_uv::param::bli::normalize2([tangent[1], -tangent[0]]);
    n
}

impl StitchState {
    /// `stitch_init` (`None` when no UV is visible).
    pub(crate) fn init(
        s: &mut UvSel,
        ssc: &Ssc,
        aspect: f32,
        stored_mode: Mode,
        to_select: &[UvElementId],
    ) -> Option<StitchState> {
        s.prepare();
        // Sync selection in face mode would detach every selected face (#86924): the element
        // map is built in vertex select mode.
        let selectmode_orig = s.ts.selectmode;
        s.ts.selectmode = SelectMode::VERT;
        let map = UvElementMap::create(s, false, true, true, true);
        s.ts.selectmode = selectmode_orig;
        let map = map?;

        let unique_uvs = map.total_unique_uvs;
        let mut uvs = vec![0u32; unique_uvs];
        let mut umap = vec![0u32; map.storage.len()];
        let mut counter: i64 = -1;
        for vi in 0..map.vertex.len() {
            for e in map.iter_from(map.vertex[vi]) {
                if map.storage[e as usize].separate {
                    counter += 1;
                    uvs[counter as usize] = e;
                }
                umap[e as usize] = counter as u32;
            }
        }

        // UV connectivity: one edge per face corner, coincident ones hashed together.
        let face_selected = !s.ts.sync;
        let mut all_edges: Vec<UvEdge> = Vec::new();
        let mut hash = EdgeHash::new();
        for f in s.faces() {
            let fl = s.bm.face(f).flag;
            if fl.contains(frap_bmesh::ElemFlag::HIDDEN)
                || (face_selected && !fl.contains(frap_bmesh::ElemFlag::SELECT))
            {
                continue;
            }
            for l in s.face_loops(f) {
                let element = map.get(s, l).expect("visible corner has an element");
                let element2 = map
                    .get(s, s.next(l))
                    .expect("visible corner has an element");
                let offset1 = umap[element as usize];
                let offset2 = umap[element2 as usize];
                let (uv1, uv2) = if offset1 < offset2 {
                    (offset1, offset2)
                } else {
                    (offset2, offset1)
                };
                let idx = all_edges.len() as u32;
                let mut edge = UvEdge {
                    uv1,
                    uv2,
                    flag: 0,
                    element,
                    next: None,
                    first: None,
                };
                match hash.get((uv1, uv2)) {
                    Some(existing) => all_edges[existing as usize].flag = 0,
                    None => {
                        hash.insert((uv1, uv2), idx);
                        edge.flag = STITCH_BOUNDARY;
                    }
                }
                all_edges.push(edge);
            }
        }
        let edges: Vec<UvEdge> = hash
            .values()
            .into_iter()
            .map(|i| all_edges[i as usize])
            .collect();
        let mut edge_hash = EdgeHash::new();
        for (i, e) in edges.iter().enumerate() {
            edge_hash.insert((e.uv1, e.uv2), i as u32);
        }

        let mut st = StitchState {
            aspect,
            map,
            uvs,
            umap,
            normals: vec![[0.0; 2]; unique_uvs],
            edges,
            edge_hash,
            island_is_stitchable: Vec::new(),
            selection: Vec::new(),
            sync_valid: s.sync_valid,
        };
        st.generate_linked_edges(s);

        // 2D normals of the boundary UVs.
        for i in 0..st.edges.len() {
            let edge = st.edges[i];
            if edge.flag & STITCH_BOUNDARY != 0 {
                let normal = edge_normal(s, &st.map, &edge, aspect);
                for u in [edge.uv1, edge.uv2] {
                    let n = &mut st.normals[u as usize];
                    n[0] += normal[0];
                    n[1] += normal[1];
                    *n = frap_uv::param::bli::normalize2(*n).0;
                }
            }
        }

        // The selection stack.
        if !to_select.is_empty() {
            let faces = s.faces();
            for &(face_index, element_index) in to_select.iter().rev() {
                let Some(&f) = usize::try_from(face_index).ok().and_then(|i| faces.get(i)) else {
                    continue;
                };
                let loops = s.face_loops(f);
                let Some(&l) = usize::try_from(element_index)
                    .ok()
                    .and_then(|i| loops.get(i))
                else {
                    continue;
                };
                if stored_mode == Mode::Vert {
                    if let Some(e) = st.map.get(s, l) {
                        st.select_uv(s, e, true);
                    }
                } else {
                    let l2 = loops[(element_index as usize + 1) % loops.len()];
                    if let (Some(e1), Some(e2)) = (st.map.get(s, l), st.map.get(s, l2)) {
                        let (a, b) = (st.umap[e1 as usize], st.umap[e2 as usize]);
                        if let Some(edge) = st.edge_hash.get((a.min(b), a.max(b))) {
                            st.select_edge(edge, true);
                        }
                    }
                }
            }
            if ssc.mode != stored_mode {
                st.set_selection_mode(s, stored_mode);
            }
        } else if ssc.mode == Mode::Vert {
            for f in s.faces() {
                for l in s.face_loops(f) {
                    if s.uv_select_test(l)
                        && let Some(e) = st.map.get(s, l)
                    {
                        st.select_uv(s, e, true);
                    }
                }
            }
        } else {
            for f in s.faces() {
                let fl = s.bm.face(f).flag;
                if !s.ts.sync
                    && (fl.contains(frap_bmesh::ElemFlag::HIDDEN)
                        || !fl.contains(frap_bmesh::ElemFlag::SELECT))
                {
                    continue;
                }
                for l in s.face_loops(f) {
                    if s.edge_select_test(l)
                        && let Some(edge) = st.uv_edge_get(s, l)
                    {
                        st.select_edge(edge, true);
                    }
                }
            }
        }
        st.island_is_stitchable = vec![false; st.map.total_islands];
        Some(st)
    }

    /// `stitch_uv_edge_generate_linked_edges`.
    fn generate_linked_edges(&mut self, s: &UvSel) {
        for i in 0..self.edges.len() {
            if self.edges[i].first.is_some() {
                continue;
            }
            if self.edges[i].flag & STITCH_BOUNDARY == 0 {
                self.edges[i].first = Some(i as u32);
                continue;
            }
            let element1 = self.uvs[self.edges[i].uv1 as usize];
            let element2 = self.uvs[self.edges[i].uv2 as usize];
            let elemindex2 = self.map.vidx[s.lv(self.map.storage[element2 as usize].l).slot()];
            let mut last_set = i as u32;
            self.edges[i].first = Some(i as u32);
            for iter1 in self.map.iter_from(self.map.head(s, element1)) {
                let l1 = self.map.storage[iter1 as usize].l;
                let iter2 = if self.map.vidx[s.lv(s.next(l1)).slot()] == elemindex2 {
                    self.map.get(s, s.next(l1))
                } else if self.map.vidx[s.lv(s.prev(l1)).slot()] == elemindex2 {
                    self.map.get(s, s.prev(l1))
                } else {
                    None
                };
                let Some(iter2) = iter2 else { continue };
                let (a, b) = (self.umap[iter1 as usize], self.umap[iter2 as usize]);
                let Some(edge2) = self.edge_hash.get((a.min(b), a.max(b))) else {
                    continue;
                };
                // Non-manifold case: skip edges already in the chain.
                let mut valid = true;
                let mut eiter = Some(i as u32);
                while let Some(e) = eiter {
                    if e == edge2 {
                        valid = false;
                        break;
                    }
                    eiter = self.edges[e as usize].next;
                }
                if valid {
                    self.edges[last_set as usize].next = Some(edge2);
                    last_set = edge2;
                    self.edges[edge2 as usize].first = Some(i as u32);
                }
            }
        }
    }

    /// `uv_edge_get`.
    fn uv_edge_get(&self, s: &UvSel, l: LoopId) -> Option<u32> {
        let e1 = self.map.get(s, l)?;
        let e2 = self.map.get(s, s.next(l))?;
        let (a, b) = (self.umap[e1 as usize], self.umap[e2 as usize]);
        self.edge_hash.get((a.min(b), a.max(b)))
    }

    /// The edges of the coincident chain starting at `edge.first`.
    pub(crate) fn edge_chain(&self, edge: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut e = self.edges[edge as usize].first;
        while let Some(i) = e {
            out.push(i);
            e = self.edges[i as usize].next;
        }
        out
    }

    /// `stitch_select_edge`.
    fn select_edge(&mut self, edge: u32, always_select: bool) {
        for eiter in self.edge_chain(edge) {
            let flag = &mut self.edges[eiter as usize].flag;
            if *flag & STITCH_SELECTED != 0 {
                if always_select {
                    continue;
                }
                *flag &= !STITCH_SELECTED;
                if let Some(i) = self.selection.iter().position(|&x| x == eiter) {
                    self.selection.swap_remove(i);
                }
            } else {
                *flag |= STITCH_SELECTED;
                self.selection.push(eiter);
            }
        }
    }

    /// `stitch_select_uv`.
    fn select_uv(&mut self, s: &UvSel, element: u32, always_select: bool) {
        for it in self.map.iter_from(self.map.head(s, element)) {
            if !self.map.storage[it as usize].separate {
                continue;
            }
            let flag = &mut self.map.storage[it as usize].flag;
            if *flag & STITCH_SELECTED != 0 {
                if always_select {
                    continue;
                }
                *flag &= !STITCH_SELECTED;
                if let Some(i) = self.selection.iter().position(|&x| x == it) {
                    self.selection.swap_remove(i);
                }
            } else {
                *flag |= STITCH_SELECTED;
                self.selection.push(it);
            }
        }
    }

    /// `stitch_set_selection_mode`: converts the selection stored in `from` mode.
    fn set_selection_mode(&mut self, s: &UvSel, from: Mode) {
        let old = std::mem::take(&mut self.selection);
        if from == Mode::Vert {
            for i in 0..self.edges.len() {
                let e = self.edges[i];
                let sel1 = self.map.storage[self.uvs[e.uv1 as usize] as usize].flag;
                let sel2 = self.map.storage[self.uvs[e.uv2 as usize] as usize].flag;
                if sel1 & STITCH_SELECTED != 0 && sel2 & STITCH_SELECTED != 0 {
                    self.select_edge(i as u32, true);
                }
            }
            for el in old {
                self.map.storage[el as usize].flag &= !STITCH_SELECTED;
            }
        } else {
            for edge in old {
                let e = self.edges[edge as usize];
                self.select_uv(s, self.uvs[e.uv1 as usize], true);
                self.select_uv(s, self.uvs[e.uv2 as usize], true);
                self.edges[edge as usize].flag &= !STITCH_SELECTED;
            }
        }
    }
}

/// `goto_next_island`: the next stitchable island (and object), `false` when none.
pub(crate) fn goto_next_island(ssc: &mut Ssc, states: &[&StitchState]) -> bool {
    let original_active = ssc.active_object_index;
    let original_island = ssc.static_island;
    loop {
        ssc.static_island += 1;
        if ssc.static_island as usize >= states[ssc.active_object_index as usize].map.total_islands
        {
            ssc.active_object_index = (ssc.active_object_index + 1) % states.len() as i32;
            ssc.static_island = 0;
        }
        let active = &states[ssc.active_object_index as usize];
        if active.island_is_stitchable[ssc.static_island as usize] {
            return true;
        }
        if ssc.active_object_index == original_active && ssc.static_island == original_island {
            return false;
        }
    }
}
