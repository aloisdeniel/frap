// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.hide(unselected=False)`, tier T1 (`tests/golden/cases/uv/select/hide*.py`).
//! Port of `uv_hide_exec`, `uv_mesh_hide_select_sync` and `uv_mesh_hide_no_sync`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. With sync selection the UV editor hides mesh elements: the
//! selected (`unselected`: the unselected) ones, as `mesh.hide`. Without it, "hiding" in the
//! UV editor deselects: faces with selected UVs are deselected in the mesh (vertex/edge
//! select modes deselect their vertices/edges, in two passes so neighbouring islands keep
//! their faces) and lose their UV selection. Always finishes.

use frap_bmesh::{EdgeId, ElemFlag, LoopId, SelectMode, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use crate::select::{UV_SELECT_EDGE, UV_SELECT_FACE, UvSel, mesh, op};

/// `UV_VERT_SEL_TEST`.
fn vert_sel_test(s: &UvSel, l: LoopId, test: bool) -> bool {
    s.vert_select_get_no_sync(l) == test
}
/// `UV_EDGE_SEL_TEST`.
fn edge_sel_test(s: &UvSel, l: LoopId, test: bool) -> bool {
    s.edge_select_get_no_sync(l) == test
}

/// `uv_mesh_hide_select_sync` (the UV selection follows the mesh selection in frap, D-111, so
/// this is `EDBM_mesh_hide`).
fn hide_select_sync(s: &mut UvSel, swap: bool) -> bool {
    let select_to_hide = !swap;
    if !s.sync_valid || s.ts.sync_uvselect_ignore() {
        return mesh::mesh_hide(s.bm, swap);
    }
    let mut changed = false;
    let faces = s.faces();
    let bm_mode = s.bm.select_mode;
    if bm_mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
        s.face_tags_clear();
        for &f in &faces {
            if s.fflag(f, ElemFlag::HIDDEN) {
                continue;
            }
            for l in s.face_loops(f) {
                let hit = if bm_mode.contains(SelectMode::VERT) {
                    s.vflag(s.lv(l), ElemFlag::SELECT) == select_to_hide
                        && s.lflag(l, ElemFlag::SELECT_UV) == select_to_hide
                } else {
                    s.eflag(s.le(l), ElemFlag::SELECT) == select_to_hide
                        && s.lflag(l, ElemFlag::SELECT_UV_EDGE) == select_to_hide
                };
                if hit {
                    s.set_fflag(f, ElemFlag::TAG, true);
                    changed = true;
                    break;
                }
            }
        }
        if changed {
            for &f in &faces {
                if s.fflag(f, ElemFlag::TAG) {
                    s.bm.elem_hide_set(f.into(), true);
                }
            }
            if swap {
                for &f in &faces {
                    if !s.fflag(f, ElemFlag::TAG) {
                        s.bm.face_select_set(f, true);
                    }
                }
            }
        }
    } else {
        for &f in &faces {
            if s.fflag(f, ElemFlag::HIDDEN) {
                continue;
            }
            if s.fflag(f, ElemFlag::SELECT_UV) == select_to_hide {
                s.bm.elem_hide_set(f.into(), true);
                changed = true;
            }
        }
    }
    if changed {
        if swap {
            s.bm.select_flush_mode();
        } else {
            mesh::flag_disable_all_select(s.bm);
        }
        s.sync_valid = false;
    }
    changed
}

/// `bm_face_is_any_sel`.
fn face_is_any_sel(s: &UvSel, f: frap_bmesh::FaceId, test: bool) -> bool {
    s.face_loops(f)
        .into_iter()
        .any(|l| vert_sel_test(s, l, test) || edge_sel_test(s, l, test))
}
/// `bm_face_is_any_vert_sel`.
fn face_is_any_vert_sel(s: &UvSel, f: frap_bmesh::FaceId, test: bool) -> bool {
    s.face_loops(f)
        .into_iter()
        .any(|l| vert_sel_test(s, l, test))
}
/// `bm_face_is_any_edge_sel`.
fn face_is_any_edge_sel(s: &UvSel, f: frap_bmesh::FaceId, test: bool) -> bool {
    s.face_loops(f)
        .into_iter()
        .any(|l| edge_sel_test(s, l, test))
}
/// `bm_face_is_all_uv_sel`.
fn face_is_all_uv_sel(s: &UvSel, f: frap_bmesh::FaceId, test: bool) -> bool {
    s.face_loops(f)
        .into_iter()
        .all(|l| edge_sel_test(s, l, test))
}

/// `bm_vert_deselect_for_pass_fn`.
fn vert_deselect_for_pass(s: &mut UvSel, v: VertId, pass: u8, swap: bool) -> bool {
    if pass == 0 {
        for l in s.bm.vert_loops(v) {
            if s.face_visible(s.lf(l)) && !vert_sel_test(s, l, !swap) {
                return false;
            }
        }
    }
    s.bm.vert_select_set(v, false);
    true
}

/// `bm_edge_deselect_for_pass_fn`.
fn edge_deselect_for_pass(s: &mut UvSel, e: EdgeId, pass: u8, swap: bool) -> bool {
    if pass == 0
        && let Some(first) = s.bm.edge(e).l
    {
        let mut l = first;
        loop {
            if s.face_visible(s.lf(l)) && !edge_sel_test(s, l, !swap) {
                return false;
            }
            l = s.radial_next(l);
            if l == first {
                break;
            }
        }
    }
    s.bm.edge_select_set(e, false);
    true
}

/// `uv_mesh_hide_no_sync`.
fn hide_no_sync(s: &mut UvSel, swap: bool) -> bool {
    let use_face_center = s.ts.uv_selectmode == UV_SELECT_FACE;
    let uv_edge_mode = s.ts.uv_selectmode == UV_SELECT_EDGE;
    let mut changed = false;
    let mut changed_uv = false;
    let em_mode = s.bm.select_mode;
    let pass_end = if em_mode == SelectMode::FACE { 0 } else { 1 };
    for pass in 0..=pass_end {
        for f in s.faces() {
            if !s.face_visible(f) || !face_is_any_sel(s, f, !swap) {
                continue;
            }
            let loops = s.face_loops(f);
            if em_mode.contains(SelectMode::VERT) {
                if use_face_center {
                    if face_is_all_uv_sel(s, f, true) == !swap {
                        for &l in &loops {
                            if edge_sel_test(s, l, !swap) {
                                let v = s.lv(l);
                                changed |= vert_deselect_for_pass(s, v, pass, swap);
                            }
                        }
                    }
                } else if uv_edge_mode {
                    let mut e_prev = edge_sel_test(s, s.prev(s.first_loop(f)), !swap);
                    for &l in &loops {
                        let e_curr = edge_sel_test(s, l, !swap);
                        if e_prev || e_curr {
                            let v = s.lv(l);
                            changed |= vert_deselect_for_pass(s, v, pass, swap);
                        }
                        e_prev = e_curr;
                    }
                } else {
                    for &l in &loops {
                        if vert_sel_test(s, l, !swap) {
                            let v = s.lv(l);
                            changed |= vert_deselect_for_pass(s, v, pass, swap);
                        }
                    }
                }
                if pass == pass_end && !swap {
                    s.face_select_disable(f);
                    changed_uv = true;
                }
            } else if em_mode.contains(SelectMode::EDGE) {
                if use_face_center {
                    if face_is_all_uv_sel(s, f, true) == !swap {
                        for &l in &loops {
                            if edge_sel_test(s, l, !swap) {
                                let e = s.le(l);
                                changed |= edge_deselect_for_pass(s, e, pass, swap);
                            }
                        }
                    }
                } else {
                    for &l in &loops {
                        let hit = if uv_edge_mode {
                            edge_sel_test(s, l, !swap)
                        } else {
                            vert_sel_test(s, l, !swap)
                        };
                        if hit {
                            let e = s.le(l);
                            changed |= edge_deselect_for_pass(s, e, pass, swap);
                        }
                    }
                }
                if pass == pass_end && !swap {
                    s.face_select_disable(f);
                    changed_uv = true;
                }
            } else {
                if use_face_center {
                    if face_is_all_uv_sel(s, f, !swap) {
                        s.bm.face_select_set(f, false);
                        changed = true;
                    }
                } else {
                    let has_select = if uv_edge_mode {
                        face_is_any_edge_sel(s, f, !swap)
                    } else {
                        face_is_any_vert_sel(s, f, !swap)
                    };
                    if has_select {
                        s.bm.face_select_set(f, false);
                        changed = true;
                    }
                }
                s.face_select_disable(f);
                changed_uv = true;
            }
        }
        if changed && em_mode.intersects(SelectMode::VERT | SelectMode::EDGE) {
            s.bm.select_flush_mode();
        }
    }
    if changed {
        s.bm.select_history_validate();
    }
    changed || changed_uv
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Hide) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let swap = p.unselected.value;
    for (_, me) in op::objects(cx) {
        op::edit(cx, me, ts, |s| {
            if s.ts.sync {
                hide_select_sync(s, swap);
            } else {
                hide_no_sync(s, swap);
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
