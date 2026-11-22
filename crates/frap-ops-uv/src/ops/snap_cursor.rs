// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.snap_cursor(target='PIXELS')`, tier T1
//! (`tests/golden/cases/uv/edit_m8/snap*.py`). Port of `uv_snap_cursor_exec`,
//! `uv_snap_cursor_to_pixels`, `uv_snap_cursor_to_selection` and `uv_snap_cursor_to_origin`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended*, the 2D cursor of frap's virtual
//! Image Editor (`ToolSettings::uv_cursor`, see `weld/common.rs`). `PIXELS` rounds the cursor
//! to the 256 × 256 pixel grid of an Image Editor without an image, `SELECTED` moves it to the
//! bounds centre of the selected UVs of visible faces (`CANCELLED` without any), `ORIGIN` to
//! (0, 0). `uv.snap_selected(target='CURSOR' | 'CURSOR_OFFSET')` and
//! `uv.arrange_islands(initial_position='CURSOR')` read it.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit;
use super::weld::common::{self, IMAGE_EDITOR_NOTE, IMAGE_SIZE, SIMA_AROUND};
use crate::select::{Uv, op};

/// `uv_snap_to_pixel`.
pub(crate) fn snap_to_pixel(uv: &mut Uv, w: f32, h: f32) {
    uv[0] = (uv[0] * w).round() / w;
    uv[1] = (uv[1] * h).round() / h;
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = HeadlessExtended(IMAGE_EDITOR_NOTE)
)]
fn exec(cx: &mut OpCtx, p: uv::SnapCursor) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let mut cursor = common::cursor(cx);
    let changed = match p.target.value.identifier() {
        "SELECTED" => {
            let ts = op::ts(cx);
            let objects = common::objects(cx);
            match common::center_multi(cx, &objects, ts, SIMA_AROUND) {
                Some(c) => {
                    cursor = c;
                    true
                }
                None => false,
            }
        }
        "ORIGIN" => {
            cursor = [0.0, 0.0];
            true
        }
        _ => {
            snap_to_pixel(&mut cursor, IMAGE_SIZE[0] as f32, IMAGE_SIZE[1] as f32);
            true
        }
    };
    if !changed {
        return Ok(OpStatus::Cancelled);
    }
    common::set_cursor(cx, cursor);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_snap_rounds_half_away_from_zero() {
        let mut uv = [0.5 / 256.0 + 0.1, -1.5 / 256.0];
        snap_to_pixel(&mut uv, 256.0, 256.0);
        assert_eq!(uv, [26.0 / 256.0, -2.0 / 256.0]);
    }
}
