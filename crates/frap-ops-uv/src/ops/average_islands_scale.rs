// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.average_islands_scale(scale_uv=False, shear=False)`, tier T1
//! (`tests/golden/cases/uv/average/*.py`). Port of `average_islands_scale_exec`
//! (`editors/uvedit/uvedit_unwrap_ops.cc`) on `uv_parametrizer_average` (`frap_uv`, WP 3.3),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. The UV islands (UV connectivity and seams) of the selected faces
//! with a selected UV, over every mesh in Edit Mode with a UV map, are rescaled so that their
//! UV areas are proportional to their 3D areas, each about its own centre (`scale_uv`: U and V
//! independently; `shear`: shear removed). Pinned islands keep their size. Nothing selected:
//! `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UnwrapOptions, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::AverageIslandsScale) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let mut options = UnwrapOptions::from_ts(cx.ctx().tool_settings());
    options.topology_from_uvs = true;
    options.only_selected_faces = true;
    options.only_selected_uvs = true;
    options.fill_holes = false;
    options.correct_aspect = true;
    let objects = uvedit::edit_objects_with_uvs(cx);
    let have = objects.iter().any(|&(_, me)| {
        uvedit::edit_bm(cx, me).is_some_and(|bm| uvedit::have_selection(&bm, ts, true))
    });
    if !have {
        return Ok(OpStatus::Cancelled);
    }
    let base = uvedit::path_base(cx);
    let mut obs = UvOb::gather(cx, &objects);
    let aspect = obs
        .first()
        .map_or(1.0, |o| uvedit::aspect_y(cx.main(), &base, o.ob, &o.bm));
    let weights: Vec<Option<u32>> = obs
        .iter()
        .map(|o| uvedit::defgroup_index(cx.main(), o.me, &options.weight_group))
        .collect();
    let mut h = uvedit::construct_param_handle(&obs, ts, aspect, &weights, &options);
    h.pm.average(false, p.scale_uv.value, p.shear.value);
    h.pm.flush(&mut h.uvs);
    h.scatter(&mut obs);
    for o in obs {
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}
