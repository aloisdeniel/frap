// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.snap_selected(target='PIXELS')`, tier T1
//! (`tests/golden/cases/uv/edit_m8/snap*.py`). Port of `uv_snap_selection_exec`,
//! `uv_snap_uvs_to_pixels`, `uv_snap_uvs_to_cursor`, `uv_snap_uvs_offset` and
//! `uv_snap_uvs_to_adjacent_unselected` (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended* (frap's virtual Image Editor,
//! `weld/common.rs`). On the selected UVs of visible faces of every mesh in Edit Mode with a
//! UV map and a selected vertex: `PIXELS` rounds them to the 256 × 256 pixel grid, `CURSOR`
//! moves them to the 2D cursor (`uv.snap_cursor`), `CURSOR_OFFSET` translates them so that the
//! centre of their bounds lands on the cursor (`CANCELLED` without a selection),
//! `ADJACENT_UNSELECTED` moves each to the average of the unselected UVs of visible faces at
//! its mesh vertex. `CANCELLED` when no UV changed.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::snap_cursor::snap_to_pixel;
use super::unwrap::uvedit;
use super::weld::common::{self, IMAGE_EDITOR_NOTE, IMAGE_SIZE, SIMA_AROUND};
use crate::select::{UvSel, op};

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = HeadlessExtended(IMAGE_EDITOR_NOTE)
)]
fn exec(cx: &mut OpCtx, p: uv::SnapSelected) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let target = p.target.value.identifier();
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    let cursor = common::cursor(cx);
    let mut offset = [0.0f32; 2];
    if target == "CURSOR_OFFSET" {
        let Some(center) = common::center_multi(cx, &objects, ts, SIMA_AROUND) else {
            return Ok(OpStatus::Cancelled);
        };
        offset = common::sub(cursor, center);
    }
    let mut changed_multi = false;
    for &(_, me) in &objects {
        if common::no_vert_selected(cx, me) {
            continue;
        }
        let changed = common::edit(cx, me, ts, |s| {
            let mut changed = false;
            match target {
                "CURSOR" => common::foreach_uv(s, true, true, |luv| {
                    *luv = cursor;
                    changed = true;
                }),
                "CURSOR_OFFSET" => common::foreach_uv(s, true, true, |luv| {
                    luv[0] += offset[0];
                    luv[1] += offset[1];
                    changed = true;
                }),
                "ADJACENT_UNSELECTED" => changed = to_adjacent_unselected(s),
                _ => {
                    let (w, h) = (IMAGE_SIZE[0] as f32, IMAGE_SIZE[1] as f32);
                    common::foreach_uv(s, true, true, |luv| {
                        snap_to_pixel(luv, w, h);
                        changed = true;
                    });
                }
            }
            (changed, changed)
        })?;
        changed_multi |= changed.unwrap_or(false);
    }
    Ok(if changed_multi {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}

/// `uv_snap_uvs_to_adjacent_unselected` (the face and loop tags as local tables).
fn to_adjacent_unselected(s: &mut UvSel) -> bool {
    let faces = s.faces();
    let mut face_tag = vec![false; s.bm.faces.capacity()];
    let mut loop_tag = vec![false; s.bm.loops.capacity()];
    for &f in &faces {
        if s.face_visible(f) {
            face_tag[f.slot()] = true;
            for l in s.face_loops(f) {
                loop_tag[l.slot()] = s.uv_select_test(l);
            }
        }
    }
    let mut changed = false;
    for &f in &faces {
        if !face_tag[f.slot()] {
            continue;
        }
        for l in s.face_loops(f) {
            if !loop_tag[l.slot()] {
                continue;
            }
            let mut uv = [0.0f32; 2];
            let mut uv_tot = 0u32;
            for lsub in s.bm.vert_loops(s.lv(l)) {
                if face_tag[s.lf(lsub).slot()] && !loop_tag[lsub.slot()] {
                    let luv = s.uv(lsub);
                    uv[0] += luv[0];
                    uv[1] += luv[1];
                    uv_tot += 1;
                }
            }
            if uv_tot != 0 {
                let fac = 1.0 / uv_tot as f32;
                s.uv[l.slot()] = [uv[0] * fac, uv[1] * fac];
                changed = true;
            }
        }
    }
    changed
}
