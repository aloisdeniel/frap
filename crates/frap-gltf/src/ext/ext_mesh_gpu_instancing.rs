// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_mesh_gpu_instancing` (§17.1). WP 7.5 (M7 W0 stub: the typed model).
//!
//! On a node with a mesh: per-instance accessors (`TRANSLATION`, `ROTATION`, `SCALE`, custom
//! `_*`), one element per instance. The add-on writes it with `export_gpu_instances` for the
//! children of an Empty that share a mesh (`exporter.py::manage_gpu_instancing_nodes`).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "EXT_mesh_gpu_instancing";
const REQUIRED: bool = false;

/// The extension on a node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtMeshGpuInstancing {
    /// Instance attribute semantic → accessor.
    pub attributes: IndexMap<String, Index>,
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

impl Extension for ExtMeshGpuInstancing {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

/// The accepted accessor shapes of the built-in instance attributes (spec: `TRANSLATION`
/// and `SCALE` FLOAT VEC3; `ROTATION` VEC4 FLOAT or normalized BYTE / SHORT); custom
/// attributes start with `_`.
fn attribute_ok(semantic: &str, a: &crate::json::Accessor) -> Result<(), &'static str> {
    use crate::json::{AccessorType as T, ComponentType as C};
    let normalized = a.normalized == Some(true);
    match semantic {
        "TRANSLATION" | "SCALE" => (a.type_ == T::Vec3 && a.component_type == C::Float)
            .then_some(())
            .ok_or("must be a FLOAT VEC3 accessor"),
        "ROTATION" => (a.type_ == T::Vec4
            && (a.component_type == C::Float
                || (normalized && matches!(a.component_type, C::Byte | C::Short))))
        .then_some(())
        .ok_or("must be a VEC4 accessor of FLOAT or normalized BYTE / SHORT components"),
        s if s.starts_with('_') => Ok(()),
        _ => Err("is not a valid instance attribute (custom attributes start with '_')"),
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    let Some(node) = cx.owner_index_in("nodes") else {
        cx.wrong_location(r, "a node");
        return;
    };
    let Some(ext) = cx.decode::<ExtMeshGpuInstancing>(r) else {
        return;
    };
    if cx.doc.nodes.get(node).is_some_and(|n| n.mesh.is_none()) {
        r.error(
            "EXT_MESH_GPU_INSTANCING_NO_MESH",
            cx.pointer(),
            "a node with EXT_mesh_gpu_instancing must have a mesh",
        );
    }
    if ext.attributes.is_empty() {
        r.error(
            "EMPTY_ENTITY",
            cx.member("attributes"),
            "EXT_mesh_gpu_instancing needs at least one attribute",
        );
    }
    let mut count: Option<u64> = None;
    for (semantic, &i) in &ext.attributes {
        let p = cx.member(&format!("attributes/{semantic}"));
        let Some(a) = cx.doc.accessors.get(i as usize) else {
            r.error(
                "UNRESOLVED_REFERENCE",
                p,
                format!("accessor {i} does not exist"),
            );
            continue;
        };
        if let Err(m) = attribute_ok(semantic, a) {
            r.error(
                "MESH_GPU_INSTANCING_INVALID_ATTRIBUTE",
                p.clone(),
                format!("{semantic} {m}"),
            );
        }
        match count {
            Some(c) if c != a.count => r.error(
                "MESH_GPU_INSTANCING_COUNT_MISMATCH",
                p,
                format!(
                    "{semantic} has {} instances, another attribute {c}",
                    a.count
                ),
            ),
            _ => count = Some(a.count),
        }
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
        let v = serde_json::json!({"attributes": {"TRANSLATION": 4, "ROTATION": 5, "SCALE": 6}});
        let e: ExtMeshGpuInstancing = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.attributes["SCALE"], 6);
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }

    fn doc(ext: serde_json::Value, mesh: bool) -> crate::json::Document {
        let mut j = serde_json::json!({
            "asset": {"version": "2.0"},
            "accessors": [
                {"componentType": 5126, "count": 2, "type": "VEC3"},
                {"componentType": 5126, "count": 2, "type": "VEC4"},
                {"componentType": 5126, "count": 3, "type": "VEC3"},
                {"componentType": 5123, "count": 2, "type": "VEC4", "normalized": true}
            ],
            "meshes": [{"primitives": [{"attributes": {}}]}],
            "nodes": [{"extensions": {"EXT_mesh_gpu_instancing": ext}}],
            "extensionsUsed": ["EXT_mesh_gpu_instancing"]
        });
        if mesh {
            j["nodes"][0]["mesh"] = serde_json::json!(0);
        }
        serde_json::from_value(j).unwrap()
    }

    #[test]
    fn validation_rules() {
        let ok = doc(
            serde_json::json!({"attributes": {"TRANSLATION": 0, "ROTATION": 1, "_ID": 0}}),
            true,
        );
        let r = crate::validate::validate(&ok);
        let own: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.pointer.starts_with("/nodes"))
            .collect();
        assert!(own.is_empty(), "{own:?}");
        let no_mesh = doc(serde_json::json!({"attributes": {"TRANSLATION": 0}}), false);
        assert!(crate::validate::validate(&no_mesh).has("EXT_MESH_GPU_INSTANCING_NO_MESH"));
        let bad = doc(
            serde_json::json!({"attributes": {"TRANSLATION": 1, "SCALE": 2, "ROTATION": 3, "COLOR": 0, "X": 9}}),
            true,
        );
        let r = crate::validate::validate(&bad);
        assert!(r.has("MESH_GPU_INSTANCING_INVALID_ATTRIBUTE"));
        assert!(r.has("MESH_GPU_INSTANCING_COUNT_MISMATCH"));
        assert!(r.has("UNRESOLVED_REFERENCE"));
    }
}
