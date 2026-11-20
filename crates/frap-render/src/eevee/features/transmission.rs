// SPDX-License-Identifier: GPL-3.0-or-later
//! Transmission (WP 5.5): the Principled BSDF's `Transmission Weight`
//! (`KHR_materials_transmission` in the lowered material; texture channel R, or the one a
//! Separate Color names) with `IOR`. The core splits the dielectric into reflection and
//! refraction (`bsdf_lut`, transmittance tinted by `sqrt(Base Color)`) and lights the
//! refraction closure:
//!
//! - by default (scene ray tracing off, or the material's `use_raytrace_refraction` off: both
//!   are Blender's defaults, verified on 5.2.4), with the world probe seen through the object
//!   (`refraction_probe` of the forward shader: two refraction events through EEVEE's sphere
//!   thickness model, the closure colour applied twice), so objects behind a glass do not show
//!   through it, as in Blender;
//! - with both on, with the opaque surfaces behind it (screen space,
//!   `MaterialParams::screen_refraction`).

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use super::_common::{channel, extra, set_extra, settings};
use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{MaterialParams, tex};

const WGSL: &str = r"
// Transmission (WP 5.5): weight `misc.x` × channel `extra[0].w` of `tex[2].y`.
fn feature_transmission(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    var w = material.misc.x;
    let t = material.tex[2].y;
    if (t >= 0) {
        w *= tex_sample(t, v)[u32(material.extra[0].w)];
    }
    (*s).transmission_weight = w;
}
";

/// Whether `m` transmits: a non-zero weight or a weight texture.
pub fn applies(m: &LoweredMaterial) -> bool {
    m.ext
        .transmission
        .as_ref()
        .is_some_and(|t| t.factor.factor != 0.0 || t.factor.texture.is_some())
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    let Some(tr) = &m.ext.transmission else {
        return;
    };
    p.transmission_weight = tr.factor.factor;
    if let Some(t) = &tr.factor.texture {
        p.set_texture(tex::TRANSMISSION, t, scene);
        set_extra(p, extra::TRANSMISSION_CHANNEL, channel(t.channel));
    }
    p.screen_refraction =
        scene.settings.eevee.use_raytracing && settings(m, scene).use_raytrace_refraction;
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "transmission",
    order: 50,
    wgsl: WGSL,
    applies,
    pack,
};
