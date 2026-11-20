// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_animation_pointer`: typed struct + validation (§17.1). WP 6.9.
//!
//! An animation channel whose `target.path` is `"pointer"` animates the property a JSON pointer
//! names instead of a node's TRS or weights: `target = {path: "pointer", extensions:
//! {KHR_animation_pointer: {pointer: "/materials/0/pbrMetallicRoughness/baseColorFactor"}}}`,
//! without `node` [GLTF §3.9]. Not required (a viewer without it ignores the channel).
//!
//! io_scene_gltf2 5.2.41 writes the light cone angles as `…/lights/{i}/spot.innerConeAngle`
//! (a `.`, not the spec's `/spot/innerConeAngle`) and reads them back the same way; the
//! validator accepts any property path below an existing object, so those files validate.

use serde::{Deserialize, Serialize};

use crate::json::{
    AnimationPath, ChannelTarget, Document, EXTENSIONS, Extension, ExtensionCx, ExtensionDef,
    Extensions, Extras,
};
use crate::validate::Report;

/// The `KHR_animation_pointer` object of an animation channel target.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrAnimationPointer {
    /// JSON pointer (RFC 6901) to the animated property.
    pub pointer: String,
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

impl Extension for KhrAnimationPointer {
    const NAME: &'static str = "KHR_animation_pointer";
}

impl KhrAnimationPointer {
    /// A pointer extension object for `pointer`.
    pub fn new(pointer: impl Into<String>) -> KhrAnimationPointer {
        KhrAnimationPointer {
            pointer: pointer.into(),
            ..KhrAnimationPointer::default()
        }
    }

    /// A channel target animating `pointer` (`path: "pointer"`, no `node`).
    pub fn target(pointer: impl Into<String>) -> ChannelTarget {
        let mut extensions = Extensions::default();
        extensions.set(Some(KhrAnimationPointer::new(pointer)));
        ChannelTarget {
            extensions,
            extras: None,
            node: None,
            path: AnimationPath::Pointer,
        }
    }

    /// The pointer string of a channel target (`None` when the target has no well-formed
    /// `KHR_animation_pointer` object).
    pub fn of(target: &ChannelTarget) -> Option<String> {
        target
            .extensions
            .get_raw(Self::NAME)?
            .get("pointer")?
            .as_str()
            .map(str::to_owned)
    }

    /// The tokens of the pointer as the add-on splits it (`pointer.split("/")`: the empty
    /// token before the leading `/` included, no unescaping, so `tab[1]` is the top-level
    /// collection).
    pub fn split(pointer: &str) -> Vec<&str> {
        pointer.split('/').collect()
    }
}

/// The RFC 6901 reference tokens of `pointer` (`~1` → `/`, `~0` → `~`); `None` when it does
/// not start with `/` (the empty pointer, the whole document, is not animatable).
pub fn tokens(pointer: &str) -> Option<Vec<String>> {
    let rest = pointer.strip_prefix('/')?;
    Some(
        rest.split('/')
            .map(|t| t.replace("~1", "/").replace("~0", "~"))
            .collect(),
    )
}

/// Escapes one reference token (`~` → `~0`, `/` → `~1`).
pub fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Why `pointer` does not name a property below an existing object of `doc`: the
/// collection-and-index prefix of the glTF objects KHR_animation_pointer animates (`nodes`,
/// `materials`, `meshes`, `cameras`, `extensions/KHR_lights_punctual/lights`, and any other
/// top-level array) must resolve. `None` when it does, or when the pointer starts elsewhere
/// (an extension's own root object: nothing to check).
pub fn unresolved(doc: &Document, pointer: &str) -> Option<String> {
    let t = tokens(pointer)?;
    let root = doc.to_value();
    let (array, index, depth) = match t.as_slice() {
        [ext, name, coll, i, ..] if ext == "extensions" => (
            root.get("extensions")
                .and_then(|e| e.get(name.as_str()))
                .and_then(|e| e.get(coll.as_str())),
            i,
            4,
        ),
        [coll, i, ..] => (root.get(coll.as_str()), i, 2),
        _ => return None,
    };
    let what = t[..depth - 1].join("/");
    let Some(array) = array else {
        return Some(format!("/{what} does not exist"));
    };
    let items = array.as_array()?;
    let ok = index.parse::<usize>().is_ok_and(|i| i < items.len());
    if ok {
        if t.len() == depth {
            return Some(format!("{pointer} names an object, not a property"));
        }
        None
    } else {
        Some(format!(
            "{index} is not an index of /{what} ({} defined)",
            items.len()
        ))
    }
}

/// The JSON pointer of an animation channel target: `/animations/{a}/channels/{c}/target`.
fn is_channel_target(owner: &str) -> bool {
    let t: Vec<&str> = owner.split('/').collect();
    matches!(
        t.as_slice(),
        ["", "animations", a, "channels", c, "target"]
            if a.parse::<usize>().is_ok() && c.parse::<usize>().is_ok()
    )
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !is_channel_target(cx.owner) {
        cx.wrong_location(r, "an animation channel target");
        return;
    }
    let Some(e) = cx.decode::<KhrAnimationPointer>(r) else {
        return;
    };
    if cx.owner_value.get("path").and_then(|p| p.as_str()) != Some("pointer") {
        r.error(
            "ANIMATION_POINTER_PATH",
            format!("{}/path", cx.owner),
            "a target with KHR_animation_pointer must have path \"pointer\"",
        );
    }
    if cx.owner_value.get("node").is_some() {
        r.warning(
            "ANIMATION_POINTER_NODE",
            format!("{}/node", cx.owner),
            "node is ignored when KHR_animation_pointer is used",
        );
    }
    if !e.pointer.starts_with('/') {
        r.error(
            "ANIMATION_POINTER_INVALID",
            cx.member("pointer"),
            format!("{:?} is not a JSON pointer to a property", e.pointer),
        );
        return;
    }
    if let Some(m) = unresolved(cx.doc, &e.pointer) {
        r.error("UNRESOLVED_REFERENCE", cx.member("pointer"), m);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: KhrAnimationPointer::NAME,
    required_by_default: KhrAnimationPointer::REQUIRED_BY_DEFAULT,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{
        Accessor, AccessorType, Animation, AnimationSampler, Channel, ComponentType, Material,
    };
    use crate::validate::validate;

    fn doc_with(pointer: &str) -> Document {
        let mut d = Document::default();
        d.materials.push(Material::default());
        for ty in [AccessorType::Scalar, AccessorType::Vec4] {
            let mut a = Accessor::new(None, ComponentType::Float, ty, 2);
            if ty == AccessorType::Scalar {
                a.min = Some(vec![0.0]);
                a.max = Some(vec![1.0]);
            }
            d.accessors.push(a);
        }
        d.animations.push(Animation {
            channels: vec![Channel {
                extensions: Extensions::default(),
                extras: None,
                sampler: 0,
                target: KhrAnimationPointer::target(pointer),
            }],
            samplers: vec![AnimationSampler {
                extensions: Extensions::default(),
                extras: None,
                input: 0,
                interpolation: None,
                output: 1,
            }],
            ..Animation::default()
        });
        d.use_extension(KhrAnimationPointer::NAME, false);
        d
    }

    #[test]
    fn target_round_trips() {
        let t = KhrAnimationPointer::target("/materials/0/pbrMetallicRoughness/baseColorFactor");
        assert_eq!(t.path, AnimationPath::Pointer);
        assert_eq!(t.node, None);
        let json = serde_json::to_value(&t).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "extensions": {"KHR_animation_pointer": {
                    "pointer": "/materials/0/pbrMetallicRoughness/baseColorFactor"}},
                "path": "pointer"
            })
        );
        let back: ChannelTarget = serde_json::from_value(json).unwrap();
        assert_eq!(
            KhrAnimationPointer::of(&back).as_deref(),
            Some("/materials/0/pbrMetallicRoughness/baseColorFactor")
        );
    }

    #[test]
    fn tokens_unescape_and_split_like_the_add_on() {
        assert_eq!(
            tokens("/extensions/KHR_lights_punctual/lights/0/spot.innerConeAngle").unwrap(),
            [
                "extensions",
                "KHR_lights_punctual",
                "lights",
                "0",
                "spot.innerConeAngle"
            ]
        );
        assert_eq!(tokens("/a~1b/c~0d").unwrap(), ["a/b", "c~d"]);
        assert_eq!(tokens("materials/0"), None);
        assert_eq!(escape_token("a/b~c"), "a~1b~0c");
        assert_eq!(
            KhrAnimationPointer::split("/cameras/0/perspective/yfov"),
            ["", "cameras", "0", "perspective", "yfov"]
        );
    }

    #[test]
    fn validation() {
        let d = doc_with("/materials/0/pbrMetallicRoughness/baseColorFactor");
        assert!(validate(&d).is_ok(), "{}", validate(&d));
        let d = doc_with("/materials/3/pbrMetallicRoughness/baseColorFactor");
        assert!(validate(&d).has("UNRESOLVED_REFERENCE"), "{}", validate(&d));
        let d = doc_with("/cameras/0/perspective/yfov");
        assert!(validate(&d).has("UNRESOLVED_REFERENCE"));
        let d = doc_with("/extensions/KHR_lights_punctual/lights/0/intensity");
        assert!(validate(&d).has("UNRESOLVED_REFERENCE"));
        let d = doc_with("/materials/0");
        assert!(validate(&d).has("UNRESOLVED_REFERENCE"));
        let d = doc_with("materials/0/alphaCutoff");
        assert!(validate(&d).has("ANIMATION_POINTER_INVALID"));
        let mut d = doc_with("/materials/0/alphaCutoff");
        d.animations[0].channels[0].target.path = AnimationPath::Rotation;
        d.animations[0].channels[0].target.node = None;
        assert!(validate(&d).has("ANIMATION_POINTER_PATH"));
        // On a material: wrong location.
        let mut d = doc_with("/materials/0/alphaCutoff");
        d.materials[0]
            .extensions
            .set(Some(KhrAnimationPointer::new("/x")));
        assert!(validate(&d).has("EXTENSION_INVALID_LOCATION"));
    }

    #[test]
    fn unresolved_reads_extension_arrays() {
        let mut d = Document::default();
        d.extensions.insert_raw(
            "KHR_lights_punctual",
            serde_json::json!({"lights": [{"type": "point"}]}),
        );
        assert_eq!(
            unresolved(&d, "/extensions/KHR_lights_punctual/lights/0/intensity"),
            None
        );
        assert!(unresolved(&d, "/extensions/KHR_lights_punctual/lights/1/intensity").is_some());
        assert!(unresolved(&d, "/nodes/0/translation").is_some());
    }
}
