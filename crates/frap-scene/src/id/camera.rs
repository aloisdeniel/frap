// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Camera` (§5.6: generated plain fields + hooks such as `angle` ↔ `lens`). The
//! field set is frozen at M1 W0 (decision log W0-21); WP 1.10 owns the bindings.

use frap_anim::AnimData;
use frap_rna::*;

use crate::generated::plain::CameraFields;

/// Camera data.
#[derive(Clone, Debug)]
pub struct Camera {
    /// ID header.
    pub header: IdHeader,
    /// The plain RNA fields (`type`, `lens`, `sensor_*`, `shift_*`, `clip_*`, `ortho_scale`,
    /// `dof`, …; generated, WP 1.1).
    pub fields: CameraFields,
    /// `Camera.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Camera {
    /// Camera data with the fresh values of `bpy.data.cameras.new(name)`.
    pub fn new(name: &str) -> Camera {
        Camera {
            header: IdHeader::new(name),
            fields: CameraFields::default(),
            animation_data: None,
        }
    }
}

/// `BKE_camera_sensor_size`: the sensor dimension the field of view is measured on.
fn sensor_size(c: &Camera) -> f32 {
    if c.fields.sensor_fit == "VERTICAL" {
        c.fields.sensor_height
    } else {
        c.fields.sensor_width
    }
}

/// `focallength_to_fov`
fn fov(lens: f32, sensor: f32) -> f32 {
    2.0 * ((sensor / 2.0) / lens).atan()
}

/// `fov_to_focallength`
fn focal_length(fov: f32, sensor: f32) -> f32 {
    (sensor / 2.0) / (fov * 0.5).tan()
}

// `angle`, `angle_x`, `angle_y` are derived from `lens` and the sensor (`rna_Camera_angle_*`).
crate::id::id_rna! { Camera as frap_spec_rna::CAMERA, "CAMERA" {
    angle => get(|c| fov(c.fields.lens, sensor_size(c)))
        set(|c, v: f32| c.fields.lens = focal_length(v, sensor_size(c))),
    angle_x => get(|c| fov(c.fields.lens, c.fields.sensor_width))
        set(|c, v: f32| c.fields.lens = focal_length(v, c.fields.sensor_width)),
    angle_y => get(|c| fov(c.fields.lens, c.fields.sensor_height))
        set(|c, v: f32| c.fields.lens = focal_length(v, c.fields.sensor_height)),
    type => fields.r#type,
    sensor_fit => fields.sensor_fit,
    passepartout_alpha => fields.passepartout_alpha,
    clip_start => fields.clip_start,
    clip_end => fields.clip_end,
    lens => fields.lens,
    sensor_width => fields.sensor_width,
    sensor_height => fields.sensor_height,
    ortho_scale => fields.ortho_scale,
    display_size => fields.display_size,
    shift_x => fields.shift_x,
    shift_y => fields.shift_y,
    show_limits => fields.show_limits,
    show_mist => fields.show_mist,
    show_passepartout => fields.show_passepartout,
    show_safe_areas => fields.show_safe_areas,
    show_safe_center => fields.show_safe_center,
    show_name => fields.show_name,
    show_sensor => fields.show_sensor,
    show_background_images => fields.show_background_images,
    lens_unit => fields.lens_unit,
    composition_guide_color => fields.composition_guide_color,
    show_composition_center => fields.show_composition_center,
    show_composition_center_diagonal => fields.show_composition_center_diagonal,
    show_composition_thirds => fields.show_composition_thirds,
    show_composition_golden => fields.show_composition_golden,
    show_composition_golden_tria_a => fields.show_composition_golden_tria_a,
    show_composition_golden_tria_b => fields.show_composition_golden_tria_b,
    show_composition_harmony_tri_a => fields.show_composition_harmony_tri_a,
    show_composition_harmony_tri_b => fields.show_composition_harmony_tri_b,
    panorama_type => fields.panorama_type,
    fisheye_fov => fields.fisheye_fov,
    fisheye_lens => fields.fisheye_lens,
    latitude_min => fields.latitude_min,
    latitude_max => fields.latitude_max,
    longitude_min => fields.longitude_min,
    longitude_max => fields.longitude_max,
    fisheye_polynomial_k0 => fields.fisheye_polynomial_k0,
    fisheye_polynomial_k1 => fields.fisheye_polynomial_k1,
    fisheye_polynomial_k2 => fields.fisheye_polynomial_k2,
    fisheye_polynomial_k3 => fields.fisheye_polynomial_k3,
    fisheye_polynomial_k4 => fields.fisheye_polynomial_k4,
    central_cylindrical_range_u_min => fields.central_cylindrical_range_u_min,
    central_cylindrical_range_u_max => fields.central_cylindrical_range_u_max,
    central_cylindrical_range_v_min => fields.central_cylindrical_range_v_min,
    central_cylindrical_range_v_max => fields.central_cylindrical_range_v_max,
    central_cylindrical_radius => fields.central_cylindrical_radius,
    custom_filepath => fields.custom_filepath,
    custom_shader => id(fields.custom_shader),
    custom_mode => fields.custom_mode,
    custom_bytecode => fields.custom_bytecode,
    custom_bytecode_hash => fields.custom_bytecode_hash,
    dof => struct(fields.dof),
}}

impl IdType for Camera {
    type Kind = CameraId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(t) = self.fields.custom_shader {
            f(t);
        }
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        if self.fields.custom_shader == Some(from) {
            self.fields.custom_shader = to;
        }
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
}
