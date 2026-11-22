// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.minimize_stretch(fill_holes=True, blend=0.0, iterations=0)`, tier T2
//! (`tests/golden/cases/uv/minimize_stretch/*.py`). Port of `minimize_stretch_exec`
//! (`minimize_stretch_init`, `minimize_stretch_iteration`, `minimize_stretch_exit` of
//! `editors/uvedit/uvedit_unwrap_ops.cc`) on `uv_parametrizer_stretch_*` (`frap_uv::stretch`,
//! WP 8.1), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit` (a mesh in Edit Mode with faces and a UV map). The UV islands
//! (connectivity from the UVs) of the selected faces with a selected UV, over every mesh in
//! Edit Mode with a UV map, are relaxed: their boundaries stay, user pins are ignored, and each
//! of the `iterations` passes moves every selected interior UV vertex in a random direction
//! (Blender's fixed seed) by the step that lowers the texture stretch of its faces. The result
//! is blended with the original UVs by `blend`. As in Blender, `exec` runs exactly
//! `iterations` passes, so the default `iterations=0` changes nothing but the flush (`0` means
//! "until confirmed" only in the interactive modal operator). Nothing selected: `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UnwrapOptions, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: uv::MinimizeStretch) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = UvTs::of(&cx.ctx());
    // `minimize_stretch_init`: `UnwrapOptions options{}` with these set.
    let mut options = UnwrapOptions::from_ts(cx.ctx().tool_settings());
    options.topology_from_uvs = true;
    options.topology_from_uvs_use_seams = false;
    options.fill_holes = p.fill_holes.value;
    options.only_selected_faces = true;
    options.only_selected_uvs = true;
    options.pin_unselected = false;
    options.correct_aspect = true;
    options.use_abf = false;
    options.use_subsurf = false;
    options.use_original_bounds = false;
    options.weight_group = "".into();

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
    // `cd_weight_index` of an empty group name: no weights.
    let weights: Vec<Option<u32>> = vec![None; obs.len()];
    let mut h = uvedit::construct_param_handle(&obs, ts, aspect, &weights, &options);

    let blend = p.blend.value;
    h.pm.stretch_begin();
    if blend != 0.0 {
        h.pm.stretch_blend(blend);
    }
    for _ in 0..p.iterations.value.max(0) {
        // `minimize_stretch_iteration(C, op, false)`.
        h.pm.stretch_blend(blend);
        h.pm.stretch_iter();
    }
    // `minimize_stretch_exit(C, op, false)`.
    h.pm.flush(&mut h.uvs);
    h.pm.stretch_end();
    h.scatter(&mut obs);
    for o in obs {
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}
