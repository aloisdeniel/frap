// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_mesh_quantization` (§17.1). WP 7.1.
//!
//! The extension has no JSON object: listing it in `extensionsUsed` (and `extensionsRequired`,
//! as writers must) allows the integer component types of vertex attributes the core spec
//! forbids (`POSITION` as (normalized) BYTE/SHORT, `NORMAL`/`TANGENT` as normalized BYTE/SHORT,
//! `TEXCOORD_n` as (normalized) BYTE/SHORT, morph targets likewise). The accessor codec
//! (`accessor.rs`) reads them, and the validator's attribute-format rule (`validate.rs`,
//! `attribute_formats`) accepts them exactly when the document lists the extension;
//! [`needs_quantization`] tells a writer whether its document uses such a type.

use serde::{Deserialize, Serialize};

use crate::json::{
    AccessorType, ComponentType, Document, EXTENSIONS, Extension, ExtensionCx, ExtensionDef,
};
use crate::validate::Report;

/// The extension name.
pub const NAME: &str = "KHR_mesh_quantization";
const REQUIRED: bool = true;

/// Marker type (the extension never appears as an object).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KhrMeshQuantization {}

impl Extension for KhrMeshQuantization {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    // Defined on no object.
    cx.wrong_location(r, "no object (it is only listed in extensionsUsed)");
}

/// Whether accessor format `(semantic, component type, normalized)` of a primitive attribute
/// (or morph target, `target`) is one only `KHR_mesh_quantization` allows.
pub fn is_quantized_format(
    semantic: &str,
    ct: ComponentType,
    normalized: bool,
    target: bool,
) -> bool {
    let float = ct == ComponentType::Float;
    match semantic {
        "POSITION" | "NORMAL" | "TANGENT" => !float,
        // Core TEXCOORD allows normalized unsigned bytes and shorts.
        s if s.starts_with("TEXCOORD_") => {
            !(float
                || (!target
                    && normalized
                    && matches!(
                        ct,
                        ComponentType::UnsignedByte | ComponentType::UnsignedShort
                    )))
        }
        _ => false,
    }
}

/// Whether any mesh attribute or morph target of `doc` uses a format only this extension
/// allows (so a writer must list it, required).
pub fn needs_quantization(doc: &Document) -> bool {
    let check = |sem: &str, acc: u32, target: bool| {
        doc.accessors.get(acc as usize).is_some_and(|a| {
            a.type_ != AccessorType::Scalar
                && is_quantized_format(sem, a.component_type, a.normalized_or_default(), target)
        })
    };
    doc.meshes.iter().flat_map(|m| &m.primitives).any(|p| {
        p.attributes.iter().any(|(s, &a)| check(s, a, false))
            || p.targets
                .iter()
                .any(|t| t.iter().any(|(s, &a)| check(s, a, true)))
    })
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
    fn is_required() {
        assert_eq!(<KhrMeshQuantization as Extension>::NAME, NAME);
        const { assert!(KhrMeshQuantization::REQUIRED_BY_DEFAULT) };
    }

    fn doc(with_ext: bool, pos_ct: u32, normalized: bool) -> Document {
        let mut j = serde_json::json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": 64}],
            "bufferViews": [{"buffer": 0, "byteLength": 36, "byteStride": 12, "target": 34962}],
            "accessors": [
                {"bufferView": 0, "componentType": pos_ct, "count": 3, "type": "VEC3",
                 "normalized": normalized, "min": [0, 0, 0], "max": [1, 1, 1]},
                {"bufferView": 0, "byteOffset": 8, "componentType": 5120, "normalized": true,
                 "count": 3, "type": "VEC2"}
            ],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "TEXCOORD_0": 1}}]}]
        });
        if with_ext {
            j["extensionsUsed"] = serde_json::json!([NAME]);
            j["extensionsRequired"] = serde_json::json!([NAME]);
        }
        Document::from_slice(&serde_json::to_vec(&j).unwrap()).unwrap()
    }

    #[test]
    fn integer_attributes_need_the_extension() {
        let with = doc(true, 5122, false);
        let r = crate::validate::validate(&with);
        assert!(r.is_ok(), "{r}");
        assert!(needs_quantization(&with));
        let without = doc(false, 5122, false);
        let r = crate::validate::validate(&without);
        assert!(r.has("MESH_PRIMITIVE_ATTRIBUTE_FORMAT"), "{r}");
    }

    #[test]
    fn quantized_formats() {
        use ComponentType::*;
        assert!(is_quantized_format("POSITION", UnsignedShort, false, false));
        assert!(!is_quantized_format("POSITION", Float, false, false));
        assert!(is_quantized_format("NORMAL", Byte, true, false));
        assert!(!is_quantized_format(
            "TEXCOORD_0",
            UnsignedShort,
            true,
            false
        ));
        assert!(is_quantized_format(
            "TEXCOORD_1",
            UnsignedShort,
            false,
            false
        ));
        assert!(is_quantized_format("TEXCOORD_0", UnsignedShort, true, true));
        assert!(!is_quantized_format("COLOR_0", UnsignedByte, true, false));
        assert!(!is_quantized_format("JOINTS_0", UnsignedByte, false, false));
    }
}
