// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.view3d.camera_to_view_selected()`, tier T1 (`tests/golden/cases/gltf/
//! lights_cameras/camera_fit*.py`). Moves the scene camera, keeping its rotation, so that the
//! selected objects fill its frame (orthographic cameras change `ortho_scale` instead of
//! moving along their view axis).
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `editors/space_view3d/view3d_view.cc` `view3d_camera_to_view_selected_exec`: the camera
//!   of the (scene-locked, D-10) view, i.e. `scene.camera`; none is the error
//!   `No active camera`;
//! - `editors/space_view3d/view3d_utils.cc` `view3d_camera_to_view_selected_impl`: the new
//!   world matrix is the camera's with the fitted location, applied with
//!   `BKE_object_apply_mat4(ob, mat, use_compat = true, use_parent = true)`, then the scale
//!   channels are restored (`OB_LOCK_SCALE | OB_LOCK_ROT4D`: rotation is re-derived);
//! - `blenkernel/intern/camera.cc` `camera_frame_fit_data_init`, `camera_to_frame_view_cb`,
//!   `camera_frame_fit_calc_from_data`, `BKE_camera_view_frame_fit_to_scene`,
//!   `BKE_camera_params_init/_from_object/_compute_viewplane/_compute_matrix` (resolution and
//!   pixel aspect of the scene, the resolution percentage ignored);
//! - `blenkernel/intern/scene.cc` `BKE_scene_foreach_display_point` and `object.cc`
//!   `BKE_object_foreach_display_point`: every vertex of the evaluated mesh of each selected
//!   visible object, in world space (objects without geometry, such as empties, cameras and
//!   lights, contribute nothing);
//! - `blenkernel/intern/mesh_wrapper.cc` `BKE_mesh_wrapper_vert_coords` and
//!   `mesh_data_update.cc` `editbmesh_calc_modifiers`: a mesh in Edit Mode evaluates to the
//!   `ME_WRAPPER_TYPE_BMESH` wrapper of the edit BMesh, whose display points are
//!   `edit_data->vert_positions`, empty until a modifier enabled in Edit Mode writes them (a
//!   deform modifier with `deform_verts_EM`) or converts the wrapper to mesh data (any other
//!   modifier). So an object whose mesh is in Edit Mode adds no point unless at least one of
//!   its modifiers ran in Edit Mode (`show_viewport` and `show_in_editmode`): entering Edit
//!   Mode on the factory cube and fitting is `CANCELLED` (verified against 5.2.4, golden
//!   `camera_fit_editmode.py`);
//! - `blenlib` `planes_from_projmat`, `isect_plane_plane_v3`, `isect_line_line_epsilon_v3`,
//!   `plane_from_point_normal_v3`, `normalize_v3`, `project_v3_v3v3`, `perspective_m4`,
//!   `orthographic_m4`.
//!
//! Fewer than two points (nothing or a single vertex selected) or degenerate frustum planes
//! give `CANCELLED` without a report, as in Blender. Poll `ED_operator_scene_editable`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CameraId, IdRef, LightId, MeshId, ObjectId};
use frap_scene::id::transform::{parent_world_matrix_in, world_matrix_in};
use frap_scene::{Context, EvaluatedObject, Main, ModifierStatus};
use frap_spec_ops_object::view3d;
use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::support::xform::apply_mat4;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, f: f32) -> V3 {
    [a[0] * f, a[1] * f, a[2] * f]
}
fn madd(r: &mut V3, a: V3, f: f32) {
    r[0] += a[0] * f;
    r[1] += a[1] * f;
    r[2] += a[2] * f;
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn len(a: V3) -> f32 {
    dot(a, a).sqrt()
}
fn xyz(p: &[f32; 4]) -> V3 {
    [p[0], p[1], p[2]]
}

/// `normalize_v3`: normalizes in place, returns the original length (0 and a zero vector
/// below `1e-35` squared length).
fn normalize(v: &mut V3) -> f32 {
    let d = dot(*v, *v);
    if d > 1.0e-35 {
        let d = d.sqrt();
        *v = mul(*v, 1.0 / d);
        d
    } else {
        *v = [0.0; 3];
        0.0
    }
}

/// `project_v3_v3v3`
fn project(p: V3, v_proj: V3) -> V3 {
    let m = dot(p, v_proj) / dot(v_proj, v_proj);
    mul(v_proj, m)
}

/// `plane_from_point_normal_v3`
fn plane_from_point_normal(co: V3, no: V3) -> [f32; 4] {
    [no[0], no[1], no[2], -dot(no, co)]
}

/// `isect_plane_plane_v3`: a point on the intersection line and its direction.
fn isect_plane_plane(a: &[f32; 4], b: &[f32; 4]) -> Option<(V3, V3)> {
    let c = cross(xyz(a), xyz(b));
    let det = dot(c, c);
    if det == 0.0 {
        return None;
    }
    let mut co = mul(cross(c, xyz(b)), a[3]);
    madd(&mut co, cross(xyz(a), c), b[3]);
    Some((mul(co, 1.0 / det), c))
}

/// `isect_line_line_v3` (`isect_line_line_epsilon_v3`, epsilon `1e-6`): the nearest points of
/// the lines `v1 v2` and `v3 v4`.
fn isect_line_line(v1: V3, v2: V3, v3: V3, v4: V3) -> Option<(V3, V3)> {
    let mut c = sub(v3, v1);
    let mut a = sub(v2, v1);
    let mut b = sub(v4, v3);
    let mut ab = cross(a, b);
    let d = dot(c, ab);
    let div = dot(ab, ab);
    if div == 0.0 {
        return None;
    }
    if d.abs() <= 0.000_001 {
        let cb = cross(c, b);
        a = mul(a, dot(cb, ab) / div);
        let i1 = add(v1, a);
        return Some((i1, i1));
    }
    let n = cross(a, b);
    let t = project(sub(v1, v3), n);
    let v3t = add(v3, t);
    let v4t = add(v4, t);
    c = sub(v3t, v1);
    a = sub(v2, v1);
    b = sub(v4t, v3t);
    ab = cross(a, b);
    let cb = cross(c, b);
    a = mul(a, dot(cb, ab) / dot(ab, ab));
    let i1 = add(v1, a);
    Some((i1, sub(i1, t)))
}

/// `CameraParams` (the members this operator reads).
#[derive(Clone, Debug, PartialEq)]
struct Params {
    is_ortho: bool,
    lens: f32,
    ortho_scale: f32,
    shiftx: f32,
    shifty: f32,
    sensor_x: f32,
    sensor_y: f32,
    sensor_fit: &'static str,
    clip_start: f32,
    clip_end: f32,
    /// `[xmin, xmax, ymin, ymax]`
    viewplane: [f32; 4],
    winmat: Mat4,
}

/// `BKE_camera_params_init` + `BKE_camera_params_from_object`.
fn params_from_object(main: &Main, ob: IdRef<ObjectId>) -> Params {
    let mut p = Params {
        is_ortho: false,
        lens: 0.0,
        ortho_scale: 0.0,
        shiftx: 0.0,
        shifty: 0.0,
        sensor_x: 36.0,
        sensor_y: 24.0,
        sensor_fit: "AUTO",
        clip_start: 0.1,
        clip_end: 100.0,
        viewplane: [0.0; 4],
        winmat: Mat4::ZERO,
    };
    let data = main.objects.get(ob).and_then(|o| o.data);
    let typ = main.objects.get(ob).map_or("EMPTY", |o| o.object_type());
    if let Some(cam) = data
        .and_then(|d| d.typed::<CameraId>())
        .and_then(|c| main.cameras.get(c))
    {
        let f = &cam.fields;
        p.is_ortho = f.r#type == "ORTHO";
        p.lens = f.lens;
        p.ortho_scale = f.ortho_scale;
        p.shiftx = f.shift_x;
        p.shifty = f.shift_y;
        p.sensor_x = f.sensor_width;
        p.sensor_y = f.sensor_height;
        p.sensor_fit = match f.sensor_fit.as_str() {
            "HORIZONTAL" => "HORIZONTAL",
            "VERTICAL" => "VERTICAL",
            _ => "AUTO",
        };
        p.clip_start = f.clip_start;
        p.clip_end = f.clip_end;
    } else if typ == "LIGHT" {
        let spot = data
            .and_then(|d| d.typed::<LightId>())
            .and_then(|l| main.lights.get(l))
            .map_or(0.0, |l| l.fields.spot_size);
        p.lens = 16.0 / (spot * 0.5).tan();
        if p.lens == 0.0 {
            p.lens = 35.0;
        }
    } else {
        p.lens = 35.0;
    }
    p.lens = p.lens.max(1e-9);
    p.ortho_scale = p.ortho_scale.max(1e-9);
    p.clip_end = p.clip_end.max(p.clip_start + 1e-3);
    p
}

/// `BKE_camera_sensor_size`
fn sensor_size(fit: &str, x: f32, y: f32) -> f32 {
    if fit == "VERTICAL" { y } else { x }
}

/// `BKE_camera_params_compute_viewplane` (no offset, zoom 1) and
/// `BKE_camera_params_compute_matrix`.
fn compute_viewplane_and_matrix(p: &mut Params, winx: i32, winy: i32, aspx: f32, aspy: f32) {
    let ycor = aspy / aspx;
    let mut pixsize = if p.is_ortho {
        p.ortho_scale
    } else {
        (sensor_size(p.sensor_fit, p.sensor_x, p.sensor_y) * p.clip_start) / p.lens
    };
    // BKE_camera_sensor_fit
    let horizontal = match p.sensor_fit {
        "AUTO" => aspx * winx as f32 >= aspy * winy as f32,
        f => f == "HORIZONTAL",
    };
    let viewfac = if horizontal {
        winx as f32
    } else {
        ycor * winy as f32
    };
    pixsize /= viewfac;
    let dx = p.shiftx * viewfac;
    let dy = p.shifty * viewfac;
    let vp = [
        -0.5 * winx as f32 + dx,
        0.5 * winx as f32 + dx,
        -0.5 * ycor * winy as f32 + dy,
        0.5 * ycor * winy as f32 + dy,
    ];
    p.viewplane = vp.map(|v| v * pixsize);
    let [l, r, b, t] = p.viewplane;
    let (n, f) = (p.clip_start, p.clip_end);
    let (xd, yd, zd) = (r - l, t - b, f - n);
    if xd == 0.0 || yd == 0.0 || zd == 0.0 {
        return;
    }
    p.winmat = if p.is_ortho {
        // orthographic_m4
        Mat4::from_cols(
            Vec4::new(2.0 / xd, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / yd, 0.0, 0.0),
            Vec4::new(0.0, 0.0, -2.0 / zd, 0.0),
            Vec4::new(-(r + l) / xd, -(t + b) / yd, -(f + n) / zd, 1.0),
        )
    } else {
        // perspective_m4
        Mat4::from_cols(
            Vec4::new(n * 2.0 / xd, 0.0, 0.0, 0.0),
            Vec4::new(0.0, n * 2.0 / yd, 0.0, 0.0),
            Vec4::new((r + l) / xd, (t + b) / yd, -(f + n) / zd, -1.0),
            Vec4::new(0.0, 0.0, (-2.0 * n * f) / zd, 0.0),
        )
    };
}

const Y_MIN: usize = 0;
const Y_MAX: usize = 1;
const Z_MIN: usize = 2;
const Z_MAX: usize = 3;

/// `CameraViewFrameData`
struct FrameData {
    plane_tx: [[f32; 4]; 4],
    dist_vals: [f32; 4],
    camera_no: V3,
    z_range: [f32; 2],
    tot: u32,
    do_zrange: bool,
    /// Columns of the normalized rotation part of the camera's world matrix.
    camera_rotmat: [V3; 3],
}

impl FrameData {
    /// `camera_frame_fit_data_init` (without clip distances).
    fn new(p: &Params, camera_world: &Mat4) -> FrameData {
        let mut rot = [
            camera_world.x_axis.truncate().to_array(),
            camera_world.y_axis.truncate().to_array(),
            camera_world.z_axis.truncate().to_array(),
        ];
        for c in &mut rot {
            normalize(c);
        }
        let r3 = Mat3::from_cols(Vec3::from(rot[0]), Vec3::from(rot[1]), Vec3::from(rot[2]));
        let rti = Mat4::from_mat3(r3).transpose().inverse();
        // planes_from_projmat: left, right, bottom, top = row 3 ± row 0 / row 1.
        let m = p.winmat.to_cols_array_2d();
        let plane = |row: usize, sign: f32| -> [f32; 4] {
            std::array::from_fn(|i| m[i][3] + sign * m[i][row])
        };
        let mut planes = [plane(0, 1.0), plane(0, -1.0), plane(1, 1.0), plane(1, -1.0)];
        for pl in &mut planes {
            let v = rti * Vec4::from_array(*pl);
            let mut n = [v.x, v.y, v.z];
            let l = normalize(&mut n);
            *pl = [n[0], n[1], n[2], v.w / l];
        }
        let do_zrange = p.is_ortho;
        FrameData {
            plane_tx: planes,
            dist_vals: [f32::MAX; 4],
            camera_no: mul(rot[2], -1.0),
            z_range: [f32::MAX, -f32::MAX],
            tot: 0,
            do_zrange,
            camera_rotmat: rot,
        }
    }

    /// `camera_to_frame_view_cb`
    fn add_point(&mut self, co: V3) {
        for (d, pl) in self.dist_vals.iter_mut().zip(&self.plane_tx) {
            let nd = dot(co, xyz(pl)) + pl[3];
            if *d > nd {
                *d = nd;
            }
        }
        if self.do_zrange {
            let d = dot(self.camera_no, co);
            if self.z_range[0] > d {
                self.z_range[0] = d;
            }
            if self.z_range[1] < d {
                self.z_range[1] = d;
            }
        }
        self.tot += 1;
    }

    /// `camera_frame_fit_calc_from_data`: the new camera location and orthographic scale.
    fn fit(&self, p: &Params) -> Option<(V3, f32)> {
        if self.tot <= 1 {
            return None;
        }
        let [ax, ay, az] = self.camera_rotmat;
        if p.is_ortho {
            let d = &self.dist_vals;
            let span_y = d[Y_MIN] + d[Y_MAX];
            let span_z = d[Z_MIN] + d[Z_MAX];
            let mid_y = (d[Y_MIN] - d[Y_MAX]) * 0.5;
            let mid_z = (d[Z_MIN] - d[Z_MAX]) * 0.5;
            let [xmin, xmax, ymin, ymax] = p.viewplane;
            let (size_x, size_y) = (xmax - xmin, ymax - ymin);
            let scale_diff = if span_z < span_y {
                span_z * (size_x / size_y)
            } else {
                span_y * (size_y / size_x)
            };
            let scale = p.ortho_scale - scale_diff;
            let mut co = [0.0f32; 3];
            madd(&mut co, ax, mid_y + (p.shiftx * scale_diff));
            madd(&mut co, ay, mid_z + (p.shifty * scale_diff));
            madd(&mut co, az, -(self.z_range[0] - 1.0 - p.clip_start));
            return Some((co, scale));
        }
        let mut planes = [[0.0f32; 4]; 4];
        for (i, pl) in planes.iter_mut().enumerate() {
            let n = xyz(&self.plane_tx[i]);
            *pl = plane_from_point_normal(mul(n, self.dist_vals[i]), n);
        }
        let (i1, mut n1) = isect_plane_plane(&planes[Y_MIN], &planes[Y_MAX])?;
        let (i2, mut n2) = isect_plane_plane(&planes[Z_MIN], &planes[Z_MAX])?;
        let (pt1, pt2) = isect_line_line(i1, add(i1, n1), i2, add(i2, n2))?;
        let shift_fac = sensor_size(p.sensor_fit, p.sensor_x, p.sensor_y) / p.lens;
        let cam_plane_no = mul(az, -1.0);
        let delta = sub(pt2, pt1);
        let delta_len = len(delta);
        let mut co;
        if dot(delta, cam_plane_no) > 0.0 {
            co = pt1;
            normalize(&mut n1);
            madd(&mut co, n1, p.shifty * delta_len * shift_fac);
        } else {
            co = pt2;
            normalize(&mut n2);
            madd(&mut co, n2, p.shiftx * delta_len * shift_fac);
        }
        Some((co, 1.0))
    }
}

/// `mul_v3_m4v3`
fn transform_point(m: &Mat4, v: V3) -> V3 {
    let c = m.to_cols_array_2d();
    std::array::from_fn(|i| c[0][i] * v[0] + c[1][i] * v[1] + c[2][i] * v[2] + c[3][i])
}

/// Whether the mesh of `ob` is in Edit Mode (its evaluation starts from the edit BMesh, as
/// `BKE_object_is_in_editmode` for every object sharing that mesh).
fn mesh_in_edit_mode(cx: &mut OpCtx, ob: IdRef<ObjectId>) -> bool {
    let (c, edit) = cx.split_edit();
    c.main
        .objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<MeshId>())
        .is_some_and(|me| edit.meshes.contains_key(&me.key()))
}

/// `BKE_mesh_wrapper_vert_coords` of an edit-mode evaluated mesh: the plain BMesh wrapper
/// (`edit_data->vert_positions` empty) has no points; it gains them once a modifier ran in Edit
/// Mode (deformed positions or mesh data). Skipped and disabled modifiers do not count.
fn edit_wrapper_has_points(ev: &EvaluatedObject) -> bool {
    ev.modifier_status.iter().any(|(_, s)| {
        matches!(
            s,
            ModifierStatus::Evaluated | ModifierStatus::PassedThrough | ModifierStatus::Error(_)
        )
    })
}

#[operator(poll = poll::SCENE_EDITABLE, tier = T1)]
fn exec(cx: &mut OpCtx, _p: view3d::CameraToViewSelected) -> OpResult {
    let (camera, selected, res) = {
        let c: Context = cx.ctx();
        let s = c.scene();
        let r = &s.render;
        (
            s.camera,
            c.selected_objects(),
            (
                r.resolution_x,
                r.resolution_y,
                r.pixel_aspect_x,
                r.pixel_aspect_y,
            ),
        )
    };
    let Some(camera) = camera.filter(|c| cx.main().objects.get(*c).is_some()) else {
        return Err(OpError::Runtime("No active camera".into()));
    };
    let mut params = params_from_object(cx.main(), camera);
    compute_viewplane_and_matrix(&mut params, res.0, res.1, res.2, res.3);
    let camera_world = world_matrix_in(cx.main(), camera);
    let mut data = FrameData::new(&params, &camera_world);

    // BKE_scene_foreach_display_point over the selected visible objects.
    for ob in selected {
        let in_edit_mode = mesh_in_edit_mode(cx, ob);
        let ev = cx.evaluated(ob)?;
        if in_edit_mode && !edit_wrapper_has_points(&ev) {
            continue;
        }
        if let Some(mesh) = &ev.mesh {
            for co in mesh.positions() {
                data.add_point(transform_point(&ev.matrix_world, *co));
            }
        }
    }
    let Some((co, scale)) = data.fit(&params) else {
        return Ok(OpStatus::Cancelled);
    };

    let (main, log) = cx.main_mut();
    let pw = parent_world_matrix_in(main, camera);
    let is_ortho_camera = params.is_ortho
        && main
            .objects
            .get(camera)
            .is_some_and(|o| o.object_type() == "CAMERA");
    if is_ortho_camera
        && let Some(cr) = main
            .objects
            .get(camera)
            .and_then(|o| o.data)
            .and_then(|d| d.typed::<CameraId>())
        && let Some(cam) = main.cameras.get_mut(cr, log)
    {
        cam.fields.ortho_scale = scale;
    }
    let mut obmat_new = camera_world;
    obmat_new.w_axis = Vec4::new(co[0], co[1], co[2], camera_world.w_axis.w);
    if let Some(o) = main.objects.get_mut(camera, log) {
        let (scale, dscale) = (o.scale, o.delta_scale);
        apply_mat4(o, &obmat_new, true, pw.as_ref());
        o.scale = scale;
        o.delta_scale = dscale;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_plane_and_line_line() {
        // x = 1 and y = 2 meet on the line (1, 2, t) along ±Z.
        let (co, no) = isect_plane_plane(&[1.0, 0.0, 0.0, -1.0], &[0.0, 1.0, 0.0, -2.0]).unwrap();
        assert!((co[0] - 1.0).abs() < 1e-6 && (co[1] - 2.0).abs() < 1e-6);
        assert_eq!(cross(no, [0.0, 0.0, 1.0]), [0.0, 0.0, 0.0]);
        assert!(isect_plane_plane(&[1.0, 0.0, 0.0, 0.0], &[2.0, 0.0, 0.0, 1.0]).is_none());
        // Skew lines: the X axis and the line through (0, 0, 1) along Y.
        let (a, b) =
            isect_line_line([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0]).unwrap();
        assert_eq!(a, [0.0, 0.0, 0.0]);
        assert_eq!(b, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn edit_mode_wrapper_points_need_a_modifier_run_in_edit_mode() {
        let ev = |st: Vec<ModifierStatus>| EvaluatedObject {
            matrix_world: Mat4::IDENTITY,
            mesh: None,
            bounds: None,
            pose: None,
            modifier_status: st.into_iter().map(|s| ("M".into(), s)).collect(),
            warnings: Vec::new(),
        };
        // No modifier, or only skipped / disabled ones: the plain BMesh wrapper, no points.
        assert!(!edit_wrapper_has_points(&ev(vec![])));
        assert!(!edit_wrapper_has_points(&ev(vec![
            ModifierStatus::Skipped,
            ModifierStatus::Disabled("no operand".into()),
        ])));
        // A modifier that ran in Edit Mode gives mesh data (or deformed edit positions).
        assert!(edit_wrapper_has_points(&ev(vec![
            ModifierStatus::Skipped,
            ModifierStatus::Evaluated,
        ])));
        assert!(edit_wrapper_has_points(&ev(vec![
            ModifierStatus::PassedThrough
        ])));
    }

    #[test]
    fn needs_two_points() {
        let mut p = Params {
            is_ortho: false,
            lens: 50.0,
            ortho_scale: 6.0,
            shiftx: 0.0,
            shifty: 0.0,
            sensor_x: 36.0,
            sensor_y: 24.0,
            sensor_fit: "AUTO",
            clip_start: 0.1,
            clip_end: 100.0,
            viewplane: [0.0; 4],
            winmat: Mat4::ZERO,
        };
        compute_viewplane_and_matrix(&mut p, 1920, 1080, 1.0, 1.0);
        let mut d = FrameData::new(&p, &Mat4::IDENTITY);
        d.add_point([0.0, 0.0, -5.0]);
        assert!(d.fit(&p).is_none());
        d.add_point([1.0, 1.0, -5.0]);
        let (co, _) = d.fit(&p).unwrap();
        // A camera looking down -Z frames the two points from above their middle.
        assert!(
            (co[0] - 0.5).abs() < 1e-4 && (co[1] - 0.5).abs() < 1e-4,
            "{co:?}"
        );
        assert!(co[2] > -5.0);
    }
}
