// SPDX-License-Identifier: GPL-3.0-or-later
//! Render camera: a port of Blender's `BKE_camera_params_*` (sensor fit, shift, clip,
//! orthographic scale, pixel aspect) and of the render camera model matrix.
//!
//! Ported from blender/source/blender/blenkernel/intern/camera.cc (v5.2.4):
//! `BKE_camera_params_init`, `BKE_camera_params_from_object`, `BKE_camera_sensor_size`,
//! `BKE_camera_sensor_fit`, `BKE_camera_params_compute_viewplane`,
//! `BKE_camera_params_compute_matrix`, `BKE_camera_multiview_model_matrix` (which applies
//! `normalize_m4`); `perspective_m4` / `orthographic_m4` from blenlib `math_geom.cc`;
//! `BKE_render_resolution` from blenkernel `scene.cc`. The renderer calls them through
//! `RE_SetCamera` exactly as `Object.calc_matrix_camera` does, which is what
//! `tools/blender/dump_camera_cases.py` records for the tests.

use frap_math::matrix::normalize_v3;
use frap_scene::generated::plain::{CameraFields, RenderSettings};
use frap_scene::id::Light;
use glam::{Mat4, Vec4};

use crate::types::RCamera;

/// `DEFAULT_SENSOR_WIDTH`
pub const DEFAULT_SENSOR_WIDTH: f32 = 36.0;
/// `DEFAULT_SENSOR_HEIGHT`
pub const DEFAULT_SENSOR_HEIGHT: f32 = 24.0;

/// `Camera.sensor_fit` (`CAMERA_SENSOR_FIT_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SensorFit {
    /// AUTO: horizontal when the (pixel-aspect corrected) image is at least as wide as tall.
    Auto,
    /// HORIZONTAL
    Horizontal,
    /// VERTICAL
    Vertical,
}

impl SensorFit {
    /// From the RNA identifier (unknown values read as AUTO).
    pub fn from_rna(s: &str) -> SensorFit {
        match s {
            "HORIZONTAL" => SensorFit::Horizontal,
            "VERTICAL" => SensorFit::Vertical,
            _ => SensorFit::Auto,
        }
    }
}

/// `CameraParams`: the inputs of the view plane and projection.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CameraParams {
    /// Orthographic camera.
    pub is_ortho: bool,
    /// Focal length (mm).
    pub lens: f32,
    /// `ortho_scale`
    pub ortho_scale: f32,
    /// `shift_x`
    pub shift_x: f32,
    /// `shift_y`
    pub shift_y: f32,
    /// `sensor_width`
    pub sensor_x: f32,
    /// `sensor_height`
    pub sensor_y: f32,
    /// `sensor_fit`
    pub sensor_fit: SensorFit,
    /// `clip_start`
    pub clip_start: f32,
    /// `clip_end`
    pub clip_end: f32,
    /// Extra zoom factor (1 for renders).
    pub zoom: f32,
}

impl Default for CameraParams {
    /// `BKE_camera_params_init`, plus the 35 mm lens `BKE_camera_params_from_object` gives an
    /// object that is neither a camera nor a light.
    fn default() -> Self {
        CameraParams {
            is_ortho: false,
            lens: 35.0,
            ortho_scale: 0.0,
            shift_x: 0.0,
            shift_y: 0.0,
            sensor_x: DEFAULT_SENSOR_WIDTH,
            sensor_y: DEFAULT_SENSOR_HEIGHT,
            sensor_fit: SensorFit::Auto,
            clip_start: 0.1,
            clip_end: 100.0,
            zoom: 1.0,
        }
    }
}

impl CameraParams {
    /// `BKE_camera_params_from_object` for camera data. A panoramic camera is treated as a
    /// perspective one, as these functions do.
    pub fn from_camera(cam: &CameraFields) -> CameraParams {
        CameraParams {
            is_ortho: cam.r#type == "ORTHO",
            lens: cam.lens,
            ortho_scale: cam.ortho_scale,
            shift_x: cam.shift_x,
            shift_y: cam.shift_y,
            sensor_x: cam.sensor_width,
            sensor_y: cam.sensor_height,
            sensor_fit: SensorFit::from_rna(&cam.sensor_fit),
            clip_start: cam.clip_start,
            clip_end: cam.clip_end,
            zoom: 1.0,
        }
    }

    /// `BKE_camera_params_from_object` for a light used as `scene.camera`: the lens follows
    /// the spot cone (`la->spotsize`), clip range and sensor stay at their defaults.
    pub fn from_light(light: &Light) -> CameraParams {
        let fac = libm::cosf(light.fields.spot_size * 0.5);
        let phi = libm::acosf(fac);
        let mut lens = 16.0 * fac / libm::sinf(phi);
        if lens == 0.0 {
            lens = 35.0;
        }
        CameraParams {
            lens,
            ..CameraParams::default()
        }
    }
}

/// `BKE_camera_sensor_size`: the sensor dimension the fit uses (AUTO uses the width).
pub fn sensor_size(fit: SensorFit, sensor_x: f32, sensor_y: f32) -> f32 {
    if fit == SensorFit::Vertical {
        sensor_y
    } else {
        sensor_x
    }
}

/// `BKE_camera_sensor_fit`: resolves AUTO against the image size (pixel aspect applied).
pub fn sensor_fit(fit: SensorFit, size_x: f32, size_y: f32) -> SensorFit {
    match fit {
        SensorFit::Auto if size_x >= size_y => SensorFit::Horizontal,
        SensorFit::Auto => SensorFit::Vertical,
        f => f,
    }
}

/// The view plane `[xmin, xmax, ymin, ymax]` at `clip_start` (perspective) or in view units
/// (orthographic), and the pixel aspect `ycor = aspy / aspx`
/// (`BKE_camera_params_compute_viewplane`, no sub-pixel offset).
pub fn viewplane(p: &CameraParams, winx: i32, winy: i32, aspx: f32, aspy: f32) -> ([f32; 4], f32) {
    let ycor = aspy / aspx;
    let mut pixsize = if p.is_ortho {
        // "scale == 1.0 means exact 1 to 1 mapping"
        p.ortho_scale
    } else {
        (sensor_size(p.sensor_fit, p.sensor_x, p.sensor_y) * p.clip_start) / p.lens
    };
    let fit = sensor_fit(p.sensor_fit, aspx * winx as f32, aspy * winy as f32);
    let viewfac = if fit == SensorFit::Horizontal {
        winx as f32
    } else {
        ycor * winy as f32
    };
    pixsize /= viewfac;
    pixsize *= p.zoom;

    let mut xmin = -0.5 * winx as f32;
    let mut ymin = -0.5 * ycor * winy as f32;
    let mut xmax = 0.5 * winx as f32;
    let mut ymax = 0.5 * ycor * winy as f32;
    // Lens shift (offset_x/y are 0 outside tiled/border renders).
    let dx = p.shift_x * viewfac;
    let dy = p.shift_y * viewfac;
    xmin += dx;
    ymin += dy;
    xmax += dx;
    ymax += dy;
    (
        [
            xmin * pixsize,
            xmax * pixsize,
            ymin * pixsize,
            ymax * pixsize,
        ],
        ycor,
    )
}

/// `perspective_m4` (OpenGL clip depth, column-major).
pub fn perspective_m4(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) -> Mat4 {
    let xd = r - l;
    let yd = t - b;
    let zd = far - near;
    if xd == 0.0 || yd == 0.0 || zd == 0.0 {
        return Mat4::IDENTITY;
    }
    Mat4::from_cols(
        Vec4::new(near * 2.0 / xd, 0.0, 0.0, 0.0),
        Vec4::new(0.0, near * 2.0 / yd, 0.0, 0.0),
        Vec4::new((r + l) / xd, (t + b) / yd, -(far + near) / zd, -1.0),
        Vec4::new(0.0, 0.0, (-2.0 * near * far) / zd, 0.0),
    )
}

/// `orthographic_m4` (OpenGL clip depth, column-major).
pub fn orthographic_m4(l: f32, r: f32, b: f32, t: f32, near: f32, far: f32) -> Mat4 {
    let xd = r - l;
    let yd = t - b;
    let zd = far - near;
    if xd == 0.0 || yd == 0.0 || zd == 0.0 {
        return Mat4::IDENTITY;
    }
    Mat4::from_cols(
        Vec4::new(2.0 / xd, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 / yd, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -2.0 / zd, 0.0),
        Vec4::new(-(r + l) / xd, -(t + b) / yd, -(far + near) / zd, 1.0),
    )
}

/// Blender's `winmat` (OpenGL clip depth `[-1, 1]`): `BKE_camera_params_compute_viewplane` +
/// `BKE_camera_params_compute_matrix`.
pub fn window_matrix(p: &CameraParams, winx: i32, winy: i32, aspx: f32, aspy: f32) -> Mat4 {
    let ([l, r, b, t], _) = viewplane(p, winx, winy, aspx, aspy);
    if p.is_ortho {
        orthographic_m4(l, r, b, t, p.clip_start, p.clip_end)
    } else {
        perspective_m4(l, r, b, t, p.clip_start, p.clip_end)
    }
}

/// Maps OpenGL clip depth `[-w, w]` to wgpu's `[0, w]` (`z' = (z + w) / 2`).
pub const GL_TO_WGPU_DEPTH: Mat4 = Mat4::from_cols(
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 0.5, 0.0),
    Vec4::new(0.0, 0.0, 0.5, 1.0),
);

/// `normalize_m4`: the three axis columns normalized (their `w` divided by the length).
pub fn normalize_m4(m: &Mat4) -> Mat4 {
    let mut c = m.to_cols_array_2d();
    for col in c.iter_mut().take(3) {
        let (n, len) = normalize_v3([col[0], col[1], col[2]]);
        col[0] = n[0];
        col[1] = n[1];
        col[2] = n[2];
        if len != 0.0 {
            col[3] /= len;
        }
    }
    Mat4::from_cols_array_2d(&c)
}

/// World → view matrix of a camera object (`invert(normalize_m4(object_to_world))`).
pub fn view_matrix(camera_matrix_world: &Mat4) -> Mat4 {
    normalize_m4(camera_matrix_world).inverse()
}

/// `BKE_render_resolution(r, false)`: `resolution × percentage / 100` (integer division).
pub fn render_resolution(r: &RenderSettings) -> [i32; 2] {
    [
        r.resolution_x * r.resolution_percentage / 100,
        r.resolution_y * r.resolution_percentage / 100,
    ]
}

/// The render camera for `params` seen from `camera_matrix_world`, rendered at `winx × winy`
/// with pixel aspect `aspx : aspy`. `projection` has wgpu's `[0, 1]` depth range.
pub fn render_camera(
    params: &CameraParams,
    camera_matrix_world: &Mat4,
    winx: i32,
    winy: i32,
    aspx: f32,
    aspy: f32,
) -> RCamera {
    let winmat = window_matrix(params, winx, winy, aspx, aspy);
    RCamera {
        view: view_matrix(camera_matrix_world),
        projection: GL_TO_WGPU_DEPTH * winmat,
        is_perspective: !params.is_ortho,
        clip: [params.clip_start, params.clip_end],
        pixel_aspect: aspy / aspx,
    }
}
