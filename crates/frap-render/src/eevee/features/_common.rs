// SPDX-License-Identifier: GPL-3.0-or-later
//! Helpers shared by the shading features (WP 5.5). A file with a leading underscore is no
//! feature: it registers nothing.

use frap_material::{ChannelSel, LoweredMaterial};
use frap_render_scene::{RMaterialSettings, RenderScene};

use crate::eevee::pipeline::MaterialParams;

/// The [`RMaterialSettings`] of `m` (the `Material` datablock values the lowered material does
/// not carry). The pipeline packs the materials of `scene.materials` (`for_material` receives
/// the `Arc`'s contents), so `m` is found by address; a material outside the scene (unit tests)
/// or a key without settings (synthetic scenes) reads the defaults.
pub fn settings(m: &LoweredMaterial, scene: &RenderScene) -> RMaterialSettings {
    scene
        .materials
        .iter()
        .find(|(_, lm)| std::ptr::eq(lm.as_ref(), m))
        .and_then(|(k, _)| scene.settings.material_settings.get(k))
        .cloned()
        .unwrap_or_default()
}

/// The component of an RGBA texture sample a scalar texture reads (`TexRef::channel`: R 0, G 1,
/// B 2, A 3), as an `f32` for [`MaterialParams::extra`]; colour selections read R.
pub fn channel(c: ChannelSel) -> f32 {
    match c {
        ChannelSel::G => 1.0,
        ChannelSel::B => 2.0,
        ChannelSel::A => 3.0,
        _ => 0.0,
    }
}

/// [`MaterialParams::extra`] slots of the features (row, column).
pub mod extra {
    /// Channel of the coat weight texture.
    pub const COAT_WEIGHT_CHANNEL: (usize, usize) = (0, 0);
    /// Channel of the coat roughness texture.
    pub const COAT_ROUGHNESS_CHANNEL: (usize, usize) = (0, 1);
    /// Channel of the sheen roughness texture.
    pub const SHEEN_ROUGHNESS_CHANNEL: (usize, usize) = (0, 2);
    /// Channel of the transmission texture.
    pub const TRANSMISSION_CHANNEL: (usize, usize) = (0, 3);
}

/// Sets one [`MaterialParams::extra`] slot.
pub fn set_extra(p: &mut MaterialParams, at: (usize, usize), v: f32) {
    p.extra[at.0][at.1] = v;
}
