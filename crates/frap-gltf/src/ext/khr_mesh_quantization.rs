// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_mesh_quantization` (§17.1). WP 7.1 (M7 W0 stub).
//!
//! The extension has no JSON object: listing it in `extensionsUsed` (and `extensionsRequired`,
//! as writers must) allows the integer component types of vertex attributes the core spec
//! forbids (`POSITION` as (normalized) BYTE/SHORT, `NORMAL`/`TANGENT` as normalized BYTE/SHORT,
//! `TEXCOORD_n` as (normalized) BYTE/SHORT, morph targets likewise). The accessor codec
//! (`accessor.rs`) already reads them; WP 7.1 adds the validator's attribute-type rule.

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef};
use crate::validate::Report;

const NAME: &str = "KHR_mesh_quantization";
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
}
