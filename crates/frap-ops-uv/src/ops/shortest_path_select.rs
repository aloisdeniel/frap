// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.shortest_path_select(use_face_step=False, use_topology_distance=False,
//! use_fill=False, skip=0, nth=1, offset=0)`, tier T1
//! (`tests/golden/cases/uv/select/path*.py`). Port of `uv_shortest_path_select_exec`,
//! `uv_shortest_path_pick_ex` and `mouse_mesh_uv_shortest_path_vert/edge/face`
//! (`editors/uvedit/uvedit_path.cc`) with `WM_operator_properties_checker_interval_*`
//! (`windowmanager/intern/wm_operator_props.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended*. On every mesh in Edit Mode
//! with a UV map with exactly two selected UV elements of the UV select mode (vertices, edges
//! or faces; UV-connected copies count once), the shortest UV path between them (`use_fill`:
//! every path, the region) is toggled: selected unless already all selected; `skip`/`nth`/
//! `offset` select a checker pattern along an ordered path. Distances are measured in UV
//! space scaled by the aspect of the active face's image. `CANCELLED` with a warning when no
//! mesh has two such elements. Like Blender, the operator does not prepare the synced UV
//! selection first.

use frap_bmesh::{FaceId, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::path::PathParams;
use crate::select::{UV_SELECT_EDGE, UV_SELECT_FACE, UvSel, op};

/// `CheckerIntervalParams`.
#[derive(Copy, Clone, Debug)]
struct Checker {
    nth: i32,
    skip: i32,
    offset: i32,
}

impl Checker {
    /// `WM_operator_properties_checker_interval_test`.
    fn test(&self, depth: i32) -> bool {
        self.skip == 0
            || (self.offset.wrapping_add(depth)) % (self.skip.wrapping_add(self.nth)) >= self.skip
    }
}

/// `PathSelectParams`.
#[derive(Copy, Clone, Debug)]
struct Params {
    path: PathParams,
    use_fill: bool,
    checker: Checker,
}

fn verttag_test(s: &UvSel, l: LoopId) -> bool {
    let luv = s.uv(l);
    s.bm
        .vert_loops(s.lv(l))
        .into_iter()
        .all(|li| !(s.face_visible(s.lf(li)) && s.uv(li) == luv) || s.uv_select_test(li))
}

fn verttag_set(s: &mut UvSel, l: LoopId, val: bool) {
    let luv = s.uv(l);
    for li in s.bm.vert_loops(s.lv(l)) {
        if s.face_visible(s.lf(li)) && s.uv(li) == luv {
            s.uv_select_set(li, val);
        }
    }
}

fn edgetag_test(s: &UvSel, l: LoopId) -> bool {
    s.bm
        .edge_loops(s.le(l))
        .into_iter()
        .all(|li| !(s.face_visible(s.lf(li)) && s.share_edge(l, li)) || s.edge_select_test(li))
}

fn facetag_test(s: &UvSel, f: FaceId) -> bool {
    s.face_loops(f).into_iter().all(|l| s.edge_select_test(l))
}

/// Toggles an ordered or region path; returns the flush direction (1 select, -1 deselect,
/// 0 nothing).
fn toggle_path<T: Copy + PartialEq>(
    s: &mut UvSel,
    path: Vec<T>,
    dst: T,
    ordered: bool,
    checker: Checker,
    test: fn(&UvSel, T) -> bool,
    set: fn(&mut UvSel, T, bool),
) -> i32 {
    if path.is_empty() {
        let is_act = !test(s, dst);
        set(s, dst, is_act);
        return 0;
    }
    let all_set = path.iter().all(|&e| test(s, e));
    // `depth` starts at -1 for the first element, as Blender's loop.
    for (depth, &e) in (-1..).zip(path.iter()) {
        if !ordered || checker.test(depth) {
            set(s, e, !all_set);
        }
    }
    if all_set { -1 } else { 1 }
}

/// `uv_shortest_path_pick_ex` with two elements of one mesh.
fn pick(s: &mut UvSel, p: Params, src: Elem, dst: Elem) {
    let uv_selectmode = s.ts.select_mode_get();
    let flush = match (src, dst) {
        (Elem::Face(a), Elem::Face(b)) => {
            let (path, ordered) = if a == b {
                (Vec::new(), false)
            } else if p.use_fill {
                (s.path_uv_region_face(a, b), false)
            } else {
                (s.path_uv_face(a, b, p.path), true)
            };
            toggle_path(s, path, b, ordered, p.checker, facetag_test, |s, f, v| {
                s.face_select_set_with_sticky(f, v)
            })
        }
        (Elem::Loop(a), Elem::Loop(b)) => {
            if uv_selectmode & UV_SELECT_EDGE != 0 {
                let (path, ordered) = if a == b {
                    (Vec::new(), false)
                } else if p.use_fill {
                    (s.path_uv_region_loops(a, b, true), false)
                } else {
                    (s.path_uv_edge(a, b, p.path), true)
                };
                toggle_path(s, path, b, ordered, p.checker, edgetag_test, |s, l, v| {
                    s.edge_select_set_with_sticky(l, v)
                })
            } else {
                let (path, ordered) = if a == b {
                    (Vec::new(), false)
                } else if p.use_fill {
                    (s.path_uv_region_loops(a, b, false), false)
                } else {
                    (s.path_uv_vert(a, b, p.path), true)
                };
                toggle_path(s, path, b, ordered, p.checker, verttag_test, verttag_set)
            }
        }
        _ => return,
    };
    if flush != 0 {
        let select = flush == 1;
        if s.ts.sync {
            s.select_sync_flush(select);
        } else {
            s.selectmode_flush();
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Elem {
    Loop(LoopId),
    Face(FaceId),
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::ShortestPathSelect) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let uv_selectmode = ts.select_mode_get();
    // `ED_uvedit_get_aspect_y(CTX_data_edit_object(C))`.
    let aspect_y = {
        let edit = poll::edit_object(&cx.ctx()).and_then(|(ob, o)| {
            let me = o.data?.typed::<frap_rna::MeshId>()?;
            Some((ob, me))
        });
        match edit {
            Some((ob, me)) => match uvedit::edit_bm(cx, me) {
                Some(bm) => {
                    let base = uvedit::path_base(cx);
                    uvedit::aspect_y(cx.ctx().main, &base, ob, &bm)
                }
                None => 1.0,
            },
            None => 1.0,
        }
    };
    let nth = p.nth.value;
    let skip = p.skip.value;
    let n = nth.wrapping_add(skip);
    let offset = if n == 0 {
        0
    } else {
        (p.offset.value % n + n) % n
    };
    let params = Params {
        path: PathParams {
            use_topology_distance: p.use_topology_distance.value,
            use_step_face: p.use_face_step.value,
            aspect_y,
        },
        use_fill: p.use_fill.value,
        checker: Checker { nth, skip, offset },
    };
    let mut found = false;
    for (_, me) in op::objects(cx) {
        let hit = op::edit(cx, me, ts, |s| {
            let elems: Vec<Elem> = if uv_selectmode & UV_SELECT_FACE != 0 {
                s.selected_faces(3).into_iter().map(Elem::Face).collect()
            } else if uv_selectmode & UV_SELECT_EDGE != 0 {
                s.selected_edges(3).into_iter().map(Elem::Loop).collect()
            } else {
                s.selected_verts(3).into_iter().map(Elem::Loop).collect()
            };
            if elems.len() != 2 {
                return false;
            }
            pick(s, params, elems[0], elems[1]);
            true
        })?;
        found |= hit.unwrap_or(false);
    }
    if !found {
        cx.report(
            ReportType::Warning,
            "Path selection requires two matching elements to be selected",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
