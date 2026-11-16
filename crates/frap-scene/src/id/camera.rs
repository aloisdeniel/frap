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

crate::id::name_only_rna!(Camera, frap_spec_rna::CAMERA, CameraId);

impl IdType for Camera {
    type Kind = CameraId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
}
