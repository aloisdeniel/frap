// SPDX-License-Identifier: GPL-3.0-or-later
//! KHR_lights_punctual → `bpy.data.lights` (M1: the static light data; M3 WP 3.4 completes the
//! camera/light mapping, KHR_animation_pointer arrives in M6). `range` is ignored, as in
//! Blender.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/light.py` (`BlenderLight.create`,
//! `create_directional/point/spot`, `calc_energy_*`, `calc_spot_cone_*`), Apache-2.0, The
//! glTF-Blender-IO authors.

use std::f64::consts::PI;

use frap_gltf::ext::khr_lights_punctual::{KhrLightsPunctual, KhrLightsPunctualNode, LightType};
use frap_rna::{IdRef, LightId};
use frap_scene::id::Light;
use frap_scene::{ChangeLog, Main};

use super::settings::ExportImportConvertLightingMode as Mode;
use super::{Gltf, ImportError, extras};

/// `PBR_WATTS_TO_LUMENS` (`blender/com/conversion.py`).
const PBR_WATTS_TO_LUMENS: f64 = 683.0;

/// RNA hard range of `SpotLight.spot_size`.
const SPOT_SIZE_MIN: f32 = 0.017_453_292;
const SPOT_SIZE_MAX: f32 = std::f32::consts::PI;

fn energy_directional(mode: Mode, intensity: f64) -> f64 {
    match mode {
        Mode::Spec => intensity / PBR_WATTS_TO_LUMENS,
        Mode::Compat | Mode::Raw => intensity,
    }
}

fn energy_pointlike(mode: Mode, intensity: f64) -> f64 {
    match mode {
        Mode::Spec => intensity / PBR_WATTS_TO_LUMENS * 4.0 * PI,
        Mode::Compat => intensity * 4.0 * PI,
        Mode::Raw => intensity,
    }
}

/// `BlenderLight.create` for the light referenced by glTF node `node_idx`.
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    node_idx: usize,
) -> Result<IdRef<LightId>, ImportError> {
    let bad = |m: String| ImportError::Data(m);
    let node_ext = cx.doc.nodes[node_idx]
        .extensions
        .get::<KhrLightsPunctualNode>()
        .map_err(|e| bad(e.to_string()))?
        .ok_or_else(|| bad(format!("node {node_idx} has no KHR_lights_punctual light")))?;
    let lights = cx
        .doc
        .extensions
        .get::<KhrLightsPunctual>()
        .map_err(|e| bad(e.to_string()))?
        .unwrap_or_default();
    let pylight = lights
        .lights
        .get(node_ext.light as usize)
        .ok_or_else(|| bad(format!("light {} does not exist", node_ext.light)))?;
    let mode = cx.settings.export_import_convert_lighting_mode;
    let (ty, default_name) = match pylight.type_ {
        LightType::Directional => ("SUN", "Sun"),
        LightType::Point => ("POINT", "Point"),
        LightType::Spot => ("SPOT", "Spot"),
    };
    let mut la = Light::new("", ty);
    if let Some(i) = pylight.intensity {
        let e = match pylight.type_ {
            LightType::Directional => energy_directional(mode, i),
            _ => energy_pointlike(mode, i),
        };
        la.fields.energy = e as f32;
    }
    if pylight.type_ == LightType::Spot
        && let Some(spot) = &pylight.spot
    {
        let outer = spot.outer_cone_angle_or_default();
        let inner = spot.inner_cone_angle_or_default();
        la.fields.spot_size = ((outer * 2.0) as f32).clamp(SPOT_SIZE_MIN, SPOT_SIZE_MAX);
        la.fields.spot_blend = ((1.0 - inner / outer) as f32).clamp(0.0, 1.0);
    }
    if let Some(c) = pylight.color {
        la.fields.color = [c[0] as f32, c[1] as f32, c[2] as f32];
    }
    extras::set_extras(&mut la.header, pylight.extras.as_ref(), &[]);
    let name = pylight.name.clone().unwrap_or_else(|| default_name.into());
    Ok(main.lights.insert(&name, la, log))
}
