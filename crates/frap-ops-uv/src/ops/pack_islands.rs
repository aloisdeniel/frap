// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.pack_islands(udim_source, rotate, rotate_method, scale, merge_overlap,
//! margin_method, margin, pin, pin_method, shape_method)`, tier T3 (`tests/golden/cases/uv/
//! pack/*.py`: UVs in the unit square, islands disjoint, area ratios kept). Port of
//! `pack_islands_exec` and `uvedit_pack_islands_multi` (`editors/uvedit/
//! uvedit_unwrap_ops.cc`), Blender 5.2.4, GPL-2.0-or-later; the packer is `frap_uv::pack`
//! (WP 3.3).
//!
//! Poll `ED_operator_uvedit` (a mesh in Edit Mode with faces and a UV map). The islands are
//! the connected UV regions of the selected faces with a selected UV (UV connectivity, seams
//! ignored), over every mesh in Edit Mode with a UV map; they are packed together into the
//! unit square (`CLOSEST_UDIM`, `ACTIVE_UDIM`: tile 1001 headless, no Image Editor),
//! back into the bounds of the selected UVs (`ORIGINAL_AABB`) or into the custom region
//! (`CUSTOM_REGION`: the unit square, frap stores no `uv_custom_region`). Without `margin` the
//! scene's `uvcalc_margin` is used (what `uv.smart_project`'s `island_margin` and
//! `uv.unwrap`'s `margin` leave there); a given `margin` is stored there. Nothing selected:
//! `CANCELLED`.
//!
//! Exact layouts match Blender on simple inputs (a smart-projected cube); in general the
//! packer's search is golden-tested by invariants (T3).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::pack_islands::{PinMethod, RotateMethod, ShapeMethod, UdimSource};
use frap_uv::{PackParams, RotationMethod};

use super::unwrap::margin_method_of;
use super::unwrap::uvedit::{self, PackTarget, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T3)]
fn exec(cx: &mut OpCtx, p: uv::PackIslands) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects_with_uvs(cx);
    let have = objects.iter().any(|&(_, me)| {
        uvedit::edit_bm(cx, me).is_some_and(|bm| uvedit::have_selection(&bm, ts, true))
    });
    if !have {
        return Ok(OpStatus::Cancelled);
    }

    // `uvcalc_margin` sync.
    let scene = cx.ctx().win.scene;
    let margin = if p.margin.is_set {
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.tool_settings.uvcalc_margin = p.margin.value;
        }
        p.margin.value
    } else {
        cx.ctx().tool_settings().uvcalc_margin
    };
    let target = match p.udim_source.value {
        UdimSource::ClosestUdim | UdimSource::ActiveUdim => PackTarget::Udim,
        UdimSource::OriginalAabb => PackTarget::OriginalAabb,
        // `uv_flag & UV_FLAG_CUSTOM_REGION` is unset in frap: the unit square.
        UdimSource::CustomRegion => PackTarget::Region([0.0, 0.0, 1.0, 1.0]),
    };

    let mut params = PackParams::default();
    // `setFromUnwrapOptions` (`topology_from_uvs`, only selected faces and UVs).
    params.only_selected_uvs = true;
    params.only_selected_faces = true;
    params.use_seams = false;
    params.correct_aspect = true;
    params.pin_unselected = false;
    params.rotate_method = if p.rotate.value {
        match p.rotate_method.value {
            RotateMethod::Any => RotationMethod::Any,
            RotateMethod::Cardinal => RotationMethod::Cardinal,
            RotateMethod::AxisAligned => RotationMethod::AxisAligned,
            RotateMethod::AxisAlignedX => RotationMethod::AxisAlignedX,
            RotateMethod::AxisAlignedY => RotationMethod::AxisAlignedY,
        }
    } else {
        RotationMethod::None
    };
    params.scale_to_fit = p.scale.value;
    params.merge_overlap = p.merge_overlap.value;
    params.pin_method = if p.pin.value {
        match p.pin_method.value {
            PinMethod::Scale => frap_uv::PinMethod::Scale,
            PinMethod::Rotation => frap_uv::PinMethod::Rotation,
            PinMethod::RotationScale => frap_uv::PinMethod::RotationScale,
            PinMethod::Locked => frap_uv::PinMethod::Locked,
        }
    } else {
        frap_uv::PinMethod::None
    };
    params.margin_method = margin_method_of(p.margin_method.value.identifier());
    params.margin = margin;
    params.shape_method = match p.shape_method.value {
        ShapeMethod::Concave => frap_uv::ShapeMethod::Concave,
        ShapeMethod::Convex => frap_uv::ShapeMethod::Convex,
        ShapeMethod::Aabb => frap_uv::ShapeMethod::Aabb,
    };

    let base = uvedit::path_base(cx);
    let mut obs = UvOb::gather(cx, &objects);
    let aspects: Vec<f32> = obs
        .iter()
        .map(|o| uvedit::aspect_y(cx.main(), &base, o.ob, &o.bm))
        .collect();
    uvedit::pack_islands_multi(&mut obs, ts, &aspects, target, &mut params);
    for o in obs {
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}
