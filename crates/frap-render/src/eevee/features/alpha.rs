// SPDX-License-Identifier: GPL-3.0-or-later
//! Alpha blending and dithering (WP 5.5): a material whose `Alpha` is below 1 or linked
//! (`AlphaMode::Blend` in the lowered material) renders by its `surface_render_method`:
//!
//! - `'DITHERED'` (the default): hashed transparency in the opaque pass, each TAA sample
//!   discarding the fragment when its alpha is under the sample's blue-noise threshold, so the
//!   samples average to the blend (`BlendMode::Dithered`, the core's default for `Blend`);
//! - `'BLENDED'`: drawn after the opaque and refractive surfaces, objects sorted back to front
//!   by origin depth, premultiplied over what is behind, without depth writes
//!   (`BlendMode::Blended`).
//!
//! The alpha itself is the core's (`Alpha` × the base colour texture's alpha) times the vertex
//! colour's when the vertex-colour feature reads one; this feature only clamps it. Alpha clip
//! (`AlphaMode::Mask`) is the core's `BlendMode::Clip`. Not modelled: `use_transparency_overlap
//! = False` (Blender's per-object depth prepass, deviations.md §2).

use frap_material::{AlphaMode, LoweredMaterial};
use frap_render_scene::RenderScene;

use super::_common::settings;
use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{BlendMode, MaterialParams};

const WGSL: &str = r"
// Alpha blend / dither (WP 5.5): the blend mode is a material value (`material.ints.y`); the
// alpha the transparency reads is clamped to [0, 1].
fn feature_alpha(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    (*s).alpha = saturate((*s).alpha);
}
";

/// Whether `m` is transparent (`AlphaMode::Blend`).
pub fn applies(m: &LoweredMaterial) -> bool {
    m.alpha_mode == AlphaMode::Blend
}

/// The blend mode of a transparent material with `surface_render_method` `method`.
pub fn blend_mode(method: &str) -> BlendMode {
    if method == "BLENDED" {
        BlendMode::Blended
    } else {
        BlendMode::Dithered
    }
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    p.blend = blend_mode(&settings(m, scene).surface_render_method);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "alpha",
    order: 60,
    wgsl: WGSL,
    applies,
    pack,
};
