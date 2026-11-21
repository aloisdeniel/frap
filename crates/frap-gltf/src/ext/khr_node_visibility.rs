// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_node_visibility` (§17.1). WP 7.4 (M7 W0 stub: the typed model).
//!
//! On a node: `visible` (default true) hides the node and its descendants. Written from
//! `hide_render` with `frap_node_visibility` (Blender's exporter does not produce it).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras};
use crate::validate::Report;

const NAME: &str = "KHR_node_visibility";
const REQUIRED: bool = false;

/// The extension on a node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrNodeVisibility {
    /// Whether the node is visible (default true).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
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

impl Extension for KhrNodeVisibility {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("nodes").is_none() {
        cx.wrong_location(r, "a node");
        return;
    }
    let _ = cx.decode::<KhrNodeVisibility>(r);
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
        let v = serde_json::json!({"visible": false});
        let e: KhrNodeVisibility = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.visible, Some(false));
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }
}
