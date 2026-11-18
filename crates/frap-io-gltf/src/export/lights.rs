// SPDX-License-Identifier: GPL-3.0-or-later
//! Lights → `KHR_lights_punctual` (M3, WP 3.4).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/lights.py` (`gather_lights_punctual`), `blender/exp/light_spots.py`,
//! `blender/exp/nodes.py` (`__gather_extensions`), `blender/exp/exporter.py` (`__traverse`:
//! child-of-root extensions appended once, equal lights shared) and `blender/com/
//! blender_default.py` (`LIGHTS`):
//!
//! - LIGHT objects are kept by the vtree filter only with `export_lights` (default False);
//! - `POINT` → `point`, `SUN` → `directional`, `SPOT` → `spot`; an `AREA` light keeps its
//!   node (with the camera/light correction) but gets no light, with the WARNING
//!   `Unsupported light source AREA` once per distinct (light datablock, world matrix), as
//!   the add-on's `@cached gather_lights_punctual(blender_lamp, world_matrix, …)` logs it
//!   (two AREA objects warn twice, even when they share their data, unless they also share
//!   their world matrix);
//! - `color` = `Light.color`, times `temperature_color` with `use_temperature` (always
//!   written, even white; it may exceed 1 as in Blender);
//! - `intensity` from `energy` (W), times `Light.area(matrix_world)` when `normalize` is off,
//!   then by `export_import_convert_lighting_mode`: `SPEC` (default) W → cd (`/ 4π · 683`) for
//!   point and spot, W/m² → lux (`· 683`) for sun; `COMPAT` the same without `683`; `RAW` the
//!   value as is; every mode times `2 ** exposure`;
//! - `spot`: `outerConeAngle = spot_size / 2`, `innerConeAngle = outer - outer · spot_blend`;
//! - `range` = `cutoff_distance` only with `use_custom_distance` (for every punctual type,
//!   as Blender writes it);
//! - the node references the light by index; the root extension lists each distinct light
//!   once (lights equal in every member, e.g. objects sharing light data, share an entry);
//!   the extension is both used and required.
//!
//! Light node trees (Cycles emission strength) are not modelled in frap (`Light.node_tree`
//! reads `None`), which is the add-on's no-emission-node path. `extras` of the light
//! datablock (`export_extras`) are written by `extras.rs` once it has a datablock entry point
//! (WP 3.7).

use std::f64::consts::PI;

use frap_gltf::ext::khr_lights_punctual::{
    KhrLightsPunctual, KhrLightsPunctualNode, Light as GLight, LightType, Spot,
};
use frap_gltf::json::{Extension, Index, Node};
use frap_math::glam::Mat4;
use frap_rna::{IdRef, LightId, Report, ReportType};
use frap_scene::id::{Light, Object};
use frap_spec_ops_io::export_scene::gltf::ExportImportConvertLightingMode as Mode;

use crate::export::gather::Gather;
use crate::export::vtree::matrix_world;
use crate::export::{ExportError, ExportSettings};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_lights", "export_import_convert_lighting_mode"];

/// `PBR_WATTS_TO_LUMENS` (`blender/com/conversion.py`).
pub const PBR_WATTS_TO_LUMENS: f64 = 683.0;

/// Whether LIGHT objects are exported (`export_lights`).
pub fn exported(settings: &ExportSettings) -> bool {
    settings.props.export_lights.value
}

/// The `KHR_lights_punctual` light of light data `la` with object world matrix
/// `matrix_world`, or `Err(message)` for an unsupported type (the WARNING text).
pub fn light_to_gltf(la: &Light, matrix_world: &Mat4, mode: Mode) -> Result<GLight, String> {
    let type_ = match la.r#type.as_str() {
        "POINT" => LightType::Point,
        "SUN" => LightType::Directional,
        "SPOT" => LightType::Spot,
        other => return Err(format!("Unsupported light source {other}")),
    };
    let f = &la.fields;
    let d = f64::from;

    // __gather_color: no emission node, so the lamp colour (times the blackbody colour).
    let mut color = [d(f.color[0]), d(f.color[1]), d(f.color[2])];
    if f.use_temperature {
        let t = la.temperature_color();
        for (c, t) in color.iter_mut().zip(t) {
            *c *= d(t);
        }
    }

    // __gather_intensity
    let mut strength = d(f.energy);
    if !f.normalize {
        strength *= d(la.area(matrix_world));
    }
    let exposure = 2f64.powf(d(f.exposure));
    let intensity = match mode {
        Mode::Raw => strength * exposure,
        Mode::Spec | Mode::Compat => {
            let mut luminous = if type_ == LightType::Directional {
                strength
            } else {
                strength / (4.0 * PI)
            };
            if mode == Mode::Spec {
                luminous *= PBR_WATTS_TO_LUMENS;
            }
            luminous * exposure
        }
    };

    // light_spots.py
    let spot = (type_ == LightType::Spot).then(|| {
        let angle = d(f.spot_size) * 0.5;
        Spot {
            inner_cone_angle: Some(angle - angle * d(f.spot_blend)),
            outer_cone_angle: Some(angle),
            ..Spot::default()
        }
    });

    Ok(GLight {
        color: Some(color),
        intensity: Some(intensity),
        spot,
        type_,
        range: f.use_custom_distance.then(|| d(f.cutoff_distance)),
        name: Some(la.header.name.to_string()),
        extensions: Default::default(),
        extras: None,
    })
}

/// The `(light, world matrix)` keys already gathered (the add-on's `@cached` key of
/// `gather_lights_punctual`); an unsupported light warns once per key.
#[derive(Default)]
struct GatheredLights(Vec<(IdRef<LightId>, [u32; 16])>);

/// Adds the node's `KHR_lights_punctual` reference (and the light to the root extension) for a
/// LIGHT object; nothing for other objects (and only a WARNING for an AREA light).
pub fn apply(g: &mut Gather, ob: &Object, node: &mut Node) -> Result<(), ExportError> {
    if !exported(g.settings) || ob.object_type() != "LIGHT" {
        return Ok(());
    }
    let main = g.main;
    let Some((la_ref, la)) = ob
        .data
        .and_then(|d| d.typed::<LightId>())
        .and_then(|r| Some((r, main.lights.get(r)?)))
    else {
        return Ok(());
    };
    let mode = g.settings.props.export_import_convert_lighting_mode.value;
    let world = matrix_world(main, ob);
    let light = match light_to_gltf(la, &world, mode) {
        Ok(l) => l,
        Err(msg) => {
            // `world_matrix.copy().freeze()` hashes by value (`-0.0` equals `0.0`).
            let key = (la_ref, world.to_cols_array().map(|v| (v + 0.0).to_bits()));
            let seen = &mut g.state::<GatheredLights>().0;
            if !seen.contains(&key) {
                seen.push(key);
                g.reports.push(Report::new(ReportType::Warning, msg));
            }
            return Ok(());
        }
    };
    let mut root = g
        .doc
        .extensions
        .get::<KhrLightsPunctual>()
        .map_err(|e| ExportError::Invalid(e.to_string()))?
        .unwrap_or_default();
    let idx = match root.lights.iter().position(|l| *l == light) {
        Some(i) => i,
        None => {
            root.lights.push(light);
            root.lights.len() - 1
        }
    };
    g.doc.extensions.set(Some(root));
    node.extensions.set(Some(KhrLightsPunctualNode {
        light: idx as Index,
        ..KhrLightsPunctualNode::default()
    }));
    g.use_extension(
        KhrLightsPunctual::NAME,
        KhrLightsPunctual::REQUIRED_BY_DEFAULT,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(ty: &str) -> Light {
        Light::new(ty, ty)
    }

    /// Exports the factory scene with its light re-typed to `ty`.
    fn export_factory(ty: &str, lights: bool, cameras: bool) -> crate::export::ExportOutput {
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let r = main.lights.find("Light").unwrap();
        main.lights.get_mut(r, &mut log).unwrap().r#type = ty.into();
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.export_lights.value = lights;
        p.export_cameras.value = cameras;
        let s = ExportSettings::from_props(&p);
        crate::export::export(&main, &ctx, &s).unwrap()
    }

    #[test]
    fn area_light_keeps_its_node_with_a_warning() {
        let out = export_factory("AREA", true, false);
        let names: Vec<_> = out
            .document
            .nodes
            .iter()
            .map(|n| n.name.clone().unwrap())
            .collect();
        assert_eq!(names, ["Cube", "Light"]);
        assert!(out.document.nodes[1].extensions.is_empty());
        assert!(out.extensions_used.is_empty());
        assert_eq!(out.reports.len(), 1);
        assert_eq!(out.reports[0].message, "Unsupported light source AREA");
        assert_eq!(out.reports[0].level, frap_rna::ReportType::Warning);
    }

    // Blender 5.2.4 logs the WARNING once per `(light, matrix_world)`: two AREA objects with
    // the same data and transform warn once, a third one elsewhere warns again.
    #[test]
    fn area_light_warns_once_per_light_and_world_matrix() {
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let la = main.lights.find("Light").unwrap();
        main.lights.get_mut(la, &mut log).unwrap().r#type = "AREA".into();
        let src = main.objects.find("Light").unwrap();
        let coll = main.collections.find("Collection").unwrap();
        for (name, loc) in [("L2", None), ("L3", Some([1.0, 2.0, 3.0]))] {
            let mut ob = main.objects.get(src).unwrap().clone();
            ob.header = Object::new(name, ob.data).header;
            if let Some(l) = loc {
                ob.location = l;
            }
            let r = main.objects.insert(name, ob, &mut log);
            main.collections
                .get_mut(coll, &mut log)
                .unwrap()
                .objects
                .push(r);
        }
        frap_scene::id::layers::sync_all(&mut main, &mut log);
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let mut p = crate::export::settings::default_props();
        p.export_lights.value = true;
        let s = ExportSettings::from_props(&p);
        let out = crate::export::export(&main, &ctx, &s).unwrap();
        assert_eq!(out.document.nodes.len(), 4);
        let warnings: Vec<_> = out.reports.iter().map(|r| r.message.as_str()).collect();
        assert_eq!(warnings, ["Unsupported light source AREA"; 2]);
    }

    #[test]
    fn point_light_extension_is_used_and_required() {
        let out = export_factory("POINT", true, true);
        assert_eq!(out.document.extensions_used, ["KHR_lights_punctual"]);
        assert_eq!(out.document.extensions_required, ["KHR_lights_punctual"]);
        let root = out
            .document
            .extensions
            .get::<KhrLightsPunctual>()
            .unwrap()
            .unwrap();
        assert_eq!(root.lights.len(), 1);
        assert_eq!(root.lights[0].intensity, Some(54351.41306588226));
        assert_eq!(out.document.cameras.len(), 1);
        assert_eq!(out.document.nodes.len(), 3);
        assert!(out.reports.is_empty());
        // Not exported: the light object is filtered out, no extension at all.
        let out = export_factory("POINT", false, false);
        assert_eq!(out.document.nodes.len(), 1);
        assert!(out.document.extensions_used.is_empty());
        assert!(out.document.cameras.is_empty());
    }

    // Expected values dumped from Blender 5.2.4 (`export_scene.gltf(export_lights=True)`).
    #[test]
    fn point_spec_watts_to_candela() {
        let mut la = light("POINT");
        la.fields.energy = 1000.0;
        let l = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Spec).unwrap();
        assert_eq!(l.intensity, Some(54351.41306588226));
        assert_eq!(l.color, Some([1.0; 3]));
        assert_eq!(l.range, None);
        assert_eq!(l.spot, None);
    }

    #[test]
    fn spot_cones_colour_and_range() {
        let mut la = light("SPOT");
        la.fields.energy = 500.0;
        la.fields.spot_size = 0.9;
        la.fields.spot_blend = 0.3;
        la.fields.color = [1.0, 0.5, 0.25];
        la.fields.use_custom_distance = true;
        la.fields.cutoff_distance = 12.5;
        let l = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Spec).unwrap();
        assert_eq!(l.intensity, Some(27175.70653294113));
        let s = l.spot.unwrap();
        assert_eq!(s.inner_cone_angle, Some(0.31499998629093184));
        assert_eq!(s.outer_cone_angle, Some(0.44999998807907104));
        assert_eq!(l.color, Some([1.0, 0.5, 0.25]));
        assert_eq!(l.range, Some(12.5));
    }

    #[test]
    fn sun_lux() {
        let mut la = light("SUN");
        la.fields.energy = 3.0;
        let l = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Spec).unwrap();
        assert_eq!(l.intensity, Some(2049.0));
        assert_eq!(l.type_, LightType::Directional);
        let l = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Compat).unwrap();
        assert_eq!(l.intensity, Some(3.0));
    }

    #[test]
    fn temperature_area_and_exposure() {
        let mut la = light("POINT");
        la.fields.use_temperature = true;
        la.fields.temperature = 3000.0;
        la.fields.exposure = 1.5;
        la.fields.normalize = false;
        la.fields.shadow_soft_size = 0.3;
        let l = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Spec).unwrap();
        let c = l.color.unwrap();
        let want = [1.7708408832550049, 0.8442848920822144, 0.2722227871417999];
        for (a, b) in c.iter().zip(want) {
            assert!((a - b).abs() < 1e-6, "{c:?}");
        }
        let i = l.intensity.unwrap();
        assert!((i - 1738.6343119189432).abs() < 1e-9 * i, "{i}");
    }

    #[test]
    fn raw_and_compat_modes() {
        let mut la = light("POINT");
        la.fields.energy = 100.0;
        la.fields.exposure = 1.0;
        assert_eq!(
            light_to_gltf(&la, &Mat4::IDENTITY, Mode::Raw)
                .unwrap()
                .intensity,
            Some(200.0)
        );
        let c = light_to_gltf(&la, &Mat4::IDENTITY, Mode::Compat)
            .unwrap()
            .intensity
            .unwrap();
        assert_eq!(c, 100.0 / (4.0 * PI) * 2.0);
    }

    #[test]
    fn area_is_unsupported() {
        let la = light("AREA");
        assert_eq!(
            light_to_gltf(&la, &Mat4::IDENTITY, Mode::Spec).unwrap_err(),
            "Unsupported light source AREA"
        );
    }
}
