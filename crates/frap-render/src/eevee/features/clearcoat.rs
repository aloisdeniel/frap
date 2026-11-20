// SPDX-License-Identifier: GPL-3.0-or-later
//! Clearcoat (WP 5.5): the Principled BSDF's coat layer (`KHR_materials_clearcoat` in the
//! lowered material): `Coat Weight` (texture channel R, or the one a Separate Color names),
//! `Coat Roughness` (G), `Coat Normal` (a tangent-space normal map; without one the coat uses
//! the interpolated normal, not the base layer's mapped one). The core lights the coat closure
//! (`node_bsdf_principled`: the coat's Fresnel reflectance takes its share of every layer
//! below and tints them by `Coat Tint` through the slab).
//!
//! The lowered material is glTF-shaped: `Coat IOR` and `Coat Tint` are not in it; they come
//! from `RMaterialSettings::{coat_ior, coat_tint}` (the Principled BSDF's unlinked values;
//! 1.5 and white when linked).

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use super::_common::{channel, extra, set_extra, settings};
use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{MaterialParams, tex};

const WGSL: &str = r"
// Clearcoat (WP 5.5): `material.coat` = weight, roughness, IOR, normal-map strength; textures
// `tex[1].x` (weight, channel `extra[0].x`), `tex[1].y` (roughness, `extra[0].y`), `tex[1].z`
// (coat normal, through `normal_map_perturb` of the normal-map feature).
fn feature_clearcoat(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    let t = material.tex[1];
    var w = material.coat.x;
    if (t.x >= 0) {
        w *= tex_sample(t.x, v)[u32(material.extra[0].x)];
    }
    var r = material.coat.y;
    if (t.y >= 0) {
        r *= tex_sample(t.y, v)[u32(material.extra[0].y)];
    }
    (*s).coat_weight = w;
    (*s).coat_roughness = r;
    (*s).coat_ior = material.coat.z;
    (*s).coat_tint = material.coat_tint.rgb;
    if (t.z >= 0) {
        (*s).coat_N = normal_map_perturb(tex_sample(t.z, v).rgb, material.coat.w, v, (*v).N);
    }
}
";

/// Whether `m` has a coat: a non-zero weight or a weight texture.
pub fn applies(m: &LoweredMaterial) -> bool {
    m.ext
        .clearcoat
        .as_ref()
        .is_some_and(|c| c.factor.factor != 0.0 || c.factor.texture.is_some())
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    let Some(c) = &m.ext.clearcoat else { return };
    p.coat_weight = c.factor.factor;
    p.coat_roughness = c.roughness.factor;
    let s = settings(m, scene);
    p.coat_ior = s.coat_ior;
    p.coat_tint = s.coat_tint;
    if let Some(t) = &c.factor.texture {
        p.set_texture(tex::COAT, t, scene);
        set_extra(p, extra::COAT_WEIGHT_CHANNEL, channel(t.channel));
    }
    if let Some(t) = &c.roughness.texture {
        p.set_texture(tex::COAT_ROUGHNESS, t, scene);
        set_extra(p, extra::COAT_ROUGHNESS_CHANNEL, channel(t.channel));
    }
    if let Some(n) = &c.normal {
        p.coat_normal_scale = n.scale;
        p.set_texture(tex::COAT_NORMAL, &n.texture, scene);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "clearcoat",
    order: 30,
    wgsl: WGSL,
    applies,
    pack,
};
