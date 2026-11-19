// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.smart_project(angle_limit, margin_method, rotate_method, island_margin,
//! area_weight, correct_aspect, scale_to_bounds)`, tier T1 (`tests/golden/cases/uv/smart/
//! *.py`). Port of `smart_project_exec` (`editors/uvedit/uvedit_unwrap_ops.cc`), Blender
//! 5.2.4, GPL-2.0-or-later; the grouping and projection are `frap_uv::smart` (WP 3.3).
//!
//! Poll `ED_operator_uvmap`. For every mesh in Edit Mode (a UV map is added when missing),
//! the selected faces are grouped by normal (`angle_limit`, `area_weight` biasing towards
//! large faces), each group is projected orthographically along its normal, faces of zero
//! area get (0, 0), and the islands of the projected meshes are packed into the unit square
//! (`rotate_method`, `margin_method`, `island_margin`) and finally clipped/corrected
//! (`correct_aspect` with the active face's material image, `scale_to_bounds`).
//! `island_margin` is stored as the scene's `uvcalc_margin` (what `uv.pack_islands` uses
//! without `margin`). The operator always finishes.
//!
//! The port is exact on primitives whose face order is Blender's (cube, cylinder, cone, torus,
//! icosphere, circle, monkey). A `primitive_uv_sphere_add` mesh has another face order in
//! frap (D-19), and the area sort that orders the groups breaks ties between equal-area faces
//! by face order, so smart-projecting a UV sphere gives another (valid) layout than Blender's:
//! `tests/golden/cases/uv/smart/uv_sphere.py` checks it at T3.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::smart_project::RotateMethod;
use frap_uv::{PackParams, RotationMethod};

use super::unwrap::margin_method_of;
use super::unwrap::uvedit::{self, PackTarget, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SmartProject) -> OpResult {
    uvedit::check_uvmap(cx)?;
    let angle_limit = p.angle_limit.value;
    let island_margin = p.island_margin.value;
    let area_weight = p.area_weight.value;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects(cx);

    let mut changed_obs = Vec::new();
    for &(ob, me) in &objects {
        if !uvedit::ensure_uvs(cx, me)? {
            continue;
        }
        let mut o = UvOb::gather(cx, &[(ob, me)]);
        let Some(mut o) = o.pop() else { continue };
        let bm = o.bm.clone();
        let thick: Vec<_> = uvedit::faces(&bm)
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(frap_bmesh::ElemFlag::SELECT))
            .collect();
        let normals: Vec<[f32; 3]> = thick.iter().map(|&f| bm.face(f).no).collect();
        let areas: Vec<f32> = thick.iter().map(|&f| uvedit::face_area(&bm, f)).collect();
        let mut changed = false;
        // Zero UVs on faces too small to project, so they do not overlap others.
        for (&f, &a) in thick.iter().zip(&areas) {
            if a.partial_cmp(&frap_uv::smart::AREA_IGNORE) != Some(core::cmp::Ordering::Greater) {
                for l in bm.face_loops(f) {
                    o.uvs[l.slot()] = [0.0, 0.0];
                    changed = true;
                }
            }
        }
        let groups = frap_uv::smart::project_groups(&normals, &areas, angle_limit, area_weight);
        if groups.is_empty() {
            continue;
        }
        for g in &groups {
            for &fi in &g.faces {
                let f = thick[fi as usize];
                let loops = bm.face_loops(f);
                let cos: Vec<[f32; 3]> = loops.iter().map(|&l| uvedit::loop_co(&bm, l)).collect();
                let mut out = vec![[0.0f32; 2]; loops.len()];
                frap_uv::smart::project_face(g.normal, &cos, &mut out);
                for (l, uv) in loops.iter().zip(out) {
                    o.uvs[l.slot()] = uv;
                }
                changed = true;
            }
        }
        if changed {
            changed_obs.push(o);
        }
    }

    if !changed_obs.is_empty() {
        let scene = cx.ctx().win.scene;
        {
            let (main, log) = cx.main_mut();
            if let Some(s) = main.scenes.get_mut(scene, log) {
                s.tool_settings.uvcalc_margin = island_margin;
            }
        }
        let base = uvedit::path_base(cx);
        let aspects: Vec<f32> = changed_obs
            .iter()
            .map(|o| uvedit::aspect_y(cx.main(), &base, o.ob, &o.bm))
            .collect();
        let mut params = PackParams::default();
        params.rotate_method = match p.rotate_method.value {
            RotateMethod::AxisAligned => RotationMethod::AxisAligned,
            RotateMethod::AxisAlignedX => RotationMethod::AxisAlignedX,
            RotateMethod::AxisAlignedY => RotationMethod::AxisAlignedY,
        };
        params.only_selected_uvs = false;
        params.only_selected_faces = true;
        params.correct_aspect = p.correct_aspect.value;
        params.use_seams = true;
        params.margin_method = margin_method_of(p.margin_method.value.identifier());
        params.margin = island_margin;
        uvedit::pack_islands_multi(
            &mut changed_obs,
            ts,
            &aspects,
            PackTarget::Udim,
            &mut params,
        );
        uvedit::clip_correct(
            cx.main(),
            &base,
            &mut changed_obs,
            ts,
            uvedit::clip_props(p.correct_aspect.value, false, p.scale_to_bounds.value),
            false,
            false,
        );
        for o in changed_obs {
            o.commit(cx)?;
        }
    }
    Ok(OpStatus::Finished)
}
