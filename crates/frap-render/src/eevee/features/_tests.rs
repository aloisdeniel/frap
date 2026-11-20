// SPDX-License-Identifier: GPL-3.0-or-later
//! Unit tests of the shading features (WP 5.5): registration, `applies`, `pack` and the WGSL
//! contract. The rendered look of each feature is pinned by the render goldens of
//! `tests/golden/cases/render_eevee/features/` (`frap-ops-render-tests`, `eevee_features`:
//! frap self-goldens and Blender 5.2.4 references).
#![cfg(test)]

use std::sync::Arc;

use frap_material::{
    AlphaMode, Channel, ChannelSel, Clearcoat, LoweredMaterial, Sheen, Transmission,
    VertexColorUse,
};
use frap_render_scene::{MatKey, RMaterialSettings, RenderScene};

use super::_common::{channel, settings};
use super::{alpha, vertex_color};
use crate::eevee::pipeline::{BlendMode, MaterialParams, forward_source};

/// A plain dielectric (no feature applies).
fn plain() -> LoweredMaterial {
    let mut m = LoweredMaterial::gltf_default();
    m.base_color.factor = [0.8, 0.8, 0.8, 1.0];
    m.metallic.factor = 0.0;
    m.roughness.factor = 0.5;
    m
}

/// The flag bit of a feature.
fn bit(name: &str) -> u32 {
    let i = crate::eevee::features()
        .iter()
        .position(|f| f.name == name)
        .unwrap_or_else(|| panic!("feature {name} is not registered"));
    1 << i
}

fn params(m: &LoweredMaterial) -> MaterialParams {
    MaterialParams::for_material(m, &RenderScene::default())
}

#[test]
fn every_feature_is_registered_with_its_wgsl() {
    let src = forward_source();
    for name in [
        "vertex_color",
        "normal_map",
        "clearcoat",
        "sheen",
        "emission",
        "transmission",
        "alpha",
    ] {
        let b = bit(name);
        assert!(src.contains(&format!("fn feature_{name}(")), "{name}");
        assert!(
            src.contains(&format!("& {b}u) != 0u) {{ feature_{name}(s, v); }}")),
            "{name}: dispatcher"
        );
    }
    // The coat normal reuses the normal map's perturbation.
    assert!(src.contains("fn normal_map_perturb("));
    // A plain material sets no bit and keeps the core values.
    let p = params(&plain());
    assert_eq!(p.flags & !bit("test_tint"), 0);
    assert_eq!((p.emission_strength, p.coat_weight), (0.0, 0.0));
    assert_eq!((p.sheen_weight, p.transmission_weight), (0.0, 0.0));
    assert_eq!(p.blend, BlendMode::Opaque);
}

#[test]
fn emission_packs_colour_and_strength() {
    let mut m = plain();
    m.emissive.factor = [0.5, 0.25, 0.0];
    m.emissive_strength = 6.0;
    let p = params(&m);
    assert_ne!(p.flags & bit("emission"), 0);
    assert_eq!((p.emission, p.emission_strength), ([0.5, 0.25, 0.0], 6.0));
    assert_eq!(p.gpu().emission, [0.5, 0.25, 0.0, 6.0]);
    // Black, zero strength or unlit: no emission.
    m.emissive_strength = 0.0;
    assert_eq!(params(&m).flags & bit("emission"), 0);
    m.emissive_strength = 1.0;
    m.unlit = true;
    assert_eq!(params(&m).flags & bit("emission"), 0);
}

#[test]
fn clearcoat_sheen_transmission_pack_their_factors() {
    let mut m = plain();
    m.ext.clearcoat = Some(Clearcoat {
        factor: Channel {
            factor: 0.75,
            texture: None,
        },
        roughness: Channel {
            factor: 0.1,
            texture: None,
        },
        normal: None,
    });
    m.ext.sheen = Some(Sheen {
        color: Channel {
            factor: [1.0, 0.7, 0.8],
            texture: None,
        },
        roughness: Channel {
            factor: 0.3,
            texture: None,
        },
    });
    m.ext.transmission = Some(Transmission {
        factor: Channel {
            factor: 1.0,
            texture: None,
        },
    });
    let p = params(&m);
    for f in ["clearcoat", "sheen", "transmission"] {
        assert_ne!(p.flags & bit(f), 0, "{f}");
    }
    assert_eq!((p.coat_weight, p.coat_roughness, p.coat_ior), (0.75, 0.1, 1.5));
    assert_eq!(p.coat_tint, [1.0; 3]);
    assert_eq!(
        (p.sheen_weight, p.sheen_tint, p.sheen_roughness),
        (1.0, [1.0, 0.7, 0.8], 0.3)
    );
    assert_eq!(p.transmission_weight, 1.0);
    let g = p.gpu();
    assert_eq!(g.coat, [0.75, 0.1, 1.5, 1.0]);
    assert_eq!(g.sheen, [1.0, 0.7, 0.8, 0.3]);
    assert_eq!((g.misc[0], g.misc[1]), (1.0, 1.0));
    // Refraction samples the world probe unless ray-traced refraction is on for the scene and
    // the material; only then does it need the opaque copy.
    assert!(!p.screen_refraction && !p.refracts());
    assert_eq!(g.ints[3] & 8, 0);
    // A zero coat weight is no coat.
    m.ext.clearcoat.as_mut().unwrap().factor.factor = 0.0;
    assert_eq!(params(&m).flags & bit("clearcoat"), 0);
}

#[test]
fn screen_refraction_needs_scene_and_material_ray_tracing() {
    let mut m = plain();
    m.ext.transmission = Some(Transmission {
        factor: Channel {
            factor: 0.5,
            texture: None,
        },
    });
    let mut sc = RenderScene::default();
    let m = Arc::new(m);
    sc.materials.insert(MatKey(1), m.clone());
    let mut s = RMaterialSettings::default();
    s.use_raytrace_refraction = true;
    sc.settings.material_settings.insert(MatKey(1), s);
    // The pipeline passes the scene's own material (found by address).
    let lm = &sc.materials[&MatKey(1)];
    assert!(settings(lm, &sc).use_raytrace_refraction);
    assert!(!MaterialParams::for_material(lm, &sc).screen_refraction);
    sc.settings.eevee.use_raytracing = true;
    let lm = &sc.materials[&MatKey(1)];
    let p = MaterialParams::for_material(lm, &sc);
    assert!(p.screen_refraction && p.refracts());
    assert_eq!(p.gpu().ints[3] & 8, 8);
    // An equal material outside the scene reads the defaults.
    assert!(!settings(&m.as_ref().clone(), &sc).use_raytrace_refraction);
}

#[test]
fn alpha_blend_follows_the_render_method() {
    let mut m = plain();
    m.alpha_mode = AlphaMode::Blend;
    m.base_color.factor[3] = 0.4;
    // Without settings: DITHERED (Blender's default).
    let p = params(&m);
    assert_ne!(p.flags & bit("alpha"), 0);
    assert_eq!(p.blend, BlendMode::Dithered);
    let mut sc = RenderScene::default();
    sc.materials.insert(MatKey(2), Arc::new(m));
    sc.settings
        .material_settings
        .insert(MatKey(2), RMaterialSettings::with_render_method("BLENDED"));
    let p = MaterialParams::for_material(&sc.materials[&MatKey(2)], &sc);
    assert_eq!(p.blend, BlendMode::Blended);
    assert_eq!(p.gpu().ints[1], BlendMode::Blended as u32);
    assert_eq!(alpha::blend_mode("DITHERED"), BlendMode::Dithered);
    // Transparent materials cast a hashed shadow of their alpha; opaque ones a full one.
    use crate::eevee::pipeline::transparent_shadow_alpha;
    assert_eq!(transparent_shadow_alpha(&p), Some(0.4));
    assert_eq!(transparent_shadow_alpha(&params(&plain())), None);
    // Alpha clip is the core's.
    let mut c = plain();
    c.alpha_mode = AlphaMode::Mask { cutoff: 0.5 };
    let p = params(&c);
    assert_eq!((p.flags & bit("alpha"), p.blend), (0, BlendMode::Clip));
    assert_eq!(transparent_shadow_alpha(&p), None);
}

#[test]
fn vertex_colours_bind_their_attribute() {
    let mut m = plain();
    m.vertex_color = Some(VertexColorUse::color_only(Some("Paint".into()), true));
    let p = params(&m);
    assert_ne!(p.flags & bit("vertex_color"), 0);
    assert_eq!(p.vertex_color, vertex_color::MODE_COLOR);
    assert_eq!(p.color_attribute.as_deref(), Some("Paint"));
    // Colour and alpha of the same (default) attribute.
    m.vertex_color = Some(VertexColorUse::color_and_alpha(None, false));
    assert_eq!(
        vertex_color::mode(&m),
        (vertex_color::MODE_COLOR | vertex_color::MODE_ALPHA, None)
    );
    // An alpha from another attribute is not bound (one colour buffer).
    let mut vc = VertexColorUse::color_and_alpha(Some("A".into()), true);
    vc.alpha_attribute = Some("B".into());
    m.vertex_color = Some(vc);
    assert_eq!(vertex_color::mode(&m).0, vertex_color::MODE_COLOR);
    // Alpha only.
    let mut vc = VertexColorUse::color_and_alpha(None, true);
    vc.color = false;
    vc.alpha_attribute = Some("B".into());
    m.vertex_color = Some(vc);
    assert_eq!(
        vertex_color::mode(&m),
        (vertex_color::MODE_ALPHA, Some("B".into()))
    );
}

#[test]
fn texture_channels() {
    assert_eq!(
        [
            channel(ChannelSel::R),
            channel(ChannelSel::G),
            channel(ChannelSel::B),
            channel(ChannelSel::A),
            channel(ChannelSel::Rgb),
        ],
        [0.0, 1.0, 2.0, 3.0, 0.0]
    );
}
