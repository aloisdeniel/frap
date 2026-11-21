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

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("nodes").is_none() {
        cx.wrong_location(r, "a node");
        return;
    }
    let _ = cx.decode::<ExtMeshGpuInstancing>(r);
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
}
