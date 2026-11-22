// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.align(axis='ALIGN_AUTO', position_mode='MEAN')`, tier T1
//! (`tests/golden/cases/uv/edit_m8/align*.py`). Port of `uv_align_exec`, `uv_align`,
//! `uvedit_uv_straighten`, `uvedit_uv_straighten_elements` and `uvedit_line_update_endpoint`
//! (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On the selected UVs of visible faces of every mesh in Edit Mode
//! with a UV map:
//! - `ALIGN_X` / `ALIGN_Y` set U / V to the mean of the selection (`position_mode='MEAN'`) or
//!   to its minimum / maximum; `ALIGN_AUTO` picks the axis along which the selection is
//!   narrower and always uses the mean.
//! - `ALIGN_S` / `ALIGN_T` / `ALIGN_U` (straighten) work per UV island of the selection
//!   (`BM_uv_element_map_create` with seams): two far apart end points are searched
//!   (pinned UVs first), then each UV moves to the closest point of that segment
//!   (`ALIGN_S`), horizontally (`ALIGN_T`) or vertically (`ALIGN_U`) onto the line.
//!
//! The end-point search reads the pin flags (`.pn.<uv map>`); without that layer Blender reads
//! memory before the corner's custom data (`BM_ELEM_CD_GET_BOOL` at offset -1), frap reads
//! "not pinned". Its start and end points also begin uninitialised in Blender; frap starts
//! them at the origin. Always finishes.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::stitch::element_map::UvElementMap;
use super::unwrap::uvedit;
use super::weld::common::{self, Around};
use super::weld::{Tool, align_weld};
use crate::select::{Uv, UvSel, op};

/// `eUVWeldAlign` as the `axis` enum.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Axis {
    Straighten,
    StraightenX,
    StraightenY,
    Auto,
    X,
    Y,
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Align) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let mut tool = match p.axis.value.identifier() {
        "ALIGN_S" => Axis::Straighten,
        "ALIGN_T" => Axis::StraightenX,
        "ALIGN_U" => Axis::StraightenY,
        "ALIGN_X" => Axis::X,
        "ALIGN_Y" => Axis::Y,
        _ => Axis::Auto,
    };
    let position_mode = p.position_mode.value.identifier();
    let ts = op::ts(cx);
    let objects = common::objects(cx);
    let align_auto = tool == Axis::Auto;

    if tool == Axis::Auto {
        let (min, max, _) = common::minmax_multi(cx, &objects, ts);
        tool = if max[0] - min[0] >= max[1] - min[1] {
            Axis::Y
        } else {
            Axis::X
        };
    }

    let pos: Uv = if !align_auto
        && matches!(tool, Axis::X | Axis::Y)
        && matches!(position_mode, "MIN" | "MAX")
    {
        let (min, max, _) = common::minmax_multi(cx, &objects, ts);
        if position_mode == "MIN" { min } else { max }
    } else {
        common::center_multi(cx, &objects, ts, Around::Median).unwrap_or([f32::NAN; 2])
    };

    for &(_, me) in &objects {
        if common::no_vert_selected(cx, me) {
            continue;
        }
        common::edit(cx, me, ts, |s| {
            let changed = match tool {
                Axis::X => align_weld(s, Tool::AlignX, pos),
                Axis::Y => align_weld(s, Tool::AlignY, pos),
                Axis::Auto => false,
                Axis::Straighten | Axis::StraightenX | Axis::StraightenY => straighten(s, tool),
            };
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}

/// `eUVEndPointPrecedence`: `UVEP_SELECTED` (1) or'ed with `UVEP_PINNED` (2).
fn precedence(pinned: bool) -> u8 {
    if pinned { 3 } else { 1 }
}

/// `uvedit_line_update_endpoint`.
fn line_update_endpoint(
    luv: Uv,
    pinned: bool,
    uv_a: &mut Uv,
    prec_a: &mut u8,
    uv_b: &mut Uv,
    prec_b: &mut u8,
) -> bool {
    let flags = precedence(pinned);
    let len_sq_a = common::len_squared(*uv_a, luv);
    let len_sq_b = common::len_squared(*uv_b, luv);
    let len_sq_ab = common::len_squared(*uv_a, *uv_b);
    if (*prec_a < flags && 0.0 < len_sq_b) || (*prec_a == flags && len_sq_ab < len_sq_b) {
        *prec_a = flags;
        *uv_a = luv;
        return true;
    }
    if (*prec_b < flags && 0.0 < len_sq_a) || (*prec_b == flags && len_sq_ab < len_sq_a) {
        *prec_b = flags;
        *uv_b = luv;
        return true;
    }
    false
}

/// `closest_to_line_segment_v2`.
fn closest_to_line_segment(p: Uv, l1: Uv, l2: Uv) -> Uv {
    let u = common::sub(l2, l1);
    let h = common::sub(p, l1);
    let denom = common::dot(u, u);
    let (cp, lambda) = if denom == 0.0 {
        (l1, 0.0)
    } else {
        let lambda = common::dot(u, h) / denom;
        ([l1[0] + u[0] * lambda, l1[1] + u[1] * lambda], lambda)
    };
    if lambda <= 0.0 {
        l1
    } else if lambda >= 1.0 {
        l2
    } else {
        cp
    }
}

/// `uvedit_uv_straighten_elements`.
fn straighten_elements(
    s: &mut UvSel,
    loops: &[frap_bmesh::LoopId],
    pins: &[bool],
    tool: Axis,
) -> bool {
    let mut uv_start: Uv = [0.0; 2];
    let mut uv_end: Uv = [0.0; 2];
    let mut prec_start = 0u8;
    let mut prec_end = 0u8;
    // Heuristic to prevent infinite loop.
    for _ in 0..10 {
        let mut update = false;
        for &l in loops {
            let pinned = pins.get(l.slot()).copied().unwrap_or(false);
            update |= line_update_endpoint(
                s.uv(l),
                pinned,
                &mut uv_start,
                &mut prec_start,
                &mut uv_end,
                &mut prec_end,
            );
        }
        if !update {
            break;
        }
    }
    if prec_start == 0 || prec_end == 0 {
        return false;
    }
    let mut a = 0.0f32;
    let mut tool_local = tool;
    if tool_local == Axis::StraightenX {
        if uv_start[1] == uv_end[1] {
            tool_local = Axis::Straighten;
        } else {
            a = (uv_end[0] - uv_start[0]) / (uv_end[1] - uv_start[1]);
        }
    } else if tool_local == Axis::StraightenY {
        if uv_start[0] == uv_end[0] {
            tool_local = Axis::Straighten;
        } else {
            a = (uv_end[1] - uv_start[1]) / (uv_end[0] - uv_start[0]);
        }
    }
    let mut changed = false;
    for &l in loops {
        let luv = &mut s.uv[l.slot()];
        match tool_local {
            Axis::StraightenX => luv[0] = a * (luv[1] - uv_start[1]) + uv_start[0],
            Axis::StraightenY => luv[1] = a * (luv[0] - uv_start[0]) + uv_start[1],
            _ => *luv = closest_to_line_segment(*luv, uv_start, uv_end),
        }
        changed = true;
    }
    changed
}

/// `uvedit_uv_straighten`.
fn straighten(s: &mut UvSel, tool: Axis) -> bool {
    let Some(map) = UvElementMap::create(s, true, false, true, true) else {
        return false;
    };
    let pins = common::pin_flags(s);
    let mut changed = false;
    for i in 0..map.total_islands {
        let loops: Vec<_> = map.island(i).map(|e| map.storage[e].l).collect();
        changed |= straighten_elements(s, &loops, &pins, tool);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_point_clamps_to_the_segment() {
        assert_eq!(
            closest_to_line_segment([0.5, 1.0], [0.0, 0.0], [1.0, 0.0]),
            [0.5, 0.0]
        );
        assert_eq!(
            closest_to_line_segment([-1.0, 1.0], [0.0, 0.0], [1.0, 0.0]),
            [0.0, 0.0]
        );
        assert_eq!(
            closest_to_line_segment([2.0, 1.0], [0.0, 0.0], [1.0, 0.0]),
            [1.0, 0.0]
        );
        assert_eq!(
            closest_to_line_segment([2.0, 1.0], [0.5, 0.5], [0.5, 0.5]),
            [0.5, 0.5]
        );
    }

    #[test]
    fn pinned_end_points_win() {
        let (mut a, mut b) = ([0.0; 2], [0.0; 2]);
        let (mut pa, mut pb) = (0u8, 0u8);
        for (uv, pin) in [
            ([0.1, 0.1], false),
            ([0.9, 0.2], false),
            ([0.5, 0.15], true),
        ] {
            line_update_endpoint(uv, pin, &mut a, &mut pa, &mut b, &mut pb);
        }
        assert_eq!(pa, 3);
        assert_eq!(a, [0.5, 0.15]);
    }
}
