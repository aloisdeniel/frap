// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_lights_punctual`: typed struct + validation (§17.1). WP 1.5.
//!
//! Root: `extensions.KHR_lights_punctual = {lights: [...]}`; node:
//! `extensions.KHR_lights_punctual = {light: i}`. Point and spot intensities are candela,
//! directional lux; lights shine along local -Z. io_scene_gltf2 lists the extension as required
//! (`blender/exp/nodes.py`, `Extension(...)` default `required=True`).

use serde::{Deserialize, Serialize};

use crate::json::{
    ExtensionCx, ExtensionDef, Extensions, Extension, Extras, EXTENSIONS, Index, str_enum,
};
use crate::validate::Report;

str_enum! {
    /// Light type.
    LightType {
        /// Infinitely distant light (lux).
        Directional = "directional",
        /// Point light (candela).
        Point = "point",
        /// Cone light (candela).
        Spot = "spot",
    }
}

/// The root object: every light of the file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrLightsPunctual {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
    /// Lights, referenced by index from nodes.
    pub lights: Vec<Light>,
}

impl Extension for KhrLightsPunctual {
    const NAME: &'static str = "KHR_lights_punctual";
    const REQUIRED_BY_DEFAULT: bool = true;
}

/// A node's light instance.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrLightsPunctualNode {
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
    /// Index into the root `lights`.
    pub light: Index,
}

impl Extension for KhrLightsPunctualNode {
    const NAME: &'static str = "KHR_lights_punctual";
    const REQUIRED_BY_DEFAULT: bool = true;
}

/// One light. Fields are in io_scene_gltf2's key order (`io/com/lights_punctual.py`
/// `Light.to_dict`), so serialization matches Blender's output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Light {
    /// Linear RGB (default white).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[f64; 3]>,
    /// Brightness (default 1): candela for point/spot, lux for directional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f64>,
    /// Cone (spot only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot: Option<Spot>,
    /// Type.
    #[serde(rename = "type")]
    pub type_: LightType,
    /// Cut-off distance (> 0; point and spot only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<f64>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
}

impl Light {
    /// A light of `type_` with every optional property at its default.
    pub fn new(type_: LightType) -> Self {
        Light {
            name: None,
            color: None,
            intensity: None,
            type_,
            range: None,
            spot: None,
            extensions: Extensions::default(),
            extras: None,
        }
    }
    /// Effective colour.
    pub fn color_or_default(&self) -> [f64; 3] {
        self.color.unwrap_or([1.0; 3])
    }
    /// Effective intensity.
    pub fn intensity_or_default(&self) -> f64 {
        self.intensity.unwrap_or(1.0)
    }
}

/// Spot cone angles, radians.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spot {
    /// Full-intensity cone (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inner_cone_angle: Option<f64>,
    /// Fall-off end (default π/4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outer_cone_angle: Option<f64>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
    pub extras: Option<Extras>,
}

impl Spot {
    /// Effective inner angle.
    pub fn inner_cone_angle_or_default(&self) -> f64 {
        self.inner_cone_angle.unwrap_or(0.0)
    }
    /// Effective outer angle.
    pub fn outer_cone_angle_or_default(&self) -> f64 {
        self.outer_cone_angle.unwrap_or(std::f64::consts::FRAC_PI_4)
    }
}

/// The root light list of `cx.doc`, if any (decoding errors are reported at the root).
fn root_lights(cx: &ExtensionCx) -> usize {
    cx.doc
        .extensions
        .get::<KhrLightsPunctual>()
        .ok()
        .flatten()
        .map_or(0, |e| e.lights.len())
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_is_root() {
        let Some(ext) = cx.decode::<KhrLightsPunctual>(r) else {
            return;
        };
        for (i, l) in ext.lights.iter().enumerate() {
            let p = cx.member(&format!("lights/{i}"));
            if let Some(c) = l.color
                && c.iter().any(|v| !(0.0..=1.0).contains(v))
            {
                r.error("VALUE_NOT_IN_RANGE", format!("{p}/color"), "colour components are in [0, 1]");
            }
            if l.intensity.is_some_and(|v| v < 0.0 || !v.is_finite()) {
                r.error("VALUE_NOT_IN_RANGE", format!("{p}/intensity"), "intensity must be ≥ 0");
            }
            if let Some(range) = l.range {
                if l.type_ == LightType::Directional {
                    r.error("KHR_LIGHTS_PUNCTUAL_DIRECTIONAL_RANGE", format!("{p}/range"), "directional lights have no range");
                } else if range <= 0.0 {
                    r.error("VALUE_NOT_IN_RANGE", format!("{p}/range"), "range must be > 0");
                }
            }
            match (&l.spot, l.type_) {
                (None, LightType::Spot) => {
                    r.error("UNSATISFIED_DEPENDENCY", &p, "spot lights need a spot object");
                }
                (Some(_), t) if t != LightType::Spot => {
                    r.error("UNEXPECTED_PROPERTY", format!("{p}/spot"), "only spot lights have a spot object");
                }
                (Some(s), _) => {
                    let (inner, outer) =
                        (s.inner_cone_angle_or_default(), s.outer_cone_angle_or_default());
                    if !(0.0 <= inner && inner < outer && outer <= std::f64::consts::FRAC_PI_2) {
                        r.error(
                            "KHR_LIGHTS_PUNCTUAL_LIGHT_SPOT_ANGLES",
                            format!("{p}/spot"),
                            format!("cone angles must satisfy 0 ≤ inner ({inner}) < outer ({outer}) ≤ π/2"),
                        );
                    }
                }
                (None, _) => {}
            }
        }
    } else if cx.owner_index_in("nodes").is_some() {
        let Some(ext) = cx.decode::<KhrLightsPunctualNode>(r) else {
            return;
        };
        let n = root_lights(cx);
        if ext.light as usize >= n {
            r.error(
                "UNRESOLVED_REFERENCE",
                cx.member("light"),
                format!("light {} does not exist ({n} defined)", ext.light),
            );
        }
    } else {
        cx.wrong_location(r, "the root or a node");
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrLightsPunctual::NAME,
    required_by_default: KhrLightsPunctual::REQUIRED_BY_DEFAULT,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{Document, Node};
    use crate::validate::validate;

    fn doc(lights: Vec<Light>, node_light: u32) -> Document {
        let mut d = Document::default();
        d.extensions.set(Some(KhrLightsPunctual {
            lights,
            ..Default::default()
        }));
        let mut n = Node::default();
        n.extensions.set(Some(KhrLightsPunctualNode {
            light: node_light,
            ..Default::default()
        }));
        d.nodes.push(n);
        d.use_extension(KhrLightsPunctual::NAME, KhrLightsPunctual::REQUIRED_BY_DEFAULT);
        d
    }

    #[test]
    fn typed_round_trip() {
        let mut spot = Light::new(LightType::Spot);
        spot.spot = Some(Spot {
            outer_cone_angle: Some(0.5),
            ..Default::default()
        });
        let d = doc(vec![Light::new(LightType::Point), spot.clone()], 1);
        let json = String::from_utf8(d.to_vec(&Default::default())).unwrap();
        assert!(json.contains(r#""extensionsUsed":["KHR_lights_punctual"],"extensionsRequired":["KHR_lights_punctual"],"extensions":{"KHR_lights_punctual":{"lights":[{"type":"point"},{"spot":{"outerConeAngle":0.5},"type":"spot"}]}}"#), "{json}");
        let back = Document::from_slice(json.as_bytes()).unwrap();
        let lights = back.extensions.get::<KhrLightsPunctual>().unwrap().unwrap();
        assert_eq!(lights.lights[1], spot);
        assert_eq!(
            back.nodes[0].extensions.get::<KhrLightsPunctualNode>().unwrap().unwrap().light,
            1
        );
        assert!(validate(&back).is_ok(), "{}", validate(&back));
    }

    #[test]
    fn invalid_lights_are_reported() {
        let mut dir = Light::new(LightType::Directional);
        dir.range = Some(3.0);
        let mut spot = Light::new(LightType::Spot);
        spot.spot = Some(Spot {
            inner_cone_angle: Some(1.0),
            outer_cone_angle: Some(0.5),
            ..Default::default()
        });
        let r = validate(&doc(vec![dir, spot, Light::new(LightType::Spot)], 7));
        assert!(r.has("KHR_LIGHTS_PUNCTUAL_DIRECTIONAL_RANGE"), "{r}");
        assert!(r.has("KHR_LIGHTS_PUNCTUAL_LIGHT_SPOT_ANGLES"), "{r}");
        assert!(r.has("UNSATISFIED_DEPENDENCY"), "{r}");
        assert!(
            r.issues.iter().any(|i| i.code == "UNRESOLVED_REFERENCE"
                && i.pointer == "/nodes/0/extensions/KHR_lights_punctual/light"),
            "{r}"
        );
        // Malformed JSON and a wrong owner.
        let mut d = doc(vec![], 0);
        d.extensions
            .insert_raw("KHR_lights_punctual", serde_json::json!({"lights": [{"type": "laser"}]}));
        d.scenes.push(Default::default());
        d.scenes[0]
            .extensions
            .insert_raw("KHR_lights_punctual", serde_json::json!({"light": 0}));
        let r = validate(&d);
        assert!(r.has("EXTENSION_INVALID"), "{r}");
        assert!(r.has("EXTENSION_INVALID_LOCATION"), "{r}");
    }
}
