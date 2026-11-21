// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_texture_basisu` (§17.1). WP 7.3 (M7 W0 stub: the typed model).
//!
//! On a texture: `source` is the image in the extension's format; the texture's own `source`,
//! when present, is the fallback image. The image is a KTX2 file with Basis Universal supercompression (`image/ktx2`).

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "KHR_texture_basisu";
const REQUIRED: bool = false;

/// The extension on a texture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KhrTextureBasisu {
    /// Image in this extension's format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Index>,
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

impl Extension for KhrTextureBasisu {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("textures").is_none() {
        cx.wrong_location(r, "a texture");
        return;
    }
    let _ = cx.decode::<KhrTextureBasisu>(r);
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
        let v = serde_json::json!({"source": 3});
        let e: KhrTextureBasisu = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.source, Some(3));
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }
}
