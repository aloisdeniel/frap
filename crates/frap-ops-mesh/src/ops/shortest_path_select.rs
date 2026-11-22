// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.shortest_path_select(edge_mode='SELECT', use_face_step=False,
//! use_topology_distance=False, use_fill=False, skip=0, nth=1, offset=0)`, tier T1
//! (`tests/golden/cases/edit/select/shortest_path*.py`). Port of
//! `edbm_shortest_path_select_exec`, `edbm_shortest_path_pick_ex`,
//! `mouse_mesh_shortest_path_vert/edge/face` and the `*tag_*_cb` callbacks
//! (`editors/mesh/editmesh_path.cc`), `WM_operator_properties_checker_interval_from_op/_test`
//! (`windowmanager/intern/wm_operator_props.cc`), Blender 5.2.4, GPL-2.0-or-later; the path
//! searches are in `path.rs`.
//!
//! Per mesh in Edit Mode, the two endpoints are the last two entries of the selection history
//! when they have the same type (from the last to the one before), else the first two selected
//! vertices (vertex mode), edges (edge mode) or faces (face mode) in index order. The shortest
//! path between them (Dijkstra over edge lengths with a penalty for turns, or step counts with
//! `use_topology_distance`; `use_face_step` also steps across faces) is then selected, or
//! deselected when it is already fully selected; `skip`/`nth`/`offset` select a checker
//! pattern along the path; `use_fill` selects every element of all shortest paths instead.
//! In edge mode `edge_mode` may tag the path as seam, sharp, crease, bevel weight or Freestyle
//! edge instead of selecting it. Without two matching elements on any mesh the operator is
//! `CANCELLED` with Blender's WARNING `Path selection requires two matching elements to be
//! selected`. An unset `edge_mode` is the tool setting's factory value `SELECT` (frap keeps no
//! `tool_settings.edge_mode`).

pub(crate) mod path;

use std::sync::Arc;

use frap_bmesh::{BMElem, BMesh, CdLayer, EdgeId, ElemFlag, FaceId, SelectMode, VertId};
use frap_mesh::AttrData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, shortest_path_select::EdgeMode};

use self::path::{Elem, PathParams, Region};
use super::select_more::common;

/// `CheckerIntervalParams`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Interval {
    nth: i32,
    skip: i32,
    offset: i32,
}

/// C's `int` `%` as compiled for Blender's arm64/x86-64 builds, total over all inputs:
/// `INT_MIN % -1` is 0 and (never reached by the property ranges) `i % 0` is `i`, as arm64's
/// `sdiv`/`msub` pair gives.
fn c_rem(i: i32, n: i32) -> i32 {
    if n == 0 { i } else { i.wrapping_rem(n) }
}

/// `mod_i` (`blenlib/intern/math_base_inline.cc`): `(i % n + n) % n` in wrapping `int`
/// arithmetic.
fn mod_i(i: i32, n: i32) -> i32 {
    c_rem(c_rem(i, n).wrapping_add(n), n)
}

impl Interval {
    /// `WM_operator_properties_checker_interval_from_op`: `offset = mod_i(offset, nth + skip)`.
    /// The properties allow `nth + skip` past `INT_MAX`; it wraps like Blender's `int` sum.
    pub(crate) fn new(nth: i32, skip: i32, offset: i32) -> Self {
        let offset = mod_i(offset, nth.wrapping_add(skip));
        Interval { nth, skip, offset }
    }
    /// `WM_operator_properties_checker_interval_test`, with Blender's `int` sums wrapping.
    pub(crate) fn test(&self, depth: i32) -> bool {
        self.skip == 0
            || c_rem(
                self.offset.wrapping_add(depth),
                self.skip.wrapping_add(self.nth),
            ) >= self.skip
    }
}

/// `PathSelectParams`.
pub(crate) struct Params {
    pub(crate) edge_mode: EdgeMode,
    pub(crate) path: PathParams,
    pub(crate) use_fill: bool,
    pub(crate) interval: Interval,
    /// `track_active`: the pick operator moves the active element along the path.
    pub(crate) track_active: bool,
}

/// Toggles the flag of every path element (`!all_set`), keeping the checker pattern on ordered
/// paths; returns the last element set on an ordered path (`*_dst_last`).
fn toggle<T: Copy>(
    bm: &mut BMesh,
    path: &[T],
    ordered: bool,
    interval: Interval,
    test: impl Fn(&BMesh, T) -> bool,
    mut set: impl FnMut(&mut BMesh, T, bool),
) -> Option<T> {
    let all_set = path.iter().all(|&x| test(bm, x));
    let mut last = None;
    // The source has depth -1.
    for (depth, &x) in (-1..).zip(path.iter()) {
        if !ordered || interval.test(depth) {
            set(bm, x, !all_set);
            if ordered {
                last = Some(x);
            }
        }
    }
    last
}

/// `mouse_mesh_shortest_path_vert`.
pub(crate) fn shortest_path_vert(bm: &mut BMesh, p: &Params, v_act: VertId, v_dst: VertId) {
    let mut path: Option<Vec<VertId>> = None;
    let mut ordered = false;
    if v_act != v_dst {
        if p.use_fill {
            if let Some(Region::Verts(r)) =
                path::path_region(bm, Elem::Vert(v_act), Elem::Vert(v_dst))
            {
                path = Some(r);
            }
        } else {
            ordered = true;
            path = path::path_vert(bm, v_act, v_dst, p.path);
        }
    }
    let test = |bm: &BMesh, v: VertId| common::selected(bm, v);
    let path = path.filter(|x| !x.is_empty());
    if path.is_some() && p.track_active {
        bm.select_history_remove(BMElem::Vert(v_act));
    }
    let mut v_dst_last = v_dst;
    match path {
        Some(path) => {
            if let Some(last) = toggle(bm, &path, ordered, p.interval, test, |bm, v, val| {
                bm.vert_select_set(v, val);
            }) {
                v_dst_last = last;
            }
        }
        None => {
            let is_act = !test(bm, v_dst);
            bm.vert_select_set(v_dst, is_act);
        }
    }
    common::selectmode_flush(bm);
    if p.track_active {
        let set = common::selected(bm, v_dst_last);
        track_last(bm, BMElem::Vert(v_dst_last), set);
    }
}

/// The end of `mouse_mesh_shortest_path_*` with `track_active`: the last element joins the
/// selection history when it is set (`BM_select_history_store`: appended unless already
/// stored), else leaves it.
fn track_last(bm: &mut BMesh, ele: BMElem, set: bool) {
    if set {
        bm.select_history_store_if_absent(ele);
    } else {
        bm.select_history_remove(ele);
    }
}

/// Layer name and default of the custom-data `edge_mode`s.
fn edge_layer(mode: EdgeMode) -> Option<(&'static str, bool)> {
    match mode {
        EdgeMode::Crease => Some(("crease_edge", false)),
        EdgeMode::Bevel => Some(("bevel_weight_edge", false)),
        EdgeMode::Freestyle => Some(("freestyle_edge", true)),
        _ => None,
    }
}

/// `edgetag_ensure_cd_flag`.
fn ensure_edge_layer(bm: &mut BMesh, mode: EdgeMode) {
    let Some((name, is_bool)) = edge_layer(mode) else {
        return;
    };
    let exists = bm.edata.get(name).is_some_and(|l| {
        matches!(
            (&l.data, is_bool),
            (AttrData::Bool(_), true) | (AttrData::Float(_), false)
        )
    });
    if exists {
        return;
    }
    let n = bm.edges.capacity();
    let data = if is_bool {
        AttrData::Bool(Arc::new(vec![false; n]))
    } else {
        AttrData::Float(Arc::new(vec![0.0; n]))
    };
    bm.edata.add_layer(CdLayer {
        name: name.into(),
        data,
    });
}

/// `edgetag_test_cb`.
fn edge_test(bm: &BMesh, mode: EdgeMode, e: EdgeId) -> bool {
    let f = bm.edge(e).flag;
    match mode {
        EdgeMode::Select => f.contains(ElemFlag::SELECT),
        EdgeMode::Seam => f.contains(ElemFlag::SEAM),
        EdgeMode::Sharp => !f.contains(ElemFlag::SMOOTH),
        _ => {
            let Some((name, _)) = edge_layer(mode) else {
                return false;
            };
            match bm.edata.get(name).map(|l| &l.data) {
                Some(AttrData::Float(v)) => v.get(e.slot()).is_some_and(|&x| x != 0.0),
                Some(AttrData::Bool(v)) => v.get(e.slot()).copied().unwrap_or(false),
                _ => false,
            }
        }
    }
}

/// `edgetag_set_cb`.
fn edge_set(bm: &mut BMesh, mode: EdgeMode, e: EdgeId, val: bool) {
    match mode {
        EdgeMode::Select => bm.edge_select_set(e, val),
        EdgeMode::Seam => bm.edge_mut(e).flag.set(ElemFlag::SEAM, val),
        EdgeMode::Sharp => bm.edge_mut(e).flag.set(ElemFlag::SMOOTH, !val),
        _ => {
            let Some((name, _)) = edge_layer(mode) else {
                return;
            };
            let Some(i) = bm.edata.find(name) else { return };
            let slot = e.slot();
            match &mut bm.edata.layers[i].data {
                AttrData::Float(v) => {
                    if let Some(x) = Arc::make_mut(v).get_mut(slot) {
                        *x = if val { 1.0 } else { 0.0 };
                    }
                }
                AttrData::Bool(v) => {
                    if let Some(x) = Arc::make_mut(v).get_mut(slot) {
                        *x = val;
                    }
                }
                _ => {}
            }
        }
    }
}

/// `mouse_mesh_shortest_path_edge`.
pub(crate) fn shortest_path_edge(bm: &mut BMesh, p: &Params, e_act: EdgeId, e_dst: EdgeId) {
    ensure_edge_layer(bm, p.edge_mode);
    let mode = p.edge_mode;
    let mut path: Option<Vec<EdgeId>> = None;
    let mut ordered = false;
    if e_act != e_dst {
        if p.use_fill {
            if let Some(Region::Edges(r)) =
                path::path_region(bm, Elem::Edge(e_act), Elem::Edge(e_dst))
            {
                path = Some(r);
            }
        } else {
            ordered = true;
            path = path::path_edge(bm, e_act, e_dst, p.path);
        }
    }
    let test = move |bm: &BMesh, e: EdgeId| edge_test(bm, mode, e);
    let path = path.filter(|x| !x.is_empty());
    if path.is_some() && p.track_active {
        bm.select_history_remove(BMElem::Edge(e_act));
    }
    let mut e_dst_last = e_dst;
    match path {
        Some(path) => {
            if let Some(last) = toggle(bm, &path, ordered, p.interval, test, |bm, e, val| {
                edge_set(bm, mode, e, val);
            }) {
                e_dst_last = last;
            }
        }
        None => {
            let is_act = !test(bm, e_dst);
            edge_set(bm, mode, e_dst, is_act);
        }
    }
    if mode != EdgeMode::Select && p.track_active {
        // Simple rules: the last edge is always active and selected.
        bm.edge_select_set(e_act, false);
        bm.edge_select_set(e_dst_last, true);
        track_last(bm, BMElem::Edge(e_dst_last), true);
    }
    common::selectmode_flush(bm);
    if p.track_active && mode == EdgeMode::Select {
        let set = edge_test(bm, mode, e_dst_last);
        track_last(bm, BMElem::Edge(e_dst_last), set);
    }
}

/// `mouse_mesh_shortest_path_face`; returns the last face (`f_dst_last`) for the caller's
/// `material_active_index_set` when `track_active`.
pub(crate) fn shortest_path_face(
    bm: &mut BMesh,
    p: &Params,
    f_act: FaceId,
    f_dst: FaceId,
) -> FaceId {
    let path: Option<Vec<FaceId>>;
    let mut ordered = false;
    if p.use_fill {
        path = match path::path_region(bm, Elem::Face(f_act), Elem::Face(f_dst)) {
            Some(Region::Faces(r)) => Some(r),
            _ => None,
        };
    } else {
        ordered = true;
        path = path::path_face(bm, f_act, f_dst, p.path);
    }
    let test = |bm: &BMesh, f: FaceId| common::selected(bm, f);
    let path = path.filter(|x| !x.is_empty());
    if f_act != f_dst && path.is_some() && p.track_active {
        bm.select_history_remove(BMElem::Face(f_act));
    }
    let mut f_dst_last = f_dst;
    match path {
        Some(path) => {
            if let Some(last) = toggle(bm, &path, ordered, p.interval, test, |bm, f, val| {
                bm.face_select_set(f, val);
            }) {
                f_dst_last = last;
            }
        }
        None => {
            let is_act = !test(bm, f_dst);
            bm.face_select_set(f_dst, is_act);
        }
    }
    common::selectmode_flush(bm);
    if p.track_active {
        let set = common::selected(bm, f_dst_last);
        track_last(bm, BMElem::Face(f_dst_last), set);
        bm.act_face = Some(f_dst_last);
    }
    f_dst_last
}

/// The two endpoints of `edbm_shortest_path_select_exec`: the last two history entries of the
/// same type (source = last), else the first two selected elements of the select mode.
fn endpoints(bm: &BMesh) -> Option<(BMElem, BMElem)> {
    let h = &bm.select_history;
    if h.len() >= 2 {
        let (src, dst) = (h[h.len() - 1], h[h.len() - 2]);
        if src.htype() == dst.htype() {
            return Some((src, dst));
        }
    }
    let (tv, te, tf) = bm.selected_counts();
    let mode = bm.select_mode;
    let first_two = |all: Vec<BMElem>| -> Option<(BMElem, BMElem)> {
        let mut it = all.into_iter().filter(|&e| bm.is_selected(e));
        let a = it.next()?;
        let b = it.next()?;
        Some((a, b))
    };
    if mode.contains(SelectMode::VERT)
        && tv >= 2
        && let Some(x) = first_two(bm.vert_ids().into_iter().map(BMElem::Vert).collect())
    {
        return Some(x);
    }
    if mode.contains(SelectMode::EDGE)
        && te >= 2
        && let Some(x) = first_two(bm.edge_ids().into_iter().map(BMElem::Edge).collect())
    {
        return Some(x);
    }
    if mode.contains(SelectMode::FACE)
        && tf >= 2
        && let Some(x) = first_two(bm.face_ids().into_iter().map(BMElem::Face).collect())
    {
        return Some(x);
    }
    None
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "the endpoints are the last two selection-history entries of the same type, else the first two selected elements of the select mode",
        "an unset edge_mode is the factory tool setting SELECT (frap keeps no tool_settings.edge_mode)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::ShortestPathSelect) -> OpResult {
    let params = Params {
        edge_mode: p.edge_mode.value,
        path: PathParams {
            use_topology_distance: p.use_topology_distance.value,
            use_step_face: p.use_face_step.value,
        },
        use_fill: p.use_fill.value,
        interval: Interval::new(p.nth.value, p.skip.value, p.offset.value),
        track_active: false,
    };
    let mut found_valid_elements = false;
    for (_, me) in common::edit_meshes(cx) {
        let ends = common::read(cx, me, |bm| {
            if !common::any_selected(bm) {
                return None;
            }
            endpoints(bm)
        })
        .flatten();
        let Some((src, dst)) = ends else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        match (src, dst) {
            (BMElem::Vert(a), BMElem::Vert(b)) => shortest_path_vert(bm, &params, a, b),
            (BMElem::Edge(a), BMElem::Edge(b)) => shortest_path_edge(bm, &params, a, b),
            (BMElem::Face(a), BMElem::Face(b)) => {
                shortest_path_face(bm, &params, a, b);
            }
            _ => {}
        }
        found_valid_elements = true;
    }
    if !found_valid_elements {
        cx.report(
            ReportType::Warning,
            "Path selection requires two matching elements to be selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checker_interval_matches_blender() {
        // nth=1, skip=0: everything.
        let all = Interval::new(1, 0, 0);
        assert!((-1..5).all(|d| all.test(d)));
        // nth=1, skip=1, offset=0: offset becomes 0; depth -1 (the source) is skipped since
        // C's `%` keeps the sign.
        let every_other = Interval::new(1, 1, 0);
        let got: Vec<bool> = (-1..5).map(|d| every_other.test(d)).collect();
        assert_eq!(got, vec![false, false, true, false, true, false]);
        // A negative offset wraps like `mod_i`.
        assert_eq!(Interval::new(2, 1, -1).offset, 2);
    }

    #[test]
    fn checker_interval_extremes_wrap_like_c_int() {
        const MAX: i32 = i32::MAX;
        // `nth + skip` wraps past INT_MAX; no panic, and the C results.
        let a = Interval::new(1, MAX, 0);
        assert_eq!(a.offset, mod_i(0, i32::MIN));
        let b = Interval::new(MAX, 1, 0);
        let c = Interval::new(MAX, MAX, MAX);
        let d = Interval::new(MAX, MAX, -MAX);
        let e = Interval::new(1, 1, MAX);
        for iv in [a, b, c, d, e] {
            for depth in [-1, 0, 1, 2, MAX] {
                let _ = iv.test(depth);
            }
        }
        // skip = INT_MAX: n = INT_MIN, offset 0; (0 + d) % INT_MIN = d >= INT_MAX only at INT_MAX.
        assert!(!a.test(-1) && !a.test(0) && !a.test(5));
        assert!(a.test(MAX));
        // nth = INT_MAX, skip = 1: n wraps to INT_MIN, offset = 0.
        assert_eq!(b.offset, 0);
        assert!(!b.test(-1) && !b.test(0) && b.test(1) && b.test(2));
        // INT_MIN % -1 and the never-reached `% 0` are total.
        assert_eq!(c_rem(i32::MIN, -1), 0);
        assert_eq!(c_rem(7, 0), 7);
        assert_eq!(mod_i(-1, 3), 2);
        assert_eq!(mod_i(5, -3), -1);
    }
}
