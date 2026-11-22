// SPDX-License-Identifier: GPL-3.0-or-later
//! The UV selection API of the `uv.*` operators (WP 8.3.8; WP 8.3.9's editing operators use
//! it too): the 5.x UV selection flags of the edit BMesh, the tool settings that steer them,
//! and the selection tests, setters and flushes of the UV editor.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_select.cc (the "Visibility and
//! Selection Utilities", "Low Level Selection API", "Helper functions", "UV Selection Non-Sync
//! API", "Select-Mode Flushing", "Flush selection", "(De)Select All", "Select/Tag Flushing
//! Utils", "Selected Elements as Arrays" and "Select Mode" sections) and
//! blender/source/blender/bmesh/intern/bmesh_uvselect.cc (in [`bm`]), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! - **Flags.** A loop carries `BM_ELEM_SELECT_UV` (its UV vertex, [`ElemFlag::SELECT_UV`]) and
//!   `BM_ELEM_SELECT_UV_EDGE` (the UV edge to the next corner, [`ElemFlag::SELECT_UV_EDGE`]); a
//!   face carries `BM_ELEM_SELECT_UV`. They live on the edit BMesh and reach the mesh as
//!   `.uv_select_vert`, `.uv_select_edge` and `.uv_select_face` (`frap_bmesh::convert`).
//! - **Tool settings** ([`Ts`]): `use_uv_select_sync`, `mesh_select_mode`, `uv_select_mode`,
//!   `uv_sticky_select_mode` and `use_uv_select_island`.
//! - **Sync selection.** With `use_uv_select_sync` the UV selection either follows the mesh
//!   selection (`bm->uv_select_sync_valid == false`: the tests read the vertex/edge/face
//!   flags) or is its own partial selection kept in step with the mesh
//!   (`uv_select_sync_valid`, [`UvSel::sync_valid`]). frap keeps that validity for the
//!   duration of one operator only (deviation D-111): each operator starts from the mesh
//!   selection, as Blender does after any mesh selection change, and the mesh selection it
//!   leaves is Blender's.

pub(crate) mod bm;
pub(crate) mod hooks;
pub(crate) mod kdtree;
pub(crate) mod linked;
pub(crate) mod mesh;
pub(crate) mod op;
pub(crate) mod path;

use frap_bmesh::{BMesh, CdLayer, EdgeId, ElemFlag, FaceId, LoopId, SelectMode, VertId};
use frap_mesh::AttrData;
use frap_scene::Context;
use frap_scene::generated::plain::ToolSettings;

/// A UV coordinate.
pub(crate) type Uv = [f32; 2];

/// `UV_SELECT_VERT` (`ts->uv_selectmode`).
pub(crate) const UV_SELECT_VERT: u8 = 1;
/// `UV_SELECT_EDGE`
pub(crate) const UV_SELECT_EDGE: u8 = 2;
/// `UV_SELECT_FACE`
pub(crate) const UV_SELECT_FACE: u8 = 4;

/// `ts->uv_sticky` (`uv_sticky_select_mode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Sticky {
    /// `UV_STICKY_LOCATION` (`SHARED_LOCATION`, the factory setting).
    Location,
    /// `UV_STICKY_DISABLE` (`DISABLED`).
    Disable,
    /// `UV_STICKY_VERT` (`SHARED_VERTEX`).
    Vert,
}

/// The tool settings the UV selection reads.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Ts {
    /// `uv_flag & UV_FLAG_SELECT_SYNC` (`use_uv_select_sync`).
    pub sync: bool,
    /// `ts->selectmode` (`mesh_select_mode`).
    pub selectmode: SelectMode,
    /// `ts->uv_selectmode` (`uv_select_mode`: [`UV_SELECT_VERT`], …).
    pub uv_selectmode: u8,
    /// `ts->uv_sticky`.
    pub sticky: Sticky,
    /// `uv_flag & UV_FLAG_SELECT_ISLAND` (`use_uv_select_island`).
    pub island: bool,
}

impl Ts {
    /// The context scene's tool settings.
    pub(crate) fn of(c: &Context) -> Ts {
        Ts::from_settings(c.tool_settings())
    }

    /// From stored tool settings.
    pub(crate) fn from_settings(ts: &ToolSettings) -> Ts {
        let mut m = SelectMode::empty();
        for (on, bit) in
            ts.mesh_select_mode
                .iter()
                .zip([SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE])
        {
            if *on {
                m |= bit;
            }
        }
        Ts {
            sync: ts.use_uv_select_sync,
            selectmode: m,
            uv_selectmode: uv_selectmode_of(&ts.uv_select_mode),
            sticky: match ts.uv_sticky_select_mode.as_str() {
                "DISABLED" => Sticky::Disable,
                "SHARED_VERTEX" => Sticky::Vert,
                _ => Sticky::Location,
            },
            island: ts.use_uv_select_island,
        }
    }

    /// `ED_uvedit_sync_uvselect_ignore`: with sync selection and `SHARED_VERTEX`, the mesh
    /// selection is used as is.
    pub(crate) fn sync_uvselect_ignore(&self) -> bool {
        self.sticky == Sticky::Vert
    }

    /// `ED_uvedit_select_mode_get`.
    pub(crate) fn select_mode_get(&self) -> u8 {
        if self.sync {
            if self.selectmode.contains(SelectMode::VERT) {
                UV_SELECT_VERT
            } else if self.selectmode.contains(SelectMode::EDGE) {
                UV_SELECT_EDGE
            } else if self.selectmode.contains(SelectMode::FACE) {
                UV_SELECT_FACE
            } else {
                UV_SELECT_VERT
            }
        } else if self.uv_selectmode & UV_SELECT_VERT != 0 {
            UV_SELECT_VERT
        } else if self.uv_selectmode & UV_SELECT_EDGE != 0 {
            UV_SELECT_EDGE
        } else if self.uv_selectmode & UV_SELECT_FACE != 0 {
            UV_SELECT_FACE
        } else {
            UV_SELECT_VERT
        }
    }

    /// `ED_uvedit_select_island_check` (not strict).
    pub(crate) fn select_island_check(&self) -> bool {
        self.island
    }

    /// `ts->selectmode` as Blender's bits (`SCE_SELECT_VERTEX` 1, `EDGE` 2, `FACE` 4).
    pub(crate) fn selectmode_bits(&self) -> u8 {
        mode_bits(self.selectmode)
    }
}

/// The `uv_select_mode` enum value (`VERTEX` 1, `EDGE` 2, `FACE` 4).
pub(crate) fn uv_selectmode_of(id: &str) -> u8 {
    match id {
        "EDGE" => UV_SELECT_EDGE,
        "FACE" => UV_SELECT_FACE,
        _ => UV_SELECT_VERT,
    }
}

/// A [`SelectMode`] as Blender's bits.
pub(crate) fn mode_bits(m: SelectMode) -> u8 {
    let mut b = 0;
    if m.contains(SelectMode::VERT) {
        b |= 1;
    }
    if m.contains(SelectMode::EDGE) {
        b |= 2;
    }
    if m.contains(SelectMode::FACE) {
        b |= 4;
    }
    b
}

/// The UV selection of one edit BMesh: its flags, the active UV map's coordinates and the
/// operator-local `uv_select_sync_valid`.
pub(crate) struct UvSel<'a> {
    /// Tool settings.
    pub ts: Ts,
    /// The edit mesh.
    pub bm: &'a mut BMesh,
    /// The active UV map's column (by loop slot).
    pub uv: Vec<Uv>,
    /// `bm->uv_select_sync_valid` (D-111: starts false in every operator).
    pub sync_valid: bool,
}

impl<'a> UvSel<'a> {
    /// The UV selection of `bm` (`None` without a UV map: `BM_uv_map_offsets_get` fails).
    pub(crate) fn new(ts: Ts, bm: &'a mut BMesh) -> Option<UvSel<'a>> {
        let layer = active_uv(bm)?;
        let uv = read_uvs(bm, layer);
        Some(UvSel {
            ts,
            bm,
            uv,
            sync_valid: false,
        })
    }

    /// The UV selection of `bm` read with an explicit UV column (tests).
    #[cfg(test)]
    pub(crate) fn with_uvs(ts: Ts, bm: &'a mut BMesh, uv: Vec<Uv>) -> UvSel<'a> {
        UvSel {
            ts,
            bm,
            uv,
            sync_valid: false,
        }
    }

    // ------------------------------------------------------------------------- raw flags

    pub(crate) fn lflag(&self, l: LoopId, flag: ElemFlag) -> bool {
        self.bm.lp(l).flag.contains(flag)
    }
    pub(crate) fn set_lflag(&mut self, l: LoopId, flag: ElemFlag, on: bool) {
        self.bm.lp_mut(l).flag.set(flag, on);
    }
    pub(crate) fn fflag(&self, f: FaceId, flag: ElemFlag) -> bool {
        self.bm.face(f).flag.contains(flag)
    }
    pub(crate) fn set_fflag(&mut self, f: FaceId, flag: ElemFlag, on: bool) {
        self.bm.face_mut(f).flag.set(flag, on);
    }
    pub(crate) fn vflag(&self, v: VertId, flag: ElemFlag) -> bool {
        self.bm.vert(v).flag.contains(flag)
    }
    pub(crate) fn eflag(&self, e: EdgeId, flag: ElemFlag) -> bool {
        self.bm.edge(e).flag.contains(flag)
    }
    /// The face of loop `l`.
    pub(crate) fn lf(&self, l: LoopId) -> FaceId {
        self.bm.lp(l).f
    }
    /// The vertex of loop `l`.
    pub(crate) fn lv(&self, l: LoopId) -> VertId {
        self.bm.lp(l).v
    }
    /// The edge of loop `l`.
    pub(crate) fn le(&self, l: LoopId) -> EdgeId {
        self.bm.lp(l).e
    }
    pub(crate) fn next(&self, l: LoopId) -> LoopId {
        self.bm.lp(l).next
    }
    pub(crate) fn prev(&self, l: LoopId) -> LoopId {
        self.bm.lp(l).prev
    }
    pub(crate) fn radial_next(&self, l: LoopId) -> LoopId {
        self.bm.lp(l).radial_next
    }
    /// The UV of loop `l`.
    pub(crate) fn uv(&self, l: LoopId) -> Uv {
        self.uv.get(l.slot()).copied().unwrap_or([0.0; 2])
    }
    /// `BM_FACES_OF_MESH`.
    pub(crate) fn faces(&self) -> Vec<FaceId> {
        self.bm.face_ids()
    }
    /// `BM_LOOPS_OF_FACE`.
    pub(crate) fn face_loops(&self, f: FaceId) -> Vec<LoopId> {
        self.bm.face_loops(f)
    }
    /// `BM_FACE_FIRST_LOOP`.
    pub(crate) fn first_loop(&self, f: FaceId) -> LoopId {
        self.bm.face(f).l_first
    }

    /// The loops using `v` in disk-then-radial order (`e = v->e` around the disk, the radial
    /// loops of each edge whose vertex is `v`): the walk of the `*_check_other_*` and
    /// `*_shared_*` functions.
    pub(crate) fn disk_radial_loops(&self, v: VertId) -> Vec<LoopId> {
        let mut out = Vec::new();
        for e in self.bm.vert_edges(v) {
            let Some(first) = self.bm.edge(e).l else {
                continue;
            };
            let mut l = first;
            loop {
                if self.lv(l) == v {
                    out.push(l);
                }
                l = self.radial_next(l);
                if l == first {
                    break;
                }
            }
        }
        out
    }

    /// The radial cycle of `l`'s edge starting at `l`.
    pub(crate) fn radial_from(&self, l: LoopId) -> Vec<LoopId> {
        let mut out = vec![l];
        let mut r = self.radial_next(l);
        while r != l {
            out.push(r);
            r = self.radial_next(r);
        }
        out
    }

    /// `BM_loop_uv_share_vert_check`.
    pub(crate) fn share_vert(&self, la: LoopId, lb: LoopId) -> bool {
        self.uv(la) == self.uv(lb)
    }

    /// `BM_loop_uv_share_edge_check`.
    pub(crate) fn share_edge(&self, la: LoopId, lb: LoopId) -> bool {
        let (mut b_curr, mut b_next) = (self.uv(lb), self.uv(self.next(lb)));
        if self.lv(la) != self.lv(lb) {
            core::mem::swap(&mut b_curr, &mut b_next);
        }
        self.uv(la) == b_curr && self.uv(self.next(la)) == b_next
    }

    // ------------------------------------------------------------------- sync validity

    /// `uvedit_sync_uvselect_flush_from_v3d`.
    pub(crate) fn sync_uvselect_flush_from_v3d(&mut self) {
        match self.ts.sticky {
            Sticky::Location => self.bm_sync_from_mesh_sticky_location(),
            Sticky::Disable => self.bm_sync_from_mesh_sticky_disabled(),
            Sticky::Vert => self.bm_sync_from_mesh_sticky_vert(),
        }
    }

    /// `ED_uvedit_sync_uvselect_ensure_if_needed`.
    pub(crate) fn sync_uvselect_ensure_if_needed(&mut self) {
        if self.ts.sync_uvselect_ignore() {
            self.sync_valid = false;
            return;
        }
        if self.sync_valid {
            return;
        }
        self.sync_uvselect_flush_from_v3d();
    }

    /// `ED_uvedit_sync_uvselect_is_valid_or_ignore`.
    pub(crate) fn sync_uvselect_is_valid_or_ignore(&self) -> bool {
        self.sync_valid || self.ts.sync_uvselect_ignore()
    }

    /// `uvedit_select_prepare_custom_data` (`BM_mesh_uvselect_clear`).
    pub(crate) fn prepare_custom_data(&mut self) {
        self.sync_valid = false;
    }

    /// `uvedit_select_prepare_sync_select`.
    pub(crate) fn prepare_sync_select(&mut self) {
        self.sync_uvselect_ensure_if_needed();
    }

    /// `uvedit_select_prepare`.
    pub(crate) fn prepare(&mut self) {
        if self.ts.sync {
            self.prepare_sync_select();
        } else {
            self.prepare_custom_data();
        }
    }

    /// `ED_uvedit_select_sync_flush`.
    pub(crate) fn select_sync_flush(&mut self, select: bool) {
        if !self.ts.sync {
            return;
        }
        if self.sync_valid {
            self.bm_mode_flush();
            if self.ts.sticky == Sticky::Location {
                self.bm_flush_shared_only_select();
            }
            self.bm_sync_to_mesh();
        } else if self.ts.selectmode != SelectMode::FACE {
            if !select {
                self.bm.deselect_flush();
            } else if self.ts.selectmode.contains(SelectMode::VERT) {
                self.bm.select_flush();
            } else {
                self.bm.select_flush_mode();
            }
        }
        if !select {
            self.bm.select_history_validate();
        }
    }

    // ---------------------------------------------------------------------- face tests

    /// `uvedit_face_visible_test_ex`.
    pub(crate) fn face_visible(&self, f: FaceId) -> bool {
        if self.ts.sync {
            return !self.fflag(f, ElemFlag::HIDDEN);
        }
        !self.fflag(f, ElemFlag::HIDDEN) && self.fflag(f, ElemFlag::SELECT)
    }

    /// `uvedit_face_select_test_ex`.
    pub(crate) fn face_select_test(&self, f: FaceId) -> bool {
        if self.ts.sync {
            if !self.sync_valid || self.ts.sync_uvselect_ignore() {
                return self.fflag(f, ElemFlag::SELECT);
            }
            return self.fflag(f, ElemFlag::SELECT_UV);
        }
        if self.ts.uv_selectmode == UV_SELECT_FACE {
            return self.fflag(f, ElemFlag::SELECT_UV);
        }
        let test = if self.ts.uv_selectmode & UV_SELECT_VERT != 0 {
            ElemFlag::SELECT_UV
        } else {
            ElemFlag::SELECT_UV_EDGE
        };
        self.face_loops(f).into_iter().all(|l| self.lflag(l, test))
    }

    /// `uvedit_face_select_set_with_sticky`.
    pub(crate) fn face_select_set_with_sticky(&mut self, f: FaceId, select: bool) {
        if self.ts.sync && self.ts.sync_uvselect_ignore() {
            self.face_select_set(f, select);
            return;
        }
        if !self.face_visible(f) {
            return;
        }
        match self.ts.sticky {
            Sticky::Disable => self.face_select_set(f, select),
            _ => self.face_select_shared_vert(f, select),
        }
    }

    /// `uvedit_face_select_shared_vert`.
    pub(crate) fn face_select_shared_vert(&mut self, f: FaceId, select: bool) {
        let loops = self.face_loops(f);
        if self.ts.sync {
            if self.ts.sticky == Sticky::Vert {
                self.bm_face_set_noflush(f, select);
                return;
            }
            if self.ts.sticky == Sticky::Disable {
                self.bm_face_set_noflush(f, select);
                for &l in &loops {
                    self.bm_loop_vert_set_noflush(l, select);
                    self.bm_loop_edge_set_noflush(l, select);
                }
            } else {
                self.bm_face_set_noflush(f, select);
                if select {
                    for &l in &loops {
                        self.bm_loop_vert_set_shared(l, true);
                        self.bm_loop_edge_set_shared(l, true);
                    }
                } else {
                    for &l in &loops {
                        if !self.bm_loop_vert_check_other_face(l, ElemFlag::SELECT_UV) {
                            self.bm_loop_vert_set_shared(l, false);
                        }
                        if !self.bm_loop_edge_check_other_face(l, ElemFlag::SELECT_UV) {
                            self.bm_loop_edge_set_shared(l, false);
                        }
                    }
                }
            }
            return;
        }
        self.face_select_set_no_sync(f, select);
        for &l in &loops {
            self.edge_select_set_no_sync(l, select);
            if select || !self.vert_is_face_select_any_other(l) {
                self.uv_select_shared_vert(l, select, Sticky::Location);
            }
        }
    }

    /// `uvedit_face_select_set`.
    pub(crate) fn face_select_set(&mut self, f: FaceId, select: bool) {
        if select {
            self.face_select_enable(f);
        } else {
            self.face_select_disable(f);
        }
    }

    /// `uvedit_face_select_enable`.
    pub(crate) fn face_select_enable(&mut self, f: FaceId) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                self.bm.face_select_set(f, true);
            } else {
                self.bm_face_set(f, true);
            }
        } else {
            self.face_select_set_no_sync(f, true);
            for l in self.face_loops(f) {
                self.vert_select_set_no_sync(l, true);
                self.edge_select_set_no_sync(l, true);
            }
        }
    }

    /// `uvedit_face_select_disable`.
    pub(crate) fn face_select_disable(&mut self, f: FaceId) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                self.bm.face_select_set(f, false);
            } else {
                self.bm_face_set(f, false);
            }
        } else {
            self.face_select_set_no_sync(f, false);
            for l in self.face_loops(f) {
                self.vert_select_set_no_sync(l, false);
                self.edge_select_set_no_sync(l, false);
            }
        }
    }

    // ---------------------------------------------------------------------- edge tests

    /// `uvedit_edge_select_test_ex`.
    pub(crate) fn edge_select_test(&self, l: LoopId) -> bool {
        if self.ts.sync {
            let mode = self.ts.selectmode;
            if !self.sync_valid && mode == SelectMode::FACE {
                return match self.ts.sticky {
                    Sticky::Location => {
                        self.fflag(self.lf(l), ElemFlag::SELECT)
                            || self.edge_is_face_select_any_other(l)
                    }
                    Sticky::Disable => self.fflag(self.lf(l), ElemFlag::SELECT),
                    Sticky::Vert => self.eflag(self.le(l), ElemFlag::SELECT),
                };
            }
            if !self.sync_valid || self.ts.sync_uvselect_ignore() {
                if mode.contains(SelectMode::FACE) {
                    return self.fflag(self.lf(l), ElemFlag::SELECT);
                }
                if mode - SelectMode::FACE == SelectMode::EDGE {
                    return self.eflag(self.le(l), ElemFlag::SELECT);
                }
                return self.vflag(self.lv(l), ElemFlag::SELECT)
                    && self.vflag(self.lv(self.next(l)), ElemFlag::SELECT);
            }
            return self.lflag(l, ElemFlag::SELECT_UV_EDGE);
        }
        if self.ts.uv_selectmode & UV_SELECT_VERT != 0 {
            return self.vert_select_get_no_sync(l) && self.vert_select_get_no_sync(self.next(l));
        }
        self.edge_select_get_no_sync(l)
    }

    /// `uvedit_edge_select_set_with_sticky`.
    pub(crate) fn edge_select_set_with_sticky(&mut self, l: LoopId, select: bool) {
        if self.ts.sync && self.ts.sync_uvselect_ignore() {
            self.edge_select_set(l, select);
            return;
        }
        match self.ts.sticky {
            Sticky::Disable => {
                if self.face_visible(self.lf(l)) {
                    self.edge_select_set(l, select);
                }
            }
            Sticky::Vert => self.edge_select_shared_vert(l, select, Sticky::Vert),
            Sticky::Location => self.edge_select_shared_vert(l, select, Sticky::Location),
        }
    }

    /// `bm_loop_select_edge_check_internal`.
    fn loop_select_edge_check_internal(&self, l: LoopId) -> bool {
        if self.ts.sync {
            if !self.sync_valid || self.ts.sync_uvselect_ignore() {
                return self.eflag(self.le(l), ElemFlag::SELECT);
            }
            return self.lflag(l, ElemFlag::SELECT_UV_EDGE);
        }
        self.edge_select_get_no_sync(l)
    }

    /// `uvedit_edge_select_shared_vert`.
    pub(crate) fn edge_select_shared_vert(&mut self, l: LoopId, select: bool, sticky: Sticky) {
        self.edge_select_set_noflush(l, select, sticky);
        let mut l_iter = l;
        loop {
            if select {
                if self.loop_select_edge_check_internal(l_iter) {
                    self.uv_select_shared_vert(l_iter, true, Sticky::Location);
                    let n = self.next(l_iter);
                    self.uv_select_shared_vert(n, true, Sticky::Location);
                }
            } else if !self.loop_select_edge_check_internal(l_iter) {
                if !self.vert_is_edge_select_any_other(l) {
                    self.uv_select_shared_vert(l_iter, false, Sticky::Location);
                }
                let ln = self.next(l);
                if !self.vert_is_edge_select_any_other(ln) {
                    let n = self.next(l_iter);
                    self.uv_select_shared_vert(n, false, Sticky::Location);
                }
            }
            l_iter = self.radial_next(l_iter);
            if l_iter == l || sticky == Sticky::Location {
                break;
            }
        }
    }

    /// `uvedit_edge_select_set_noflush`.
    pub(crate) fn edge_select_set_noflush(&mut self, l: LoopId, select: bool, sticky: Sticky) {
        let mut l_iter = l;
        loop {
            if self.face_visible(self.lf(l_iter))
                && (sticky == Sticky::Vert || self.share_edge(l, l_iter))
            {
                if self.ts.sync {
                    self.bm_loop_edge_set_noflush(l_iter, select);
                } else {
                    self.edge_select_set_no_sync(l_iter, select);
                }
            }
            l_iter = self.radial_next(l_iter);
            if l_iter == l || sticky == Sticky::Disable {
                break;
            }
        }
    }

    /// `uvedit_edge_select_set`.
    pub(crate) fn edge_select_set(&mut self, l: LoopId, select: bool) {
        if select {
            self.edge_select_enable(l);
        } else {
            self.edge_select_disable(l);
        }
    }

    /// `uvedit_edge_select_enable`.
    pub(crate) fn edge_select_enable(&mut self, l: LoopId) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                if self.ts.selectmode.contains(SelectMode::FACE) {
                    let f = self.lf(l);
                    self.bm.face_select_set(f, true);
                } else if self.ts.selectmode.contains(SelectMode::EDGE) {
                    let e = self.le(l);
                    self.bm.edge_select_set(e, true);
                } else {
                    let [a, b] = self.bm.edge(self.le(l)).v;
                    self.bm.vert_select_set(a, true);
                    self.bm.vert_select_set(b, true);
                }
            } else {
                self.bm_loop_edge_set(l, true);
            }
        } else {
            self.vert_select_set_no_sync(l, true);
            let n = self.next(l);
            self.vert_select_set_no_sync(n, true);
            self.edge_select_set_no_sync(l, true);
        }
    }

    /// `uvedit_edge_select_disable`.
    pub(crate) fn edge_select_disable(&mut self, l: LoopId) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                if self.ts.selectmode.contains(SelectMode::FACE) {
                    let f = self.lf(l);
                    self.bm.face_select_set(f, false);
                } else if self.ts.selectmode.contains(SelectMode::EDGE) {
                    let e = self.le(l);
                    self.bm.edge_select_set(e, false);
                } else {
                    let [a, b] = self.bm.edge(self.le(l)).v;
                    self.bm.vert_select_set(a, false);
                    self.bm.vert_select_set(b, false);
                }
            } else {
                self.bm_loop_edge_set_noflush(l, false);
                let (p, n) = (self.prev(l), self.next(l));
                if !self.ts.selectmode.contains(SelectMode::VERT) {
                    if !self.lflag(p, ElemFlag::SELECT_UV_EDGE) {
                        self.bm_loop_vert_set_noflush(l, false);
                    }
                    if !self.lflag(n, ElemFlag::SELECT_UV_EDGE) {
                        self.bm_loop_vert_set_noflush(n, false);
                    }
                } else {
                    self.bm_loop_vert_set_noflush(l, false);
                    self.bm_loop_vert_set_noflush(n, false);
                }
            }
        } else {
            self.edge_select_set_no_sync(l, false);
            let (p, n) = (self.prev(l), self.next(l));
            if self.ts.uv_selectmode & UV_SELECT_VERT == 0 {
                if !self.edge_select_get_no_sync(n) {
                    self.vert_select_set_no_sync(n, false);
                }
                if !self.edge_select_get_no_sync(p) {
                    self.vert_select_set_no_sync(l, false);
                }
            } else {
                self.vert_select_set_no_sync(l, false);
                self.vert_select_set_no_sync(n, false);
            }
        }
    }

    // ------------------------------------------------------------------------ UV tests

    /// `uvedit_uv_select_test_ex`.
    pub(crate) fn uv_select_test(&self, l: LoopId) -> bool {
        if self.ts.sync {
            let mode = self.ts.selectmode;
            if !self.sync_valid && mode == SelectMode::FACE {
                return match self.ts.sticky {
                    Sticky::Location => {
                        self.fflag(self.lf(l), ElemFlag::SELECT)
                            || self.vert_is_face_select_any_other(l)
                    }
                    Sticky::Disable => self.fflag(self.lf(l), ElemFlag::SELECT),
                    Sticky::Vert => self.vflag(self.lv(l), ElemFlag::SELECT),
                };
            }
            if self.sync_valid {
                // Pass.
            } else if mode - SelectMode::FACE == SelectMode::EDGE {
                return match self.ts.sticky {
                    Sticky::Location => {
                        self.fflag(self.lf(l), ElemFlag::SELECT)
                            || self.vert_is_edge_select_any_other(l)
                    }
                    Sticky::Disable => {
                        self.eflag(self.le(l), ElemFlag::SELECT)
                            || self.eflag(self.le(self.prev(l)), ElemFlag::SELECT)
                    }
                    Sticky::Vert => self.vflag(self.lv(l), ElemFlag::SELECT),
                };
            }
            if !self.sync_valid || self.ts.sync_uvselect_ignore() {
                if mode.contains(SelectMode::FACE) {
                    return self.fflag(self.lf(l), ElemFlag::SELECT);
                }
                return self.vflag(self.lv(l), ElemFlag::SELECT);
            }
            return self.lflag(l, ElemFlag::SELECT_UV);
        }
        self.vert_select_get_no_sync(l)
    }

    /// `uvedit_uv_select_set_with_sticky`.
    pub(crate) fn uv_select_set_with_sticky(&mut self, l: LoopId, select: bool) {
        if self.ts.sync && self.ts.sync_uvselect_ignore() {
            self.uv_select_set(l, select);
            return;
        }
        match self.ts.sticky {
            Sticky::Disable => {
                if self.face_visible(self.lf(l)) {
                    self.uv_select_set(l, select);
                }
            }
            Sticky::Vert => self.uv_select_shared_vert(l, select, Sticky::Vert),
            Sticky::Location => self.uv_select_shared_vert(l, select, Sticky::Location),
        }
    }

    /// `uvedit_uv_select_shared_vert`.
    pub(crate) fn uv_select_shared_vert(&mut self, l: LoopId, select: bool, sticky: Sticky) {
        let v = self.lv(l);
        for lr in self.disk_radial_loops(v) {
            if self.face_visible(self.lf(lr)) && (sticky == Sticky::Vert || self.share_vert(l, lr))
            {
                self.uv_select_set(lr, select);
            }
        }
    }

    /// `uvedit_uv_select_set`.
    pub(crate) fn uv_select_set(&mut self, l: LoopId, select: bool) {
        if select {
            self.uv_select_enable(l);
        } else {
            self.uv_select_disable(l);
        }
    }

    /// `uvedit_uv_select_enable`.
    pub(crate) fn uv_select_enable(&mut self, l: LoopId) {
        self.uv_select_enable_set(l, true);
    }

    /// `uvedit_uv_select_disable`.
    pub(crate) fn uv_select_disable(&mut self, l: LoopId) {
        self.uv_select_enable_set(l, false);
    }

    fn uv_select_enable_set(&mut self, l: LoopId, select: bool) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                if self.ts.selectmode.contains(SelectMode::FACE) {
                    let f = self.lf(l);
                    self.bm.face_select_set(f, select);
                } else {
                    let v = self.lv(l);
                    self.bm.vert_select_set(v, select);
                }
            } else {
                self.bm_loop_vert_set_noflush(l, select);
            }
        } else {
            self.vert_select_set_no_sync(l, select);
        }
    }

    // ------------------------------------------------------------------ low level API

    /// `uvedit_loop_vert_select_get`.
    pub(crate) fn loop_vert_select_get(&self, l: LoopId) -> bool {
        if self.ts.sync {
            return self.lflag(l, ElemFlag::SELECT_UV);
        }
        self.vert_select_get_no_sync(l)
    }

    /// `uvedit_loop_edge_select_get`.
    pub(crate) fn loop_edge_select_get(&self, l: LoopId) -> bool {
        if self.ts.sync {
            return self.lflag(l, ElemFlag::SELECT_UV_EDGE);
        }
        self.edge_select_get_no_sync(l)
    }

    /// `uvedit_loop_vert_select_set`.
    pub(crate) fn loop_vert_select_set(&mut self, l: LoopId, select: bool) {
        if self.ts.sync {
            self.set_lflag(l, ElemFlag::SELECT_UV, select);
            return;
        }
        self.vert_select_set_no_sync(l, select);
    }

    /// `uvedit_loop_edge_select_set`.
    pub(crate) fn loop_edge_select_set(&mut self, l: LoopId, select: bool) {
        if self.ts.sync {
            self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, select);
            return;
        }
        self.edge_select_set_no_sync(l, select);
    }

    // ------------------------------------------------------------------- any-other tests

    /// `uvedit_vert_is_edge_select_any_other`.
    pub(crate) fn vert_is_edge_select_any_other(&self, l: LoopId) -> bool {
        let v = self.lv(l);
        for e in self.bm.vert_edges(v) {
            let Some(first) = self.bm.edge(e).l else {
                continue;
            };
            let mut r = first;
            loop {
                if self.face_visible(self.lf(r)) {
                    let other = if self.lv(r) != v { self.next(r) } else { r };
                    if self.share_vert(l, other) && self.edge_select_test(r) {
                        return true;
                    }
                }
                r = self.radial_next(r);
                if r == first {
                    break;
                }
            }
        }
        false
    }

    /// `uvedit_edge_is_face_select_any_other`.
    pub(crate) fn edge_is_face_select_any_other(&self, l: LoopId) -> bool {
        let mut r = self.radial_next(l);
        while r != l {
            if self.face_visible(self.lf(r))
                && self.share_edge(l, r)
                && self.face_select_test(self.lf(r))
            {
                return true;
            }
            r = self.radial_next(r);
        }
        false
    }

    /// `uvedit_vert_is_face_select_any_other`.
    pub(crate) fn vert_is_face_select_any_other(&self, l: LoopId) -> bool {
        let f0 = self.lf(l);
        self.bm.vert_loops(self.lv(l)).into_iter().any(|li| {
            let f = self.lf(li);
            self.face_visible(f) && f != f0 && self.share_vert(l, li) && self.face_select_test(f)
        })
    }

    /// `uvedit_vert_is_all_other_faces_selected`.
    pub(crate) fn vert_is_all_other_faces_selected(&self, l: LoopId) -> bool {
        let f0 = self.lf(l);
        for li in self.bm.vert_loops(self.lv(l)) {
            let f = self.lf(li);
            if f == f0 || !self.face_visible(f) {
                continue;
            }
            if self.share_vert(l, li) && !self.face_select_test(f) {
                return false;
            }
        }
        true
    }

    /// `bm_clear_uv_vert_selection`.
    fn clear_uv_vert_selection(&mut self) {
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            for l in self.face_loops(f) {
                self.vert_select_set_no_sync(l, false);
            }
        }
    }

    // ------------------------------------------------------------------- non-sync API

    /// `uvedit_vert_select_get_no_sync`.
    pub(crate) fn vert_select_get_no_sync(&self, l: LoopId) -> bool {
        self.lflag(l, ElemFlag::SELECT_UV)
    }
    /// `uvedit_edge_select_get_no_sync`.
    pub(crate) fn edge_select_get_no_sync(&self, l: LoopId) -> bool {
        self.lflag(l, ElemFlag::SELECT_UV_EDGE)
    }
    /// `uvedit_face_select_get_no_sync`.
    pub(crate) fn face_select_get_no_sync(&self, f: FaceId) -> bool {
        self.fflag(f, ElemFlag::SELECT_UV)
    }
    /// `uvedit_vert_select_set_no_sync`.
    pub(crate) fn vert_select_set_no_sync(&mut self, l: LoopId, select: bool) {
        self.set_lflag(l, ElemFlag::SELECT_UV, select);
    }
    /// `uvedit_edge_select_set_no_sync`.
    pub(crate) fn edge_select_set_no_sync(&mut self, l: LoopId, select: bool) {
        self.set_lflag(l, ElemFlag::SELECT_UV_EDGE, select);
    }
    /// `uvedit_face_select_set_no_sync`.
    pub(crate) fn face_select_set_no_sync(&mut self, f: FaceId, select: bool) {
        self.set_fflag(f, ElemFlag::SELECT_UV, select);
    }

    // ------------------------------------------------------------------ mode flushing

    /// `ED_uvedit_selectmode_flush`.
    pub(crate) fn selectmode_flush(&mut self) {
        self.prepare_custom_data();
        if self.ts.uv_selectmode & UV_SELECT_VERT != 0 {
            for f in self.faces() {
                if !self.face_visible(f) {
                    continue;
                }
                let mut select_all = true;
                for l in self.face_loops(f) {
                    let n = self.next(l);
                    let sel = self.vert_select_get_no_sync(l) && self.vert_select_get_no_sync(n);
                    self.edge_select_set_no_sync(l, sel);
                    if !sel {
                        select_all = false;
                    }
                }
                self.face_select_set_no_sync(f, select_all);
            }
        } else if self.ts.uv_selectmode & UV_SELECT_EDGE != 0 {
            for f in self.faces() {
                if !self.face_visible(f) {
                    continue;
                }
                let select_all = self
                    .face_loops(f)
                    .into_iter()
                    .all(|l| self.edge_select_get_no_sync(l));
                self.face_select_set_no_sync(f, select_all);
            }
        }
    }

    /// `uvedit_select_flush_from_verts`.
    pub(crate) fn select_flush_from_verts(&mut self, select: bool) {
        self.prepare_custom_data();
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            let mut select_all = true;
            for l in self.face_loops(f) {
                let n = self.next(l);
                let both = self.vert_select_get_no_sync(l) && self.vert_select_get_no_sync(n);
                if select {
                    if both {
                        self.edge_select_set_no_sync(l, true);
                    } else {
                        select_all = false;
                    }
                } else if !both {
                    self.edge_select_set_no_sync(l, false);
                    select_all = false;
                }
            }
            if select && select_all {
                self.face_select_set_no_sync(f, true);
            } else if !select && !select_all {
                self.face_select_set_no_sync(f, false);
            }
        }
    }

    // ------------------------------------------------------------------- tag flushing

    /// Clears `BM_ELEM_TAG` on every face (`BM_mesh_elem_hflag_disable_all(bm, BM_FACE, …)`).
    pub(crate) fn face_tags_clear(&mut self) {
        for f in self.faces() {
            self.set_fflag(f, ElemFlag::TAG, false);
        }
    }

    /// Clears `BM_ELEM_TAG` on every loop (`bm_loop_tags_clear`).
    pub(crate) fn loop_tags_clear(&mut self) {
        for f in self.faces() {
            for l in self.face_loops(f) {
                self.set_lflag(l, ElemFlag::TAG, false);
            }
        }
    }

    /// `uv_select_flush_from_tag_face`.
    pub(crate) fn flush_from_tag_face(&mut self, select: bool) {
        self.prepare();
        let mut use_sticky = true;
        if self.ts.sync && self.ts.sync_uvselect_ignore() {
            use_sticky = false;
        }
        if self.ts.sticky == Sticky::Disable {
            use_sticky = false;
        }
        let faces = self.faces();
        if use_sticky {
            for &f in &faces {
                if !self.fflag(f, ElemFlag::TAG) {
                    continue;
                }
                if self.ts.sync {
                    self.bm_face_set_noflush(f, select);
                } else {
                    self.face_select_set_no_sync(f, select);
                }
            }
            for &f in &faces {
                if !self.fflag(f, ElemFlag::TAG) {
                    continue;
                }
                for l in self.face_loops(f) {
                    self.loop_edge_select_set(l, select);
                    if select {
                        self.edge_select_shared_vert(l, select, Sticky::Location);
                    } else {
                        if !self.vert_is_face_select_any_other(l) {
                            self.uv_select_shared_vert(l, select, Sticky::Location);
                        }
                        if !self.edge_is_face_select_any_other(l) {
                            self.edge_select_shared_vert(l, select, Sticky::Location);
                        }
                    }
                }
            }
        } else {
            for &f in &faces {
                if self.fflag(f, ElemFlag::TAG) {
                    self.face_select_set(f, select);
                }
            }
        }
    }

    /// `uv_select_flush_from_tag_loop`.
    pub(crate) fn flush_from_tag_loop(&mut self, select: bool) {
        self.prepare();
        let use_mesh_select = self.ts.sync && !self.sync_valid;
        let faces = self.faces();
        if !use_mesh_select && self.ts.sticky == Sticky::Vert {
            let mut vtag = vec![false; self.bm.verts.capacity()];
            for &f in &faces {
                for l in self.face_loops(f) {
                    if self.lflag(l, ElemFlag::TAG) {
                        vtag[self.lv(l).slot()] = true;
                    }
                }
            }
            // `BM_ELEM_TAG` of the vertices (scratch): set as Blender leaves it.
            for v in self.bm.vert_ids() {
                let on = vtag[v.slot()];
                self.bm.vert_mut(v).flag.set(ElemFlag::TAG, on);
            }
            for &f in &faces {
                let mut tag_all = true;
                let mut tag_any = false;
                for l in self.face_loops(f) {
                    if vtag[self.lv(l).slot()] {
                        self.uv_select_set(l, select);
                        tag_any = true;
                    } else {
                        tag_all = false;
                    }
                }
                self.flush_tag_face_result(f, select, tag_all, tag_any);
            }
        } else if !use_mesh_select && self.ts.sticky == Sticky::Location {
            for &f in &faces {
                let mut tag_all = true;
                let mut tag_any = false;
                for l in self.face_loops(f) {
                    if self.lflag(l, ElemFlag::TAG) {
                        self.uv_select_shared_vert(l, select, Sticky::Location);
                        tag_any = true;
                    } else {
                        tag_all = false;
                    }
                }
                self.flush_tag_face_result(f, select, tag_all, tag_any);
            }
        } else {
            for &f in &faces {
                for l in self.face_loops(f) {
                    if self.lflag(l, ElemFlag::TAG) {
                        self.uv_select_set(l, select);
                    }
                }
            }
        }
    }

    fn flush_tag_face_result(&mut self, f: FaceId, select: bool, tag_all: bool, tag_any: bool) {
        let hit = if select { tag_all } else { tag_any };
        if hit && self.face_visible(f) {
            if self.ts.sync {
                self.bm_face_set_noflush(f, select);
            } else {
                self.face_select_set_no_sync(f, select);
            }
        }
    }

    /// `uv_select_flush_from_loop_edge_flag`.
    pub(crate) fn flush_from_loop_edge_flag(&mut self) {
        self.prepare();
        let mut use_sticky = true;
        if self.ts.sync && self.ts.sticky == Sticky::Vert {
            use_sticky = false;
        }
        if self.ts.sticky == Sticky::Disable {
            use_sticky = false;
        }
        if use_sticky {
            self.clear_uv_vert_selection();
            for f in self.faces() {
                if !self.face_visible(f) {
                    continue;
                }
                for l in self.face_loops(f) {
                    if self.edge_select_get_no_sync(l) {
                        self.uv_select_shared_vert(l, true, Sticky::Location);
                        let n = self.next(l);
                        self.uv_select_shared_vert(n, true, Sticky::Location);
                    }
                }
            }
        } else if self.ts.sync {
            self.bm_flush_from_loop_edges(false);
        } else {
            for f in self.faces() {
                let mut select_all = true;
                for l in self.face_loops(f) {
                    let (p, n) = (self.prev(l), self.next(l));
                    if self.edge_select_get_no_sync(l) {
                        self.vert_select_set_no_sync(l, true);
                        self.vert_select_set_no_sync(n, true);
                    } else if !self.edge_select_get_no_sync(p) {
                        self.vert_select_set_no_sync(n, false);
                        select_all = false;
                    }
                }
                self.face_select_set_no_sync(f, select_all);
            }
        }
    }

    // ------------------------------------------------------------------- (de)select all

    /// `uvedit_select_is_any_selected`.
    pub(crate) fn is_any_selected(&self) -> bool {
        if self.ts.sync {
            let (v, e, f) = self.bm.selected_counts();
            return v != 0 || e != 0 || f != 0;
        }
        self.faces().into_iter().any(|f| {
            self.face_visible(f)
                && self
                    .face_loops(f)
                    .into_iter()
                    .any(|l| self.vert_select_get_no_sync(l))
        })
    }

    /// `uv_select_all`.
    pub(crate) fn select_all(&mut self, select_all: bool) {
        if self.ts.sync {
            self.sync_valid = false;
            if select_all {
                mesh::flag_enable_all_select(self.bm);
            } else {
                mesh::flag_disable_all_select(self.bm);
            }
            return;
        }
        self.prepare_custom_data();
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            self.face_select_set_no_sync(f, select_all);
            for l in self.face_loops(f) {
                self.vert_select_set_no_sync(l, select_all);
                self.edge_select_set_no_sync(l, select_all);
            }
        }
    }

    /// `uv_select_invert`.
    pub(crate) fn select_invert(&mut self) {
        if self.ts.sync {
            if self.ts.sync_uvselect_ignore() {
                self.sync_valid = false;
            }
            if !self.sync_valid {
                mesh::select_swap(self.bm);
                self.bm.select_flush_mode();
                return;
            }
            let faces = self.faces();
            let bm_mode = self.bm.select_mode;
            if bm_mode.contains(SelectMode::VERT) {
                for &f in &faces {
                    if self.fflag(f, ElemFlag::HIDDEN) {
                        continue;
                    }
                    for l in self.face_loops(f) {
                        let s = !self.lflag(l, ElemFlag::SELECT_UV);
                        self.bm_loop_vert_set_noflush(l, s);
                    }
                }
                self.bm_mode_flush();
            } else if bm_mode.contains(SelectMode::EDGE) {
                for &f in &faces {
                    if self.fflag(f, ElemFlag::HIDDEN) {
                        continue;
                    }
                    for l in self.face_loops(f) {
                        let s = !self.lflag(l, ElemFlag::SELECT_UV_EDGE);
                        self.bm_loop_edge_set_noflush(l, s);
                        self.bm_loop_vert_set_noflush(l, false);
                    }
                }
                for &f in &faces {
                    if self.fflag(f, ElemFlag::HIDDEN) {
                        continue;
                    }
                    let mut face_select = true;
                    for l in self.face_loops(f) {
                        if self.lflag(l, ElemFlag::SELECT_UV_EDGE) {
                            let n = self.next(l);
                            if self.ts.sticky == Sticky::Location {
                                self.bm_loop_vert_set_shared(l, true);
                                self.bm_loop_vert_set_shared(n, true);
                            } else {
                                self.bm_loop_vert_set_noflush(l, true);
                                self.bm_loop_vert_set_noflush(n, true);
                            }
                        } else {
                            face_select = false;
                        }
                    }
                    self.bm_face_set_noflush(f, face_select);
                }
            } else {
                for &f in &faces {
                    if self.fflag(f, ElemFlag::HIDDEN) {
                        continue;
                    }
                    let s = !self.fflag(f, ElemFlag::SELECT_UV);
                    self.bm_face_set(f, s);
                }
                if self.ts.sticky == Sticky::Location {
                    self.bm_flush_shared_only_select();
                }
            }
            self.bm_sync_to_mesh();
            return;
        }
        self.prepare_custom_data();
        let uv_selectmode = self.ts.uv_selectmode;
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            let mut select_all = true;
            for l in self.face_loops(f) {
                if uv_selectmode & (UV_SELECT_EDGE | UV_SELECT_FACE) != 0 {
                    let es = !self.edge_select_get_no_sync(l);
                    self.edge_select_set_no_sync(l, es);
                    self.vert_select_set_no_sync(l, false);
                    if !es {
                        select_all = false;
                    }
                } else {
                    let vs = !self.vert_select_get_no_sync(l);
                    self.vert_select_set_no_sync(l, vs);
                    self.edge_select_set_no_sync(l, false);
                    if !vs {
                        select_all = false;
                    }
                }
            }
            self.face_select_set_no_sync(f, select_all);
        }
        if uv_selectmode == UV_SELECT_EDGE || uv_selectmode == UV_SELECT_FACE {
            self.flush_from_loop_edge_flag();
        } else {
            self.select_flush_from_verts(true);
        }
    }

    /// `ED_uvedit_deselect_all` for one object (`action` already resolved from `TOGGLE` by
    /// the multi-object caller, except for single-object callers).
    pub(crate) fn deselect_all(&mut self, action: SelAction) {
        if self.ts.sync
            && self.sync_valid
            && matches!(action, SelAction::Select | SelAction::Deselect)
        {
            self.sync_valid = false;
        }
        match action {
            SelAction::Toggle => {
                let any = self.is_any_selected();
                self.select_all(!any);
            }
            SelAction::Select => self.select_all(true),
            SelAction::Deselect => self.select_all(false),
            SelAction::Invert => self.select_invert(),
        }
    }

    // ------------------------------------------------------------ selected arrays

    /// `ED_uvedit_selected_faces` (at most `len_max`).
    pub(crate) fn selected_faces(&self, len_max: usize) -> Vec<FaceId> {
        let len_max = len_max.min(self.bm.faces_num());
        let mut out = Vec::new();
        if len_max == 0 {
            return out;
        }
        for f in self.faces() {
            if self.face_visible(f) && self.face_select_test(f) {
                out.push(f);
                if out.len() == len_max {
                    break;
                }
            }
        }
        out
    }

    /// `ED_uvedit_selected_edges` (at most `len_max`; tags loops with `BM_ELEM_TAG`).
    pub(crate) fn selected_edges(&mut self, len_max: usize) -> Vec<LoopId> {
        let len_max = len_max.min(self.bm.loops_num());
        let mut out = Vec::new();
        self.loop_tags_clear();
        if len_max == 0 {
            return out;
        }
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            for l in self.face_loops(f) {
                if self.lflag(l, ElemFlag::TAG) || !self.edge_select_test(l) {
                    continue;
                }
                self.set_lflag(l, ElemFlag::TAG, true);
                out.push(l);
                if out.len() == len_max {
                    return out;
                }
                let mut r = self.radial_next(l);
                while r != l {
                    if self.share_edge(l, r) {
                        self.set_lflag(r, ElemFlag::TAG, true);
                    }
                    r = self.radial_next(r);
                }
            }
        }
        out
    }

    /// `ED_uvedit_selected_verts` (at most `len_max`; Blender tags the loop's vertex, not the
    /// loop, and then the loops sharing its UV).
    pub(crate) fn selected_verts(&mut self, len_max: usize) -> Vec<LoopId> {
        let len_max = len_max.min(self.bm.loops_num());
        let mut out = Vec::new();
        self.loop_tags_clear();
        if len_max == 0 {
            return out;
        }
        for f in self.faces() {
            if !self.face_visible(f) {
                continue;
            }
            for l in self.face_loops(f) {
                if self.lflag(l, ElemFlag::TAG) || !self.uv_select_test(l) {
                    continue;
                }
                let v = self.lv(l);
                self.bm.vert_mut(v).flag.insert(ElemFlag::TAG);
                out.push(l);
                if out.len() == len_max {
                    return out;
                }
                for ld in self.bm.vert_loops(v) {
                    if self.share_vert(l, ld) {
                        self.set_lflag(ld, ElemFlag::TAG, true);
                    }
                }
            }
        }
        out
    }

    // --------------------------------------------------------------- select mode clean

    /// `ED_uvedit_selectmode_clean`.
    pub(crate) fn selectmode_clean(&mut self) {
        let sticky = self.ts.sticky;
        self.prepare_custom_data();
        let mode = self.ts.uv_selectmode;
        if mode == UV_SELECT_VERT {
            if sticky != Sticky::Disable {
                self.loop_tags_clear();
                for f in self.faces() {
                    if !self.face_visible(f) {
                        continue;
                    }
                    for l in self.face_loops(f) {
                        if self.uv_select_test(l) {
                            self.set_lflag(l, ElemFlag::TAG, true);
                        }
                    }
                }
                self.flush_from_tag_loop(true);
            }
        } else if mode == UV_SELECT_EDGE {
            if sticky != Sticky::Disable {
                for f in self.faces() {
                    if !self.face_visible(f) {
                        continue;
                    }
                    for l in self.face_loops(f) {
                        if self.edge_select_test(l) {
                            self.edge_select_set_noflush(l, true, sticky);
                        }
                    }
                }
            }
            self.flush_from_loop_edge_flag();
        } else if mode == UV_SELECT_FACE {
            for f in self.faces() {
                self.set_fflag(f, ElemFlag::TAG, false);
                if self.face_visible(f) {
                    if self.face_select_test(f) {
                        self.set_fflag(f, ElemFlag::TAG, true);
                    }
                    self.face_select_set(f, false);
                }
            }
            self.flush_from_tag_face(true);
        }
        self.selectmode_flush();
    }

    /// `uv_select_sync_update` (sync selection was just disabled; the validity is never
    /// carried over in frap, D-111).
    pub(crate) fn sync_update(&mut self) {
        if self.ts.sync {
            return;
        }
        let uv_mode = self.ts.uv_selectmode;
        if mode_bits(self.bm.select_mode) == uv_mode {
            if !self.sync_valid {
                self.sync_uvselect_flush_from_v3d();
            }
        } else {
            let orig = self.bm.select_mode;
            self.bm.select_mode = mode_of_bits(uv_mode);
            self.sync_uvselect_flush_from_v3d();
            self.bm.select_mode = orig;
            self.sync_valid = false;
        }
    }
}

// Ported from the WP 3.8 glue (`ops/unwrap/uvedit.rs`, private to `ops`): the active UV map
// (`CustomData_get_active_layer(ldata, CD_PROP_FLOAT2)`) and its column.
/// The position of the active UV map: the BMesh's `uv_active` layer, else its first `FLOAT2`
/// corner layer.
pub(crate) fn active_uv(bm: &BMesh) -> Option<usize> {
    let float2 = |l: &CdLayer| matches!(l.data, AttrData::Float2(_)) && !l.name.starts_with('.');
    if let Some(a) = &bm.ldata.uv_active
        && let Some(i) = bm.ldata.find(a)
        && float2(&bm.ldata.layers[i])
    {
        return Some(i);
    }
    bm.ldata.layers.iter().position(float2)
}

/// The UV column of layer `uv` (by loop slot).
pub(crate) fn read_uvs(bm: &BMesh, uv: usize) -> Vec<Uv> {
    match &bm.ldata.layers[uv].data {
        AttrData::Float2(v) => {
            let mut v = v.as_ref().clone();
            v.resize(bm.loops.capacity().max(v.len()), [0.0; 2]);
            v
        }
        _ => vec![[0.0; 2]; bm.loops.capacity()],
    }
}

/// Blender's `SCE_SELECT_*` bits as a [`SelectMode`].
pub(crate) fn mode_of_bits(b: u8) -> SelectMode {
    let mut m = SelectMode::empty();
    if b & 1 != 0 {
        m |= SelectMode::VERT;
    }
    if b & 2 != 0 {
        m |= SelectMode::EDGE;
    }
    if b & 4 != 0 {
        m |= SelectMode::FACE;
    }
    m
}

/// `SEL_TOGGLE` / `SEL_SELECT` / `SEL_DESELECT` / `SEL_INVERT`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelAction {
    /// `SEL_TOGGLE`
    Toggle,
    /// `SEL_SELECT`
    Select,
    /// `SEL_DESELECT`
    Deselect,
    /// `SEL_INVERT`
    Invert,
}

impl SelAction {
    /// From the `action` enum identifier.
    pub(crate) fn of(id: &str) -> SelAction {
        match id {
            "SELECT" => SelAction::Select,
            "DESELECT" => SelAction::Deselect,
            "INVERT" => SelAction::Invert,
            _ => SelAction::Toggle,
        }
    }
}

#[cfg(test)]
mod tests;
