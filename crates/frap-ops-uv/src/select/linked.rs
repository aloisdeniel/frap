// SPDX-License-Identifier: GPL-3.0-or-later
//! UV connectivity: the UV vertex map (`BM_uv_vert_map_create`), linked selection
//! (`uv_select_linked_multi` without a picked face) and `UVSelectLinkedHelper`.
//!
//! Ported from blender/source/blender/editors/mesh/editmesh_utils.cc
//! (`BM_uv_vert_map_create`, `STD_UV_CONNECT_LIMIT` from `BKE_mesh_mapping.hh`) and
//! blender/source/blender/editors/uvedit/uvedit_select.cc (`uv_select_linked_delimit_check`,
//! `uv_select_linked_multi`, `UVSelectLinkedHelper`), Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::{ElemFlag, FaceId, SelectMode};

use super::UvSel;

/// `STD_UV_CONNECT_LIMIT`.
const STD_UV_CONNECT_LIMIT: f32 = 0.0001;

/// `UVDelimitMode` bits of `uv.select_linked(delimit=…)`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Delimit {
    /// `SEAM`
    pub seam: bool,
    /// `SHARP`
    pub sharp: bool,
    /// `MATERIAL`
    pub material: bool,
}

impl Delimit {
    fn any(self) -> bool {
        self.seam || self.sharp || self.material
    }
}

/// `UvMapVert`.
#[derive(Copy, Clone, Debug)]
struct MapVert {
    next: Option<usize>,
    face_index: usize,
    loop_of_face_index: usize,
    separate: bool,
}

/// `UvVertMap`: per vertex index, the corners using it, grouped by UV (each group starts
/// with a `separate` entry).
pub(crate) struct VertMap {
    vert: Vec<Option<usize>>,
    buf: Vec<MapVert>,
}

/// `compare_v2v2(a, b, limit)`.
fn compare_v2v2(a: [f32; 2], b: [f32; 2], limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit
}

impl VertMap {
    /// `BM_uv_vert_map_create(bm, use_select, respect_hide)` (`None` without a corner).
    pub(crate) fn create(
        s: &UvSel,
        faces: &[FaceId],
        use_select: bool,
        respect_hide: bool,
    ) -> Option<VertMap> {
        let verts = s.bm.vert_ids();
        let mut vidx = vec![usize::MAX; s.bm.verts.capacity()];
        for (i, v) in verts.iter().enumerate() {
            vidx[v.slot()] = i;
        }
        let skip = |f: FaceId| {
            (use_select && !s.fflag(f, ElemFlag::SELECT))
                || (respect_hide && s.fflag(f, ElemFlag::HIDDEN))
        };
        let totuv: usize = faces
            .iter()
            .filter(|&&f| !skip(f))
            .map(|&f| s.bm.face(f).len as usize)
            .sum();
        if totuv == 0 {
            return None;
        }
        let mut vert: Vec<Option<usize>> = vec![None; verts.len()];
        let mut buf = Vec::with_capacity(totuv);
        for (a, &f) in faces.iter().enumerate() {
            if skip(f) {
                continue;
            }
            for (i, l) in s.face_loops(f).into_iter().enumerate() {
                let vi = vidx[s.lv(l).slot()];
                buf.push(MapVert {
                    next: vert[vi],
                    face_index: a,
                    loop_of_face_index: i,
                    separate: false,
                });
                vert[vi] = Some(buf.len() - 1);
            }
        }
        let uv_of = |buf: &[MapVert], k: usize| {
            let f = faces[buf[k].face_index];
            s.uv(s.face_loops(f)[buf[k].loop_of_face_index])
        };
        for slot in vert.iter_mut() {
            let mut newvlist: Option<usize> = None;
            let mut vlist = *slot;
            while let Some(v) = vlist {
                vlist = buf[v].next;
                buf[v].next = newvlist;
                newvlist = Some(v);
                let uv = uv_of(&buf, v);
                let mut lastv: Option<usize> = None;
                let mut iterv = vlist;
                while let Some(it) = iterv {
                    let next = buf[it].next;
                    let uv2 = uv_of(&buf, it);
                    if compare_v2v2(uv2, uv, STD_UV_CONNECT_LIMIT) {
                        match lastv {
                            Some(lv) => buf[lv].next = next,
                            None => vlist = next,
                        }
                        buf[it].next = newvlist;
                        newvlist = Some(it);
                    } else {
                        lastv = Some(it);
                    }
                    iterv = next;
                }
                if let Some(n) = newvlist {
                    buf[n].separate = true;
                }
            }
            *slot = newvlist;
        }
        Some(VertMap { vert, buf })
    }

    /// The faces UV-connected to face `a` at the vertex with index `vi` (`a`'s own UV group).
    fn linked_faces(&self, vi: usize, a: usize, out: &mut Vec<usize>) {
        out.clear();
        let vlist = self.vert[vi];
        let mut startv = vlist;
        let mut iterv = vlist;
        while let Some(it) = iterv {
            if self.buf[it].separate {
                startv = Some(it);
            }
            if self.buf[it].face_index == a {
                break;
            }
            iterv = self.buf[it].next;
        }
        let mut iterv = startv;
        while let Some(it) = iterv {
            if Some(it) != startv && self.buf[it].separate {
                break;
            }
            out.push(self.buf[it].face_index);
            iterv = self.buf[it].next;
        }
    }
}

/// Vertex index by slot (`BM_elem_index_get(v)` after `BM_mesh_elem_index_ensure`).
fn vert_index(s: &UvSel) -> Vec<usize> {
    let mut vidx = vec![usize::MAX; s.bm.verts.capacity()];
    for (i, v) in s.bm.vert_ids().into_iter().enumerate() {
        vidx[v.slot()] = i;
    }
    vidx
}

/// `uv_select_linked_delimit_check`.
fn delimit_check(
    s: &UvSel,
    f_src: FaceId,
    l_vert_step: frap_bmesh::LoopId,
    f_dst: FaceId,
    d: Delimit,
) -> bool {
    if d.material && s.bm.face(f_src).mat_nr != s.bm.face(f_dst).mat_nr {
        return true;
    }
    if d.seam || d.sharp {
        let edges = [s.le(l_vert_step), s.le(s.prev(l_vert_step))];
        let mut has_valid_edge = false;
        for e in edges {
            if !s.bm.edge_in_face(e, f_dst) {
                continue;
            }
            if d.seam && s.eflag(e, ElemFlag::SEAM) {
                continue;
            }
            if d.sharp && !s.eflag(e, ElemFlag::SMOOTH) {
                continue;
            }
            has_valid_edge = true;
            break;
        }
        if !has_valid_edge {
            return true;
        }
    }
    false
}

/// `uv_select_linked_multi` for one object without a picked face (`hit == nullptr`), with
/// `hflag == BM_ELEM_SELECT`.
pub(crate) fn select_linked(
    s: &mut UvSel,
    extend: bool,
    mut deselect: bool,
    toggle: bool,
    select_faces: bool,
    delimit: Delimit,
) {
    let sync = s.ts.sync;
    if sync {
        s.prepare_sync_select();
    } else {
        s.prepare_custom_data();
    }
    let faces = s.faces();
    let Some(vmap) = VertMap::create(s, &faces, !sync, true) else {
        return;
    };
    let vidx = vert_index(s);
    let mut flag = vec![false; faces.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (a, &f) in faces.iter().enumerate() {
        if !s.face_visible(f) {
            continue;
        }
        if select_faces {
            if s.fflag(f, ElemFlag::SELECT) {
                stack.push(a);
                flag[a] = true;
            }
            continue;
        }
        for l in s.face_loops(f) {
            if !s.uv_select_test(l) {
                continue;
            }
            let mut add = true;
            if sync && s.ts.sticky == super::Sticky::Vert && !s.face_select_test(s.lf(l)) {
                for lo in s.bm.vert_loops(s.lv(l)) {
                    if lo != l && !s.share_vert(l, lo) && s.face_select_test(s.lf(lo)) {
                        add = false;
                        break;
                    }
                }
            }
            if add {
                stack.push(a);
                flag[a] = true;
                break;
            }
        }
    }
    let mut linked = Vec::new();
    while let Some(a) = stack.pop() {
        frap_math::budget::check();
        let f = faces[a];
        for l in s.face_loops(f) {
            vmap.linked_faces(vidx[s.lv(l).slot()], a, &mut linked);
            for &fi in &linked {
                if flag[fi] {
                    continue;
                }
                if delimit.any() && delimit_check(s, f, l, faces[fi], delimit) {
                    continue;
                }
                flag[fi] = true;
                stack.push(fi);
            }
        }
    }
    if toggle && !extend && !deselect {
        for (a, &f) in faces.iter().enumerate() {
            if !flag[a] {
                continue;
            }
            let found = if select_faces {
                !s.fflag(f, ElemFlag::HIDDEN) && s.fflag(f, ElemFlag::SELECT)
            } else {
                s.face_loops(f).into_iter().any(|l| s.uv_select_test(l))
            };
            if found && !select_faces {
                deselect = true;
                break;
            }
        }
    }
    let set_selection = |s: &mut UvSel, f: FaceId, value: bool| {
        if select_faces {
            s.bm.face_select_set(f, value);
        } else {
            s.face_select_set(f, value);
        }
    };
    let needs_multi_pass = sync
        && s.ts
            .selectmode
            .intersects(SelectMode::VERT | SelectMode::EDGE)
        && !deselect;
    let deselect_elem = !extend && !deselect && !toggle;
    if !needs_multi_pass {
        for (a, &f) in faces.iter().enumerate() {
            if !flag[a] {
                if deselect_elem {
                    set_selection(s, f, false);
                }
                continue;
            }
            set_selection(s, f, !deselect);
        }
    } else {
        if deselect_elem {
            for (a, &f) in faces.iter().enumerate() {
                if !flag[a] {
                    set_selection(s, f, false);
                }
            }
        }
        for (a, &f) in faces.iter().enumerate() {
            if flag[a] {
                set_selection(s, f, true);
            }
        }
    }
    if sync {
        if s.ts.sync_uvselect_ignore() {
            if deselect {
                s.bm.deselect_flush();
            } else if !select_faces {
                s.bm.select_flush_mode();
            }
        } else if s.sync_valid {
            if deselect {
                s.bm_flush_from_faces_only_deselect();
            } else {
                s.bm_flush_from_faces_only_select();
            }
            s.bm_sync_to_mesh();
        }
    }
}

/// `UVSelectLinkedHelper`: tags the faces UV-linked to the faces added.
pub(crate) struct LinkedHelper {
    faces: Vec<FaceId>,
    vmap: Option<VertMap>,
    vidx: Vec<usize>,
    tagged: Vec<bool>,
    has_face: bool,
}

impl LinkedHelper {
    /// An empty helper (connectivity is built lazily).
    pub(crate) fn new() -> LinkedHelper {
        LinkedHelper {
            faces: Vec::new(),
            vmap: None,
            vidx: Vec::new(),
            tagged: Vec::new(),
            has_face: false,
        }
    }

    /// `face_add`: tags `f` and every face UV-linked to it; `true` for a new island.
    pub(crate) fn face_add(&mut self, s: &UvSel, f: FaceId) -> bool {
        if self.vmap.is_none() {
            self.faces = s.faces();
            self.vmap = VertMap::create(s, &self.faces, !s.ts.sync, true);
            if self.vmap.is_none() {
                return false;
            }
            self.vidx = vert_index(s);
            self.tagged = vec![false; self.faces.len()];
        }
        self.has_face = true;
        let Some(face_index) = self.faces.iter().position(|&x| x == f) else {
            return false;
        };
        if self.tagged[face_index] {
            return false;
        }
        let vmap = self.vmap.as_ref().expect("built above");
        let mut stack = vec![face_index];
        self.tagged[face_index] = true;
        let mut linked = Vec::new();
        while let Some(i) = stack.pop() {
            for l in s.face_loops(self.faces[i]) {
                vmap.linked_faces(self.vidx[s.lv(l).slot()], i, &mut linked);
                for &fi in &linked {
                    if !self.tagged[fi] {
                        self.tagged[fi] = true;
                        stack.push(fi);
                    }
                }
            }
        }
        true
    }

    /// `tag_all`: `BM_ELEM_TAG` on every tagged face.
    pub(crate) fn tag_all(&self, s: &mut UvSel) {
        if !self.has_face {
            return;
        }
        for (a, &f) in self.faces.iter().enumerate() {
            if self.tagged[a] {
                s.set_fflag(f, ElemFlag::TAG, true);
            }
        }
    }
}
