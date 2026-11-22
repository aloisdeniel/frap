// SPDX-License-Identifier: GPL-3.0-or-later
//! `UvElementMap`: the UV corners of a BMesh grouped per vertex into coincident ("unique")
//! UVs and, optionally, sorted into UV islands. Used by `uv.stitch`, `uv.align` (straighten)
//! and `uv.arrange_islands`.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_utils.cc
//! (`BM_uv_element_map_create`, `bm_uv_build_islands`, `bm_uv_edge_select_build_islands`,
//! `seam_connected`, `BM_uv_element_get`, `BM_uv_element_get_head`,
//! `BM_uv_element_map_ensure_head_table`) and blender/source/blender/blenkernel/
//! BKE_mesh_mapping.hh (`UvElement`, `UvElementMap`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Elements live in `storage` and point to each other by index (Blender's pointers into the
//! same array); after island sorting `storage` is the island-sorted buffer, as in Blender.

use frap_bmesh::{EdgeId, ElemFlag, LoopId, SelectMode};
use rustc_hash::FxHashSet;

use crate::select::{UV_SELECT_EDGE, Uv, UvSel};

use super::super::weld::common::{face_index, vert_index};

/// `STD_UV_CONNECT_LIMIT`
pub(crate) const STD_UV_CONNECT_LIMIT: f32 = 0.0001;

/// `INVALID_ISLAND`
pub(crate) const INVALID_ISLAND: u32 = u32::MAX;

/// `UvElement`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct UvElement {
    /// Next element of the same vertex.
    pub next: Option<u32>,
    /// The corner.
    pub l: LoopId,
    /// The corner's index in its face.
    pub loop_of_face_index: u16,
    /// Whether this element is the first of coincident elements.
    pub separate: bool,
    /// General use flag (stitch).
    pub flag: u8,
    /// The island index (with island sorting).
    pub island: u32,
}

/// `UvElementMap`.
#[derive(Clone, Debug, Default)]
pub(crate) struct UvElementMap {
    /// Element storage (island-sorted with `do_islands`).
    pub storage: Vec<UvElement>,
    /// Total number of unique UVs.
    pub total_unique_uvs: usize,
    /// The first element of each vertex, by vertex index.
    pub vertex: Vec<Option<u32>>,
    /// First element of each island in `storage`.
    pub island_indices: Vec<u32>,
    /// Number of elements of each island.
    pub island_total_uvs: Vec<u32>,
    /// Number of unique UVs of each island.
    pub island_total_unique_uvs: Vec<u32>,
    /// Number of islands (0 without island sorting).
    pub total_islands: usize,
    /// Vertex index by vertex slot (`BM_elem_index_get(v)`).
    pub vidx: Vec<u32>,
    /// Face index by face slot (`BM_elem_index_get(f)`).
    pub fidx: Vec<u32>,
}

/// `compare_v2v2`.
#[inline]
fn compare_v2v2(a: Uv, b: Uv, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit
}

/// `BM_face_calc_area_uv_signed`.
fn face_area_uv_signed(s: &UvSel, f: frap_bmesh::FaceId) -> f32 {
    let mut cross = 0.0f32;
    for l in s.face_loops(f) {
        let luv = s.uv(l);
        let luv_next = s.uv(s.next(l));
        cross += (luv_next[0] - luv[0]) * (luv_next[1] + luv[1]);
    }
    cross * 0.5
}

/// `loop_uv_match`.
fn loop_uv_match(s: &UvSel, l: LoopId, luv_a: Uv, luv_b: Uv) -> bool {
    compare_v2v2(luv_a, s.uv(l), STD_UV_CONNECT_LIMIT)
        && compare_v2v2(luv_b, s.uv(s.next(l)), STD_UV_CONNECT_LIMIT)
}

/// `seam_connected_recursive`.
fn seam_connected_recursive(
    s: &UvSel,
    edge: EdgeId,
    luv_anchor: Uv,
    luv_fan: Uv,
    needle: LoopId,
    visited: &mut FxHashSet<EdgeId>,
) -> bool {
    let anchor = s.lv(needle);
    if s.bm.edge(edge).flag.contains(ElemFlag::SEAM) {
        return false;
    }
    if !visited.insert(edge) {
        return false;
    }
    for l in s.bm.edge_loops(edge) {
        if s.lv(l) == anchor {
            if !loop_uv_match(s, l, luv_anchor, luv_fan) {
                continue;
            }
            if l == needle {
                return true;
            }
            let prev = s.prev(l);
            let luv_far = s.uv(prev);
            if seam_connected_recursive(s, s.le(prev), luv_anchor, luv_far, needle, visited) {
                return true;
            }
        } else {
            if !loop_uv_match(s, l, luv_fan, luv_anchor) {
                continue;
            }
            let next = s.next(l);
            if next == needle {
                return true;
            }
            let luv_far = s.uv(s.next(next));
            if seam_connected_recursive(s, s.le(next), luv_anchor, luv_far, needle, visited) {
                return true;
            }
        }
    }
    false
}

/// `seam_connected`.
fn seam_connected(
    s: &UvSel,
    loop_a: LoopId,
    loop_b: LoopId,
    visited: &mut FxHashSet<EdgeId>,
) -> bool {
    visited.clear();
    let luv_anchor = s.uv(loop_a);
    let luv_next_fan = s.uv(s.next(loop_a));
    let mut result =
        seam_connected_recursive(s, s.le(loop_a), luv_anchor, luv_next_fan, loop_b, visited);
    if !result {
        let prev = s.prev(loop_a);
        let luv_prev_fan = s.uv(prev);
        result = seam_connected_recursive(s, s.le(prev), luv_anchor, luv_prev_fan, loop_b, visited);
    }
    result
}

impl UvElementMap {
    /// `BM_uv_element_map_create` (`None` when no corner qualifies).
    pub(crate) fn create(
        s: &UvSel,
        uv_selected: bool,
        use_winding: bool,
        use_seams: bool,
        do_islands: bool,
    ) -> Option<UvElementMap> {
        // In UV sync selection, all UVs (from unhidden geometry) are visible.
        let face_selected = !s.ts.sync;
        let vidx = vert_index(s);
        let fidx = face_index(s);
        let faces = s.faces();
        let skip = |f| {
            s.bm.face(f).flag.contains(ElemFlag::HIDDEN)
                || (face_selected && !s.bm.face(f).flag.contains(ElemFlag::SELECT))
        };

        let mut totuv = 0usize;
        for &f in &faces {
            if skip(f) {
                continue;
            }
            if !uv_selected {
                totuv += s.bm.face(f).len as usize;
            } else {
                totuv += s
                    .face_loops(f)
                    .into_iter()
                    .filter(|&l| s.uv_select_test(l))
                    .count();
            }
        }
        if totuv == 0 {
            return None;
        }

        let mut map = UvElementMap {
            vertex: vec![None; s.bm.verts_num()],
            vidx,
            fidx,
            ..Default::default()
        };
        map.storage.reserve(totuv);
        let mut winding = if use_winding {
            vec![false; s.bm.faces_num()]
        } else {
            Vec::new()
        };
        for (j, &f) in faces.iter().enumerate() {
            if skip(f) {
                continue;
            }
            for (i, l) in s.face_loops(f).into_iter().enumerate() {
                if uv_selected && !s.uv_select_test(l) {
                    continue;
                }
                let vi = map.vidx[s.lv(l).slot()] as usize;
                let idx = map.storage.len() as u32;
                map.storage.push(UvElement {
                    next: map.vertex[vi],
                    l,
                    loop_of_face_index: i as u16,
                    separate: false,
                    flag: 0,
                    island: INVALID_ISLAND,
                });
                map.vertex[vi] = Some(idx);
            }
            if use_winding {
                winding[j] = face_area_uv_signed(s, f) > 0.0;
            }
        }

        let mut seam_visited = FxHashSet::default();
        // For each BMVert, sort associated linked list into unique uvs.
        for ev_index in 0..map.vertex.len() {
            let mut newvlist: Option<u32> = None;
            let mut vlist = map.vertex[ev_index];
            while let Some(v) = vlist {
                // Detach head from unsorted list.
                vlist = map.storage[v as usize].next;
                map.storage[v as usize].next = newvlist;
                newvlist = Some(v);

                let vl = map.storage[v as usize].l;
                let uv = s.uv(vl);
                let uv_vert_sel = s.uv_select_test(vl);

                let mut lastv: Option<u32> = None;
                let mut iterv = vlist;
                while let Some(it) = iterv {
                    let next = map.storage[it as usize].next;
                    let il = map.storage[it as usize].l;
                    let mut connected = compare_v2v2(s.uv(il), uv, STD_UV_CONNECT_LIMIT);
                    if connected {
                        connected = uv_vert_sel == s.uv_select_test(il);
                    }
                    if connected && use_winding {
                        connected = winding[map.fidx[s.lf(il).slot()] as usize]
                            == winding[map.fidx[s.lf(vl).slot()] as usize];
                    }
                    if connected && use_seams {
                        connected = seam_connected(s, il, vl, &mut seam_visited);
                    }
                    if connected {
                        match lastv {
                            Some(lv) => map.storage[lv as usize].next = next,
                            None => vlist = next,
                        }
                        map.storage[it as usize].next = newvlist;
                        newvlist = Some(it);
                    } else {
                        lastv = Some(it);
                    }
                    iterv = next;
                }
                map.total_unique_uvs += 1;
                if let Some(h) = newvlist {
                    map.storage[h as usize].separate = true;
                }
            }
            map.vertex[ev_index] = newvlist;
        }

        if do_islands {
            map.build_islands(s, uv_selected);
        }

        map.total_unique_uvs = map.storage.iter().filter(|e| e.separate).count();
        Some(map)
    }

    /// The vertex index of element `e`.
    #[inline]
    fn ev(&self, s: &UvSel, e: u32) -> usize {
        self.vidx[s.lv(self.storage[e as usize].l).slot()] as usize
    }

    /// `BM_uv_element_get`.
    pub(crate) fn get(&self, s: &UvSel, l: LoopId) -> Option<u32> {
        let mut e = self.vertex[self.vidx[s.lv(l).slot()] as usize];
        while let Some(i) = e {
            if self.storage[i as usize].l == l {
                return Some(i);
            }
            e = self.storage[i as usize].next;
        }
        None
    }

    /// `BM_uv_element_get_head`.
    pub(crate) fn head(&self, s: &UvSel, child: u32) -> Option<u32> {
        self.vertex[self.ev(s, child)]
    }

    /// The elements of the vertex list starting at `first` (`for (; e; e = e->next)`).
    pub(crate) fn iter_from(&self, first: Option<u32>) -> Vec<u32> {
        let mut out = Vec::new();
        let mut e = first;
        while let Some(i) = e {
            out.push(i);
            e = self.storage[i as usize].next;
        }
        out
    }

    /// The elements of island `i` in storage order.
    pub(crate) fn island(&self, i: usize) -> core::ops::Range<usize> {
        let start = self.island_indices[i] as usize;
        start..start + self.island_total_uvs[i] as usize
    }

    /// `BM_uv_element_map_ensure_head_table`.
    fn head_table(&self) -> Vec<u32> {
        let mut head_table = vec![u32::MAX; self.storage.len()];
        for i in 0..self.storage.len() {
            if self.storage[i].separate {
                let head = i as u32;
                let mut e = Some(head);
                while let Some(x) = e {
                    head_table[x as usize] = head;
                    e = self.storage[x as usize].next;
                    if let Some(n) = e
                        && self.storage[n as usize].separate
                    {
                        break;
                    }
                }
            }
        }
        head_table
    }

    /// `bm_uv_assign_island`.
    fn assign_island(
        &mut self,
        element: u32,
        nisland: u32,
        map: &mut [u32],
        islandbuf: &mut [UvElement],
        islandbufsize: usize,
    ) {
        let e = &mut self.storage[element as usize];
        e.island = nisland;
        map[element as usize] = islandbufsize as u32;
        let b = &mut islandbuf[islandbufsize];
        b.l = e.l;
        b.separate = e.separate;
        b.loop_of_face_index = e.loop_of_face_index;
        b.island = e.island;
        b.flag = e.flag;
    }

    /// Assigns `tail` and the rest of its unique UV to the island (the `while (tail)` loops).
    fn assign_unique(
        &mut self,
        mut e: Option<u32>,
        nisland: u32,
        map: &mut [u32],
        islandbuf: &mut [UvElement],
        islandbufsize: &mut usize,
    ) {
        while let Some(x) = e {
            self.assign_island(x, nisland, map, islandbuf, *islandbufsize);
            *islandbufsize += 1;
            e = self.storage[x as usize].next;
            if let Some(n) = e
                && self.storage[n as usize].separate
            {
                break;
            }
        }
    }

    /// `bm_uv_edge_select_build_islands`.
    fn edge_select_build_islands(
        &mut self,
        s: &UvSel,
        islandbuf: &mut [UvElement],
        map: &mut [u32],
        uv_selected: bool,
    ) -> u32 {
        let head_table = self.head_table();
        let total_uvs = self.storage.len();
        let mut nislands = 0u32;
        let mut islandbufsize = 0usize;
        let mut stack: Vec<u32> = Vec::with_capacity(total_uvs);
        for i in 0..total_uvs {
            if self.storage[i].island != INVALID_ISLAND {
                continue;
            }
            stack.push(i as u32);
            self.assign_unique(Some(i as u32), nislands, map, islandbuf, &mut islandbufsize);
            while let Some(top) = stack.pop() {
                let mut element = Some(top);
                while let Some(el) = element {
                    let l = self.storage[el as usize].l;
                    if (!uv_selected || s.edge_select_test(l))
                        && let Some(next) = self.get(s, s.next(l))
                        && self.storage[next as usize].island == INVALID_ISLAND
                    {
                        let tail = head_table[next as usize];
                        stack.push(tail);
                        self.assign_unique(Some(tail), nislands, map, islandbuf, &mut islandbufsize);
                    }
                    if (!uv_selected || s.edge_select_test(s.prev(l)))
                        && let Some(prev) = self.get(s, s.prev(l))
                        && self.storage[prev as usize].island == INVALID_ISLAND
                    {
                        let tail = head_table[prev as usize];
                        stack.push(tail);
                        self.assign_unique(Some(tail), nislands, map, islandbuf, &mut islandbufsize);
                    }
                    element = self.storage[el as usize].next;
                    if let Some(n) = element
                        && self.storage[n as usize].separate
                    {
                        break;
                    }
                }
            }
            nislands += 1;
        }
        nislands
    }

    /// `bm_uv_build_islands`.
    fn build_islands(&mut self, s: &UvSel, uv_selected: bool) {
        let totuv = self.storage.len();
        let mut nislands = 0u32;
        let mut islandbufsize = 0usize;
        let mut map = vec![0u32; totuv];
        let mut islandbuf: Vec<UvElement> = self
            .storage
            .iter()
            .map(|e| UvElement {
                next: None,
                l: e.l,
                loop_of_face_index: 0,
                separate: false,
                flag: 0,
                island: 0,
            })
            .collect();
        let mut island_number = vec![INVALID_ISLAND; s.bm.faces_num()];

        let use_uv_edge_connectivity = if s.ts.sync {
            s.ts.selectmode.contains(SelectMode::EDGE)
        } else {
            s.ts.uv_selectmode & UV_SELECT_EDGE != 0
        };
        if use_uv_edge_connectivity {
            nislands = self.edge_select_build_islands(s, &mut islandbuf, &mut map, uv_selected);
            islandbufsize = totuv;
        }

        let mut stack = Vec::new();
        for i in 0..totuv {
            if self.storage[i].island != INVALID_ISLAND {
                continue;
            }
            self.storage[i].island = nislands;
            let f0 = s.lf(self.storage[i].l);
            stack.clear();
            stack.push(f0);
            island_number[self.fidx[f0.slot()] as usize] = nislands;
            while let Some(efa) = stack.pop() {
                for l in s.face_loops(efa) {
                    if uv_selected && !s.uv_select_test(l) {
                        continue;
                    }
                    let mut initelement = self.vertex[self.vidx[s.lv(l).slot()] as usize];
                    let mut element = initelement;
                    while let Some(el) = element {
                        if self.storage[el as usize].separate {
                            initelement = Some(el);
                        }
                        if s.lf(self.storage[el as usize].l) == efa {
                            // Found the UV corresponding to our face and vertex.
                            self.assign_island(el, nislands, &mut map, &mut islandbuf, islandbufsize);
                            islandbufsize += 1;
                            let mut e2 = initelement;
                            while let Some(x) = e2 {
                                if self.storage[x as usize].separate && Some(x) != initelement {
                                    break;
                                }
                                let fi = self.fidx[s.lf(self.storage[x as usize].l).slot()] as usize;
                                if island_number[fi] == INVALID_ISLAND {
                                    stack.push(s.lf(self.storage[x as usize].l));
                                    island_number[fi] = nislands;
                                }
                                e2 = self.storage[x as usize].next;
                            }
                            break;
                        }
                        element = self.storage[el as usize].next;
                    }
                }
            }
            nislands += 1;
        }

        // Remap.
        for v in self.vertex.iter_mut() {
            if let Some(e) = *v {
                *v = Some(map[e as usize]);
            }
        }
        let n = nislands as usize;
        self.island_indices = vec![0; n];
        self.island_total_uvs = vec![0; n];
        self.island_total_unique_uvs = vec![0; n];
        let mut j = 0usize;
        for i in 0..totuv {
            let next = self.storage[i].next;
            islandbuf[map[i] as usize].next = next.map(|x| map[x as usize]);
            if islandbuf[i].island as usize != j {
                j += 1;
                self.island_indices[j] = i as u32;
            }
            self.island_total_uvs[j] += 1;
            if islandbuf[i].separate {
                self.island_total_unique_uvs[j] += 1;
            }
        }
        self.storage = islandbuf;
        self.total_islands = n;
    }
}
