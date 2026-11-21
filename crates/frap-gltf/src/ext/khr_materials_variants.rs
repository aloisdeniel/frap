// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_variants` (§17.1). WP 7.5 (M7 W0 stub: the typed model).
//!
//! On the root: the variant list (`variants[].name`). On a mesh primitive: `mappings`, each a
//! material and the variant indices that select it. The add-on's data model (variants on
//! `bpy.data.scenes[0]`, per-slot mappings on the mesh) exists only while the add-on
//! preference `KHR_materials_variants_ui` is on (§4.1).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "KHR_materials_variants";
const REQUIRED: bool = false;

/// One variant of the root list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// Variant name.
    pub name: String,
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

/// The extension on the root: the variants.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsVariants {
    /// Variants, referenced by index from the primitives' mappings.
    pub variants: Vec<Variant>,
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

/// One mapping of a primitive: the material the listed variants select.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantMapping {
    /// Material (absent: the add-on's empty slot).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Index>,
    /// Indices into the root's `variants`.
    pub variants: Vec<Index>,
    /// Optional name.
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

/// The extension on a mesh primitive.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrMaterialsVariantsPrimitive {
    /// Material mappings.
    pub mappings: Vec<VariantMapping>,
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

impl Extension for KhrMaterialsVariants {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

impl Extension for KhrMaterialsVariantsPrimitive {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_is_root() {
        let _ = cx.decode::<KhrMaterialsVariants>(r);
    } else if cx.owner.contains("/primitives/") {
        let _ = cx.decode::<KhrMaterialsVariantsPrimitive>(r);
    } else {
        cx.wrong_location(r, "the root or a mesh primitive");
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(EXTENSIONS)]
static DEF: ExtensionDef = ExtensionDef {
    name: NAME,
    required_by_default: REQUIRED,
    validate,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_round_trip() {
        let v = serde_json::json!({"variants": [{"name": "Red"}, {"name": "Blue"}]});
        let e: KhrMaterialsVariants = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.variants[1].name, "Blue");
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
        let p = serde_json::json!({"mappings": [{"material": 0, "variants": [0, 1]}]});
        let e: KhrMaterialsVariantsPrimitive = serde_json::from_value(p.clone()).unwrap();
        assert_eq!(e.mappings[0].variants, vec![0, 1]);
        assert_eq!(serde_json::to_value(&e).unwrap(), p);
    }
}
