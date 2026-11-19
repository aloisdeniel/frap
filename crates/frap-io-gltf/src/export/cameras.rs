// SPDX-License-Identifier: GPL-3.0-or-later
//! Cameras → glTF `cameras[]` (M3, WP 3.4).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/cameras.py` (`gather_camera`), `blender/com/conversion.py`
//! (`yvof_blender_to_gltf`) and `blender/exp/tree.py` (`recursive_node_traverse`: the camera
//! and light correction rotation, `node_filter_not_inheritable_is_kept`):
//!
//! - CAMERA objects are kept by the vtree filter only with `export_cameras` (default False);
//! - `PERSP` → `perspective` with `aspectRatio = (pixel_aspect_x · resolution_x) /
//!   (pixel_aspect_y · resolution_y)` of the context scene (the resolution percentage is
//!   ignored), `yfov` from `Camera.angle` (measured on the sensor `sensor_fit` names, AUTO =
//!   width) converted to a vertical angle unless the fit already is vertical (`VERTICAL`, or
//!   a portrait image with `AUTO`), `znear`/`zfar` = `clip_start`/`clip_end`;
//! - `ORTHO` → `orthographic` with `xmag`/`ymag` = `ortho_scale` · (side / longest side) / 2;
//! - `PANO` cameras have no glTF type: the node is written without `camera`;
//! - every value is computed in double precision from the float RNA values, as Python does;
//! - the camera is gathered once per camera datablock (`@cached`), so objects sharing data
//!   share the glTF camera;
//! - with `export_yup` the node's world matrix is post-multiplied by the rotation
//!   `Quaternion((√2/2, -√2/2, 0, 0))`, a -90° turn about X that undoes the Z-up → Y-up axis
//!   swap for the object's own axes, so the camera or light still looks down its local -Z
//!   with +Y up as glTF expects ([`correct_matrix_world`]); children of a camera or light are
//!   expressed relative to the corrected matrix.
//!
//! `extras` of the camera datablock (`export_extras`): `camera.py`'s `__gather_extras`, through
//! `extras::id` (WP 3.7).

use std::collections::HashMap;

use frap_gltf::json::{Camera as GCamera, CameraType, Index, Orthographic, Perspective};
use frap_math::glam::Mat4;
use frap_math::quat::quat_to_mat3;
use frap_rna::{CameraId, IdRef};
use frap_scene::generated::plain::RenderSettings;
use frap_scene::id::{Camera, Object};

use crate::export::gather::Gather;
use crate::export::vtree::matmul;
use crate::export::{ExportError, ExportSettings};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_cameras"];

/// Whether CAMERA objects are exported (`export_cameras`).
pub fn exported(settings: &ExportSettings) -> bool {
    settings.props.export_cameras.value
}

/// The add-on's correction of camera and light nodes: `Quaternion((2**0.5 / 2, -2**0.5 / 2,
/// 0.0, 0.0)).to_matrix().to_4x4()` with `export_yup` (a float quaternion turned into a matrix
/// by `quat_to_mat3`), the identity otherwise.
pub fn correction(export_yup: bool) -> Mat4 {
    if !export_yup {
        return Mat4::IDENTITY;
    }
    let h = (std::f64::consts::SQRT_2 / 2.0) as f32;
    Mat4::from_mat3(quat_to_mat3([h, -h, 0.0, 0.0]))
}

/// `node.matrix_world @= correction` for a kept CAMERA (with `export_cameras`) or LIGHT (with
/// `export_lights`) object; other objects keep `matrix_world`. Called by `vtree.rs` while it
/// builds the tree, so children of a camera or light are expressed relative to the corrected
/// matrix, as in Blender.
pub fn correct_matrix_world(settings: &ExportSettings, ob: &Object, matrix_world: Mat4) -> Mat4 {
    let corrected = match ob.object_type() {
        "CAMERA" => exported(settings),
        "LIGHT" => crate::export::lights::exported(settings),
        _ => false,
    };
    if corrected && settings.export_yup {
        matmul(&matrix_world, &correction(true))
    } else {
        matrix_world
    }
}

/// The glTF camera of each camera datablock already gathered (the add-on's `@cached`).
#[derive(Default)]
struct Cache(HashMap<IdRef<CameraId>, Option<Index>>);

/// `yvof_blender_to_gltf(angle, width, height, sensor_fit)`.
pub fn yfov_blender_to_gltf(angle: f64, width: f64, height: f64, sensor_fit: &str) -> f64 {
    let aspect_ratio = width / height;
    let horizontal = 2.0 * ((angle * 0.5).tan() / aspect_ratio).atan();
    if width >= height {
        if sensor_fit != "VERTICAL" {
            horizontal
        } else {
            angle
        }
    } else if sensor_fit != "HORIZONTAL" {
        angle
    } else {
        horizontal
    }
}

/// `Camera.angle` as RNA reads it (`rna_Camera_angle_get`: `focallength_to_fov(lens,
/// BKE_camera_sensor_size(...))` in float).
fn angle(cam: &Camera) -> f32 {
    let f = &cam.fields;
    let sensor = if f.sensor_fit == "VERTICAL" {
        f.sensor_height
    } else {
        f.sensor_width
    };
    // The platform `atanf`, as Blender (libm::atanf differs by one ulp on macOS).
    2.0 * ((sensor / 2.0) / f.lens).atan()
}

/// `gather_camera` for camera data `cam` with the context scene's render settings: `None`
/// for a panoramic camera.
pub fn camera_to_gltf(cam: &Camera, render: &RenderSettings) -> Option<GCamera> {
    let f = &cam.fields;
    let (type_, perspective, orthographic) = match f.r#type.as_str() {
        "PERSP" => {
            let width = f64::from(render.pixel_aspect_x) * f64::from(render.resolution_x);
            let height = f64::from(render.pixel_aspect_y) * f64::from(render.resolution_y);
            let p = Perspective {
                aspect_ratio: Some(width / height),
                yfov: yfov_blender_to_gltf(f64::from(angle(cam)), width, height, &f.sensor_fit),
                zfar: Some(f64::from(f.clip_end)),
                znear: f64::from(f.clip_start),
                ..Perspective::default()
            };
            (CameraType::Perspective, Some(p), None)
        }
        "ORTHO" => {
            let scene_x = f64::from(render.resolution_x) * f64::from(render.pixel_aspect_x);
            let scene_y = f64::from(render.resolution_y) * f64::from(render.pixel_aspect_y);
            let square = scene_x.max(scene_y);
            let scale = f64::from(f.ortho_scale);
            let o = Orthographic {
                xmag: scale * (scene_x / square) / 2.0,
                ymag: scale * (scene_y / square) / 2.0,
                zfar: f64::from(f.clip_end),
                znear: f64::from(f.clip_start),
                ..Orthographic::default()
            };
            (CameraType::Orthographic, None, Some(o))
        }
        _ => return None,
    };
    Some(GCamera {
        extensions: Default::default(),
        extras: None,
        name: Some(cam.header.name.to_string()),
        orthographic,
        perspective,
        type_,
    })
}

/// The glTF camera of a CAMERA object (`None` for other objects and panoramic cameras).
pub fn gather(g: &mut Gather, ob: &Object) -> Result<Option<Index>, ExportError> {
    if ob.object_type() != "CAMERA" {
        return Ok(None);
    }
    let Some(r) = ob.data.and_then(|d| d.typed::<CameraId>()) else {
        return Ok(None);
    };
    if let Some(i) = g.state::<Cache>().0.get(&r) {
        return Ok(*i);
    }
    let main = g.main;
    let Some(cam) = main.cameras.get(r) else {
        return Ok(None);
    };
    let ctx = g.ctx;
    let idx = camera_to_gltf(cam, &ctx.scene().render).map(|mut c| {
        c.extras = crate::export::extras::id(g, &cam.header);
        g.doc.cameras.push(c);
        (g.doc.cameras.len() - 1) as Index
    });
    g.state::<Cache>().0.insert(r, idx);
    Ok(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(x: i32, y: i32) -> RenderSettings {
        RenderSettings {
            resolution_x: x,
            resolution_y: y,
            ..RenderSettings::default()
        }
    }

    // Values dumped from Blender 5.2.4 (`export_scene.gltf(export_cameras=True)`).
    #[test]
    fn perspective_portrait_auto_keeps_angle() {
        let cam = Camera::new("Camera");
        let c = camera_to_gltf(&cam, &render(1000, 1500)).unwrap();
        let p = c.perspective.unwrap();
        assert_eq!(p.aspect_ratio, Some(0.6666666666666666));
        assert_eq!(p.yfov, 0.691111147403717);
        assert_eq!(p.znear, 0.10000000149011612);
    }

    #[test]
    fn perspective_vertical_fit() {
        let mut cam = Camera::new("Camera");
        cam.fields.sensor_fit = "VERTICAL".into();
        cam.fields.lens = 40.0;
        let c = camera_to_gltf(&cam, &render(1000, 1500)).unwrap();
        assert_eq!(c.perspective.unwrap().yfov, 0.5829135775566101);
    }

    #[test]
    fn orthographic_magnification() {
        let mut cam = Camera::new("Camera");
        cam.fields.r#type = "ORTHO".into();
        cam.fields.ortho_scale = 7.0;
        let c = camera_to_gltf(&cam, &render(1000, 1500)).unwrap();
        let o = c.orthographic.unwrap();
        assert_eq!((o.xmag, o.ymag), (2.333333333333333, 3.5));
        assert_eq!(c.type_, CameraType::Orthographic);
        cam.fields.r#type = "PANO".into();
        assert!(camera_to_gltf(&cam, &render(1000, 1500)).is_none());
    }

    #[test]
    fn yfov_rules() {
        let a = 0.8f64;
        let h = 2.0 * ((a * 0.5).tan() / 2.0).atan();
        assert_eq!(yfov_blender_to_gltf(a, 2.0, 1.0, "AUTO"), h);
        assert_eq!(yfov_blender_to_gltf(a, 2.0, 1.0, "HORIZONTAL"), h);
        assert_eq!(yfov_blender_to_gltf(a, 2.0, 1.0, "VERTICAL"), a);
        assert_eq!(yfov_blender_to_gltf(a, 1.0, 2.0, "AUTO"), a);
        assert_eq!(yfov_blender_to_gltf(a, 1.0, 2.0, "VERTICAL"), a);
        let h2 = 2.0 * ((a * 0.5).tan() / 0.5).atan();
        assert_eq!(yfov_blender_to_gltf(a, 1.0, 2.0, "HORIZONTAL"), h2);
    }

    #[test]
    fn correction_is_minus_90_about_x() {
        let m = correction(true);
        let v = m.transform_vector3(frap_math::glam::Vec3::Z);
        assert!((v - frap_math::glam::Vec3::Y).length() < 1e-6, "{v}");
        assert_eq!(correction(false), Mat4::IDENTITY);
    }
}
