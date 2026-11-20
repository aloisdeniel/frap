// SPDX-License-Identifier: GPL-3.0-or-later
//! Emission (WP 5.5): the Principled BSDF's `Emission Color` × `Emission Strength`
//! (`emissiveFactor` × `emissiveTexture` × `KHR_materials_emissive_strength` in the lowered
//! material). The core adds it after the sheen and coat layers took their share
//! (`node_bsdf_principled`: `weight * coat_tint * emission * strength`).

use frap_material::LoweredMaterial;
use frap_render_scene::RenderScene;

use crate::eevee::EeveeFeature;
use crate::eevee::pipeline::{MaterialParams, tex};

const WGSL: &str = r"
// Emission (WP 5.5): colour (times its texture) and strength.
fn feature_emission(s: ptr<function, Surface>, v: ptr<function, Inputs>) {
    var e = material.emission.rgb;
    let t = material.tex[0].w;
    if (t >= 0) {
        e *= tex_sample(t, v).rgb;
    }
    (*s).emission = e;
    (*s).emission_strength = material.emission.w;
}
";

/// Whether `m` emits: a non-black factor or an emission texture and a non-zero strength (an
/// unlit material shows its base colour instead).
pub fn applies(m: &LoweredMaterial) -> bool {
    !m.unlit
        && m.emissive_strength != 0.0
        && (m.emissive.factor.iter().any(|&c| c != 0.0) || m.emissive.texture.is_some())
}

fn pack(m: &LoweredMaterial, scene: &RenderScene, p: &mut MaterialParams) {
    p.emission = m.emissive.factor;
    p.emission_strength = m.emissive_strength;
    if let Some(t) = &m.emissive.texture {
        p.set_texture(tex::EMISSION, t, scene);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::eevee::EEVEE_FEATURES)]
static FEATURE: EeveeFeature = EeveeFeature {
    name: "emission",
    order: 40,
    wgsl: WGSL,
    applies,
    pack,
};
