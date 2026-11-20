// SPDX-License-Identifier: GPL-3.0-or-later
//! Sheen (WP 5.5): the Principled BSDF's sheen layer (`KHR_materials_sheen` in the lowered
//! material): `Sheen Tint` (RGB, times its texture) and `Sheen Roughness` (texture channel A,
//! or the one a Separate Color names). The core evaluates it as EEVEE does (`principled_sheen`
//! albedo fit, added to the diffuse closure, every layer below scaled by `1 − max(sheen)`).
//!
//! The lowered material is glTF-shaped: it has a sheen only when `Sheen Weight` is not 0 and
//! keeps no weight; the weight comes from `RMaterialSettings::sheen_weight` (the Principled
//! BSDF's unlinked value; 1 when linked).

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use super::_common::{channel, extra, set_extra, settings};
use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{MaterialParams, tex};

const WGSL: &str = r"
// Sheen (WP 5.5): weight `misc.y`, tint `sheen.rgb` × `tex[1].w`, roughness `sheen.w` ×
// channel `extra[0].z` of `tex[2].x`.
fn feature_sheen(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    var tint = material.sheen.rgb;
    let tc = material.tex[1].w;
    if (tc >= 0) {
        tint *= tex_sample(tc, v).rgb;
    }
    var r = material.sheen.w;
    let tr = material.tex[2].x;
    if (tr >= 0) {
        r *= tex_sample(tr, v)[u32(material.extra[0].z)];
    }
    (*s).sheen_weight = material.misc.y;
    (*s).sheen_tint = tint;
    (*s).sheen_roughness = r;
}
";

/// Whether `m` has a sheen layer.
pub fn applies(m: &LoweredMaterial) -> bool {
    m.ext.sheen.is_some()
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    let Some(sh) = &m.ext.sheen else { return };
    p.sheen_weight = settings(m, scene).sheen_weight;
    p.sheen_tint = sh.color.factor;
    p.sheen_roughness = sh.roughness.factor;
    if let Some(t) = &sh.color.texture {
        p.set_texture(tex::SHEEN_COLOR, t, scene);
    }
    if let Some(t) = &sh.roughness.texture {
        p.set_texture(tex::SHEEN_ROUGHNESS, t, scene);
        set_extra(p, extra::SHEEN_ROUGHNESS_CHANNEL, channel(t.channel));
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "sheen",
    order: 35,
    wgsl: WGSL,
    applies,
    pack,
};
