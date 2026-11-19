// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.project_from_view(orthographic=False, camera_bounds=True, correct_aspect=True,
//! clip_to_bounds=False, scale_to_bounds=False)`, tier T1 (`tests/golden/cases/uv/project/
//! from_view_*.py`). Port of `uv_from_view_exec` (`editors/uvedit/uvedit_unwrap_ops.cc`) and
//! `BKE_uvproject_camera_info` / `BKE_uvproject_from_camera` /
//! `BKE_uvproject_from_view_ortho` (`blenkernel/intern/uvproject.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Blender projects "as seen in the current 3D view": through the view's camera when the
//! view looks through it, otherwise through the interactive viewport, which a headless run
//! does not have (`poll()` fails in `-b`). **frap extension:** the projection goes through
//! `scene.camera` (as if the virtual 3D View, D-10, looked through it): perspective,
//! orthographic and panoramic cameras, shifts, and with `camera_bounds` the render
//! resolution and pixel aspect (`resolution_x * pixel_aspect_x`, …). With
//! `orthographic=True` the selected faces are projected orthographically along the virtual
//! view direction (its factory Layout matrix) around the mean location of the edited
//! objects, as Blender does. Without `orthographic` and without a camera object as
//! `scene.camera`, the operator fails like the poll (`OP_POLL_FAILED`, with the fix as hint).
//! Vertex positions are the edit-mesh coordinates (Blender uses the deformed ones when a
//! modifier deforms in Edit Mode). Then per-face aspect correction and clipping/scaling.
//! `CANCELLED` when no mesh had a selected face.

use frap_bmesh::ElemFlag;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CameraId, IdRef, ObjectId};
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb, UvTs};

/// `ProjCameraInfo`.
#[derive(Copy, Clone, Debug)]
struct CameraInfo {
    camangle: f32,
    camsize: f32,
    xasp: f32,
    yasp: f32,
    shiftx: f32,
    shifty: f32,
    rotmat: glam::Mat4,
    caminv: glam::Mat4,
    do_persp: bool,
    do_pano: bool,
}

/// `BKE_uvproject_camera_info(ob, rotmat, winx, winy)`; `None` when the camera matrix is
/// singular.
fn camera_info(
    cam: &frap_scene::generated::plain::CameraFields,
    cam_world: glam::Mat4,
    rotmat: glam::Mat4,
    winx: f32,
    winy: f32,
) -> Option<CameraInfo> {
    let do_pano = cam.r#type == "PANO";
    let do_persp = matches!(cam.r#type.as_str(), "PERSP" | "CUSTOM");
    // `focallength_to_fov(lens, sensor_x) / 2`.
    let camangle = 2.0 * libm::atanf((cam.sensor_width / 2.0) / cam.lens) / 2.0;
    let camsize = if do_persp {
        libm::tanf(camangle)
    } else {
        cam.ortho_scale
    };
    // `normalize_m4` (the three axes) then `invert_m4`.
    let mut m = cam_world;
    m.x_axis = m.x_axis.truncate().normalize_or_zero().extend(m.x_axis.w);
    m.y_axis = m.y_axis.truncate().normalize_or_zero().extend(m.y_axis.w);
    m.z_axis = m.z_axis.truncate().normalize_or_zero().extend(m.z_axis.w);
    if m.determinant() == 0.0 {
        return None;
    }
    let caminv = m.inverse();
    let (xasp, yasp) = if winx > winy {
        (1.0, winx / winy)
    } else {
        (winy / winx, 1.0)
    };
    Some(CameraInfo {
        camangle,
        camsize,
        xasp,
        yasp,
        shiftx: 0.5 - (cam.shift_x * xasp),
        shifty: 0.5 - (cam.shift_y * yasp),
        rotmat,
        caminv,
        do_persp,
        do_pano,
    })
}

/// `mul_m4_v4` in Blender's summation order.
fn mul_m4_v4(m: &glam::Mat4, v: [f32; 4]) -> [f32; 4] {
    let c = m.to_cols_array_2d();
    core::array::from_fn(|i| v[0] * c[0][i] + v[1] * c[1][i] + v[2] * c[2][i] + v[3] * c[3][i])
}

/// `BKE_uvproject_from_camera`.
fn from_camera(co: [f32; 3], uci: &CameraInfo) -> [f32; 2] {
    let mut pv4 = [co[0], co[1], co[2], 1.0];
    pv4 = mul_m4_v4(&uci.rotmat, pv4);
    pv4 = mul_m4_v4(&uci.caminv, pv4);
    let mut t = [0.0f32; 2];
    if uci.do_pano {
        let angle = libm::atan2f(pv4[0], -pv4[2]) / (core::f32::consts::PI * 2.0);
        if !uci.do_persp {
            t = [angle, pv4[1] / uci.camsize];
        } else {
            let len = (pv4[0] * pv4[0] + pv4[2] * pv4[2]).sqrt();
            t = [
                angle * (core::f32::consts::PI / uci.camangle),
                pv4[1] / (len * (uci.camsize * 2.0)),
            ];
        }
    } else {
        if pv4[2] == 0.0 {
            pv4[2] = 0.00001;
        }
        if !uci.do_persp {
            t = [pv4[0] / uci.camsize, pv4[1] / uci.camsize];
        } else {
            t[0] = (-pv4[0] * ((1.0 / uci.camsize) / pv4[2])) / 2.0;
            t[1] = (-pv4[1] * ((1.0 / uci.camsize) / pv4[2])) / 2.0;
        }
    }
    t[0] *= uci.xasp;
    t[1] *= uci.yasp;
    [t[0] + uci.shiftx, t[1] + uci.shifty]
}

/// The camera object and data the projection goes through (`scene.camera`, a camera).
fn scene_camera(cx: &OpCtx) -> Option<(IdRef<ObjectId>, IdRef<CameraId>)> {
    let c = cx.ctx();
    let cam = c.scene().camera?;
    let data = c.main.objects.get(cam)?.data?.typed::<CameraId>()?;
    c.main.cameras.get(data)?;
    Some((cam, data))
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    support = HeadlessExtended(
        "projects through scene.camera (Blender: the 3D View's camera when the view looks \
         through it); orthographic=True projects along the virtual 3D View"
    )
)]
fn exec(cx: &mut OpCtx, p: uv::ProjectFromView) -> OpResult {
    uvedit::check_uvmap(cx)?;
    let ts = UvTs::of(&cx.ctx());
    let orthographic = p.orthographic.value;
    let camera = scene_camera(cx);
    if !orthographic && camera.is_none() {
        return Err(uvedit::poll_failed(
            cx,
            "frap projects from the scene camera: set bpy.context.scene.camera to a camera \
             object (e.g. bpy.ops.object.camera_add()), or pass orthographic=True",
        ));
    }
    let objects = uvedit::edit_objects(cx);
    let view = cx.ctx().win.view3d.view_matrix.to_cols_array_2d();
    let mut offset = [0.0f32; 4];
    if orthographic {
        let mut avg = [0.0f32; 4];
        for &(ob, _) in &objects {
            let m = uvedit::object_to_world(cx.main(), ob);
            for k in 0..4 {
                avg[k] += m[3][k];
            }
        }
        let inv = 1.0 / objects.len() as f32;
        offset = avg.map(|a| -(a * inv));
    }
    let (winx, winy) = {
        let r = &cx.ctx().scene().render;
        if p.camera_bounds.value {
            (
                r.resolution_x as f32 * r.pixel_aspect_x,
                r.resolution_y as f32 * r.pixel_aspect_y,
            )
        } else {
            (1.0, 1.0)
        }
    };
    let mut gathered = Vec::new();
    let mut changed_any = false;
    for &(ob, me) in &objects {
        if !uvedit::ensure_uvs(cx, me)? {
            continue;
        }
        let Some(mut o) = UvOb::gather(cx, &[(ob, me)]).pop() else {
            continue;
        };
        let bm = o.bm.clone();
        let obmat = uvedit::object_to_world(cx.main(), ob);
        let selected: Vec<_> = uvedit::faces(&bm)
            .into_iter()
            .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
            .collect();
        if orthographic {
            let rotmat = uvedit::rotation_matrix_ex(&view, &obmat, 90.0, 0.0, 1.0, offset);
            for &f in &selected {
                for l in bm.face_loops(f) {
                    let co = uvedit::loop_co(&bm, l);
                    // `BKE_uvproject_from_view_ortho`: `mul_v3_m4v3`, then (-x, z).
                    let pv: [f32; 3] = core::array::from_fn(|i| {
                        rotmat[0][i] * co[0]
                            + rotmat[1][i] * co[1]
                            + rotmat[2][i] * co[2]
                            + rotmat[3][i]
                    });
                    o.uvs[l.slot()] = [-pv[0], pv[2]];
                }
            }
        } else if let Some((cam_ob, cam_data)) = camera {
            let main = cx.main();
            let cam = &main.cameras.get(cam_data).expect("checked above").fields;
            let cam_world = frap_scene::id::transform::world_matrix(&main.objects, cam_ob);
            let rot = glam::Mat4::from_cols_array_2d(&obmat);
            if let Some(uci) = camera_info(cam, cam_world, rot, winx, winy) {
                for &f in &selected {
                    for l in bm.face_loops(f) {
                        o.uvs[l.slot()] = from_camera(uvedit::loop_co(&bm, l), &uci);
                    }
                }
            } else {
                gathered.push(o);
                continue;
            }
        }
        changed_any |= !selected.is_empty();
        gathered.push(o);
    }
    if !changed_any {
        return Ok(OpStatus::Cancelled);
    }
    let base = uvedit::path_base(cx);
    uvedit::clip_correct(
        cx.main(),
        &base,
        &mut gathered,
        ts,
        uvedit::clip_props(
            p.correct_aspect.value,
            p.clip_to_bounds.value,
            p.scale_to_bounds.value,
        ),
        true,
        false,
    );
    for o in gathered {
        o.commit(cx)?;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_projection_centres_the_view_axis() {
        let cam = frap_scene::generated::plain::CameraFields {
            r#type: "PERSP".into(),
            ..Default::default()
        };
        let uci = camera_info(
            &cam,
            glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, 10.0)),
            glam::Mat4::IDENTITY,
            1920.0,
            1080.0,
        )
        .expect("invertible");
        // A point on the view axis lands in the middle.
        let uv = from_camera([0.0, 0.0, 0.0], &uci);
        assert!((uv[0] - 0.5).abs() < 1e-6 && (uv[1] - 0.5).abs() < 1e-6);
        // Landscape: v is stretched by the aspect.
        assert!((uci.yasp - 1920.0 / 1080.0).abs() < 1e-6);
    }
}
