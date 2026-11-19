// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.sphere_project(direction='VIEW_ON_EQUATOR', align='POLAR_ZX', pole='PINCH',
//! seam=False, correct_aspect=True, clip_to_bounds=False, scale_to_bounds=False)`, tier T1
//! (`tests/golden/cases/uv/project/sphere_*.py`). Port of `sphere_project_exec`,
//! `uv_map_transform`, `uv_map_transform_center` and `uv_map_clip_correct`
//! (`editors/uvedit/uvedit_unwrap_ops.cc`), Blender 5.2.4, GPL-2.0-or-later; the projection
//! walk (`uv_sphere_project`, `uv_map_mirror`) is `frap_uv::project::curved_project`.
//!
//! Poll `ED_operator_uvmap`. For every mesh in Edit Mode with a selected face (a UV map is
//! added when missing), the selected faces are mapped onto a sphere around the transform
//! pivot (`tool_settings.transform_pivot_point`). `VIEW_ON_EQUATOR` / `VIEW_ON_POLES` orient
//! it by the view matrix of frap's virtual 3D View (D-10: the factory Layout viewport, as
//! Blender's `-b` VIEW_3D sees it) and the object rotation; `ALIGN_TO_OBJECT` by the object
//! alone. `pole='FAN'` spreads pole UVs, `seam=True` keeps islands cut by seams together
//! across the wrap. Then per-face aspect correction and clipping/scaling. Always finishes.
//!
//! [`curved_exec`] is shared with `uv.cylinder_project`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_uv::project::{CurvedProjection, ProjectAlign, ProjectDirection};

use super::unwrap::uvedit::{self, BmProject, ClipCorrect, UvOb, UvTs};

/// The `uv_transform_properties` of a curved projection.
pub(crate) struct Curved<'a> {
    /// Sphere or cylinder.
    pub kind: CurvedProjection,
    /// `direction` identifier.
    pub direction: &'a str,
    /// `align` identifier.
    pub align: &'a str,
    /// `pole == 'FAN'`.
    pub fan: bool,
    /// `seam`
    pub seam: bool,
    /// `radius` (1 for the sphere, which has none).
    pub radius: f32,
    /// The clip/correct properties.
    pub clip: ClipCorrect,
}

/// `sphere_project_exec` / `cylinder_project_exec`.
pub(crate) fn curved_exec(cx: &mut OpCtx, c: &Curved) -> OpResult {
    uvedit::check_uvmap(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let direction = match c.direction {
        "VIEW_ON_POLES" => ProjectDirection::ViewOnPoles,
        "ALIGN_TO_OBJECT" => ProjectDirection::AlignToObject,
        _ => ProjectDirection::ViewOnEquator,
    };
    let align = match c.align {
        "POLAR_ZY" => ProjectAlign::PolarZy,
        _ => ProjectAlign::PolarZx,
    };
    let view = cx.ctx().win.view3d.view_matrix.to_cols_array_2d();
    for (ob, me) in uvedit::edit_objects(cx) {
        if !uvedit::edit_bm(cx, me).is_some_and(|bm| uvedit::has_face_selection(&bm)) {
            continue;
        }
        if !uvedit::ensure_uvs(cx, me)? {
            continue;
        }
        let Some(mut o) = UvOb::gather(cx, &[(ob, me)]).pop() else {
            continue;
        };
        let obmat = uvedit::object_to_world(cx.main(), ob);
        let rotmat =
            frap_uv::project::uv_map_transform(direction, align, c.radius, Some(&view), &obmat);
        let bm = o.bm.clone();
        let (center, _) = uvedit::transform_center(&cx.ctx(), ob, &bm);
        let walk = BmProject::new(&bm);
        let mut corners = walk.gather(&o.uvs);
        frap_uv::project::curved_project(
            &walk,
            c.kind,
            center,
            &rotmat,
            c.fan,
            c.seam,
            &mut corners,
        );
        walk.scatter(&corners, &mut o.uvs);
        let base = uvedit::path_base(cx);
        let mut one = [o];
        uvedit::clip_correct(cx.main(), &base, &mut one, ts, c.clip, true, false);
        let [o] = one;
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::SphereProject) -> OpResult {
    curved_exec(
        cx,
        &Curved {
            kind: CurvedProjection::Sphere,
            direction: p.direction.value.identifier(),
            align: p.align.value.identifier(),
            fan: p.pole.value.identifier() == "FAN",
            seam: p.seam.value,
            radius: 1.0,
            clip: uvedit::clip_props(
                p.correct_aspect.value,
                p.clip_to_bounds.value,
                p.scale_to_bounds.value,
            ),
        },
    )
}
