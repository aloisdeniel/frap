// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_draco_mesh_compression` (§17.1). WP 7.2 (M7 W0 stub: the typed model).
//!
//! On a primitive: the buffer view of the Draco bitstream and the Draco unique id of each
//! compressed attribute. The primitive's accessors keep `count`, `componentType`, `type`,
//! `normalized` and `min`/`max` but have no `bufferView` (the add-on's `io/exp/draco.py`).
//! Required (`exporter.py::add_draco_extension`).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "KHR_draco_mesh_compression";
const REQUIRED: bool = true;

/// The extension on a mesh primitive.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrDracoMeshCompression {
    /// Buffer view holding the bitstream.
    pub buffer_view: Index,
    /// glTF attribute semantic → Draco unique attribute id.
    pub attributes: IndexMap<String, u32>,
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

impl Extension for KhrDracoMeshCompression {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if !cx.owner.contains("/primitives/") {
        cx.wrong_location(r, "a mesh primitive");
        return;
    }
    let _ = cx.decode::<KhrDracoMeshCompression>(r);
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
        let v = serde_json::json!({"bufferView": 2, "attributes": {"POSITION": 0, "NORMAL": 1}});
        let e: KhrDracoMeshCompression = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.attributes["NORMAL"], 1);
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }
}
