// SPDX-License-Identifier: GPL-3.0-or-later
//! `Light` data → [`RLight`] (Blender units preserved: W, W/m² for sun, radians).
//!
//! `Light.exposure` is folded into the energy (`energy × 2^exposure`, the scale Cycles and
//! EEVEE apply). `use_temperature` / `temperature`, `normalize` and the
//! per-component factors are not carried yet (the EEVEE-like engine is M5, §19.3).

use frap_scene::id::Light;
use glam::Mat4;

use crate::types::{RLight, RLightKind};

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
    }
}
