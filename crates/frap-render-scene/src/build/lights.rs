// SPDX-License-Identifier: GPL-3.0-or-later
//! `Light` data → [`RLight`] (Blender units preserved: W, W/m² for sun, radians).
//!
//! `Light.exposure` is folded into the energy (`energy × 2^exposure`, the scale Cycles and
//! EEVEE apply). The EEVEE-only settings (`use_temperature` / `temperature`, `normalize`, the
//! per-component factors, shadow filtering) are carried raw in [`RLightEevee`] (M5+M6 W0) for
//! the EEVEE-like engine (WP 5.1) to interpret.

use frap_scene::id::Light;
use glam::Mat4;

use crate::types::{RLight, RLightEevee, RLightKind};

/// The render light of `light` placed at `matrix_world`.
pub fn light(light: &Light, matrix_world: Mat4) -> RLight {
    let f = &light.fields;
    let kind = match light.r#type.as_str() {
        "SUN" => RLightKind::Sun { angle: f.angle },
        "SPOT" => RLightKind::Spot {
            size: f.spot_size,
            blend: f.spot_blend,
        },
        "AREA" => RLightKind::Area {
            shape: f.shape.clone(),
            size: [f.size, f.size_y],
        },
        _ => RLightKind::Point,
    };
    RLight {
        kind,
        color: f.color,
        energy: f.energy * libm::exp2f(f.exposure),
        radius: f.shadow_soft_size,
        use_shadow: f.use_shadow,
        matrix_world,
        eevee: RLightEevee {
            use_temperature: f.use_temperature,
            temperature: f.temperature,
            diffuse_factor: f.diffuse_factor,
            specular_factor: f.specular_factor,
            transmission_factor: f.transmission_factor,
            volume_factor: f.volume_factor,
            use_soft_falloff: f.use_soft_falloff,
            normalize: f.normalize,
            cutoff_distance: f.use_custom_distance.then_some(f.cutoff_distance),
            shadow_filter_radius: f.shadow_filter_radius,
            shadow_maximum_resolution: f.shadow_maximum_resolution,
            use_absolute_resolution: f.use_absolute_resolution,
            use_shadow_jitter: f.use_shadow_jitter,
            shadow_jitter_overblur: f.shadow_jitter_overblur,
            spread: f.spread,
        },
    }
}
