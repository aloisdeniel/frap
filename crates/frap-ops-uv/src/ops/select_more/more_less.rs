// SPDX-License-Identifier: GPL-3.0-or-later
//! `uv_select_more_less`, shared by `uv.select_more` and `uv.select_less`.
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_select.cc (`uv_select_more_less`),
//! Blender 5.2.4, GPL-2.0-or-later.

use frap_bmesh::{ElemFlag, SelectMode};
use frap_op::prelude::*;

use crate::ops::unwrap::uvedit;
use crate::select::{UV_SELECT_FACE, UvSel, mesh, op};

/// One mesh.
fn more_less(s: &mut UvSel, select: bool, is_face_mode: bool) {
    s.prepare();
    if s.ts.sync && !s.sync_valid {
        mesh::select_more_less(s.bm, !select, true);
        return;
    }
    let mut changed = false;
    if is_face_mode {
        s.face_tags_clear();
        for f in s.faces() {
            if !s.face_visible(f) {
                continue;
            }
            if select {
                // `NEIGHBORING_FACE_IS_SEL` 1, `CURR_FACE_IS_UNSEL` 2.
                let mut state = 0u8;
                for l in s.face_loops(f) {
                    if s.loop_vert_select_get(l) {
                        state |= 1;
                    } else {
                        state |= 2;
                    }
                    if !s.loop_edge_select_get(l) {
                        state |= 2;
                    }
                    if state == 3 {
                        s.set_fflag(f, ElemFlag::TAG, true);
                        changed = true;
                        break;
                    }
                }
            } else {
                if !s.face_select_test(f) {
                    continue;
                }
                for l in s.face_loops(f) {
                    if !s.vert_is_all_other_faces_selected(l) {
                        s.set_fflag(f, ElemFlag::TAG, true);
                        changed = true;
                        break;
                    }
                }
            }
        }
    } else {
        s.loop_tags_clear();
        for f in s.faces() {
            if !s.face_visible(f) {
                continue;
            }
            for l in s.face_loops(f) {
                if s.loop_vert_select_get(l) == select {
                    let (n, p) = (s.next(l), s.prev(l));
                    s.set_lflag(n, ElemFlag::TAG, true);
                    s.set_lflag(p, ElemFlag::TAG, true);
                    changed = true;
                }
            }
        }
    }
    if !changed {
        return;
    }
    if is_face_mode {
        s.flush_from_tag_face(select);
    } else {
        s.flush_from_tag_loop(select);
        if !s.ts.sync {
            s.select_flush_from_verts(select);
        }
    }
    if s.ts.sync {
        if select {
            s.bm_flush_from_loop_verts_only_select();
        } else {
            s.bm_flush_from_loop_verts_only_deselect();
        }
        s.bm_sync_to_mesh();
    }
}

/// `uv_select_more_less(C, select)`.
pub(crate) fn exec(cx: &mut OpCtx, select: bool) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let is_face_mode = if ts.sync {
        ts.selectmode == SelectMode::FACE
    } else {
        ts.uv_selectmode == UV_SELECT_FACE
    };
    for (_, me) in op::objects(cx) {
        op::edit(cx, me, ts, |s| more_less(s, select, is_face_mode))?;
    }
    Ok(OpStatus::Finished)
}
