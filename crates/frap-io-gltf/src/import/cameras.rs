// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF cameras → `bpy.data.cameras` (M1: the static camera data; KHR_animation_pointer on
//! cameras arrives in M6, M3 WP 3.4 completes the camera/light mapping).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/camera.py` (`BlenderCamera.create`),
//! Apache-2.0, The glTF-Blender-IO authors. `Camera.angle_y` is set the way RNA's
//! `rna_Camera_angle_y_set` does (`lens = fov_to_focallength(angle, sensor_y)`,
//! blender/source/blender/makesrna/intern/rna_camera.cc, GPL-2.0-or-later).

use frap_gltf::json::CameraType;
use frap_rna::{CameraId, IdRef};
use frap_scene::id::Camera;
use frap_scene::{ChangeLog, Main};

use super::{Gltf, ImportError, extras};

/// RNA hard range of `Camera.clip_start`/`clip_end`.
const CLIP_MIN: f32 = 1e-6;

/// RNA hard range of `Camera.angle_y` (0.367° … 172.847°).
const ANGLE_MIN: f32 = 0.006_405_358;
const ANGLE_MAX: f32 = 3.016_749_1;

/// `fov_to_focallength` (BLI_math_rotation, f32).
fn fov_to_focallength(hfov: f32, sensor: f32) -> f32 {
    (sensor / 2.0) / libm::tanf(hfov * 0.5)
}

/// `BlenderCamera.create`: a new camera datablock for glTF camera `idx`.
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    idx: usize,
) -> Result<IdRef<CameraId>, ImportError> {
    let pycam = cx
        .doc
        .cameras
        .get(idx)
        .ok_or_else(|| ImportError::Data(format!("camera {idx} does not exist")))?;
    // `if not pycamera.name: pycamera.name = "Camera"`
    let name = super::truthy_name(&pycam.name).unwrap_or_else(|| "Camera".into());
    let mut cam = Camera::new("");
    extras::set_extras(&mut cam.header, pycam.extras.as_ref(), &[]);
    let f = &mut cam.fields;
    match pycam.type_ {
        CameraType::Orthographic => {
            if let Some(o) = &pycam.orthographic {
                f.r#type = "ORTHO".into();
                f.ortho_scale = ((o.xmag.max(o.ymag) * 2.0) as f32).max(0.0);
                f.clip_start = (o.znear as f32).max(CLIP_MIN);
                f.clip_end = (o.zfar as f32).max(CLIP_MIN);
            }
        }
        CameraType::Perspective => {
            if let Some(p) = &pycam.perspective {
                let yfov = (p.yfov as f32).clamp(ANGLE_MIN, ANGLE_MAX);
                f.lens = fov_to_focallength(yfov, f.sensor_height).max(1.0);
                f.lens_unit = "FOV".into();
                f.sensor_fit = "VERTICAL".into();
                f.clip_start = (p.znear as f32).max(CLIP_MIN);
                f.clip_end = p.zfar.map_or(1e12, |z| (z as f32).max(CLIP_MIN));
            }
        }
    }
    Ok(main.cameras.insert(&name, cam, log))
}
