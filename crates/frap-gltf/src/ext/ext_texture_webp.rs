// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_texture_webp` (§17.1). WP 7.3 (M7 W0 stub: the typed model).
//!
//! On a texture: `source` is the image in the extension's format; the texture's own `source`,
//! when present, is the fallback image. The importer reads it since M3 (`import/images.rs`, raw JSON); WP 7.3 writes it.

use serde::{Deserialize, Serialize};

use crate::json::{EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, Index};
use crate::validate::Report;

const NAME: &str = "EXT_texture_webp";
const REQUIRED: bool = false;

/// The extension on a texture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtTextureWebp {
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

impl Extension for ExtTextureWebp {
    const NAME: &'static str = NAME;
    const REQUIRED_BY_DEFAULT: bool = REQUIRED;
}

/// The rules shared by the texture-source extensions (`EXT_texture_webp`,
/// `KHR_texture_basisu`): `source` is required and names an existing image, whose declared
/// `mimeType` (when it has one) is `mime`; without a core `source` on the texture the
/// extension must be required (a viewer without it has nothing to show).
pub(crate) fn check_texture_source(
    cx: &ExtensionCx,
    r: &mut Report,
    name: &str,
    source: Option<Index>,
    mime: &str,
) {
    let Some(s) = source else {
        r.error(
            "UNDEFINED_PROPERTY",
            cx.member("source"),
            "source is required",
        );
        return;
    };
    let Some(img) = cx.doc.images.get(s as usize) else {
        r.error(
            "UNRESOLVED_REFERENCE",
            cx.member("source"),
            format!("image {s} does not exist ({} defined)", cx.doc.images.len()),
        );
        return;
    };
    if let Some(m) = img.mime_type.as_deref()
        && m != mime
    {
        r.error(
            "IMAGE_MIME_TYPE_INVALID",
            format!("/images/{s}/mimeType"),
            format!("the {name} source must be {mime}, not {m}"),
        );
    }
    if cx.owner_value.get("source").is_none()
        && !cx.doc.extensions_required.iter().any(|e| e == name)
    {
        r.warning(
            "EXTENSION_NOT_REQUIRED_WITHOUT_FALLBACK",
            cx.pointer(),
            format!(
                "a texture whose only image is the {name} source needs {name} in extensionsRequired"
            ),
        );
    }
}

fn validate(cx: &ExtensionCx, r: &mut Report) {
    if cx.owner_index_in("textures").is_none() {
        cx.wrong_location(r, "a texture");
        return;
    }
    if let Some(e) = cx.decode::<ExtTextureWebp>(r) {
        check_texture_source(cx, r, NAME, e.source, "image/webp");
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
        let v = serde_json::json!({"source": 3});
        let e: ExtTextureWebp = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.source, Some(3));
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }

    /// A document with one texture carrying the extension (`source`, core `source`).
    fn doc(ext_source: Option<Index>, core: Option<Index>, required: bool) -> Document {
        let mut d = Document::default();
        for mime in ["image/webp", "image/png"] {
            d.images.push(crate::json::Image {
                mime_type: Some(mime.into()),
                uri: Some("x".into()),
                ..Default::default()
            });
        }
        let mut t = crate::json::Texture {
            source: core,
            ..Default::default()
        };
        t.extensions.set(Some(ExtTextureWebp {
            source: ext_source,
            ..Default::default()
        }));
        d.textures.push(t);
        d.use_extension(NAME, required);
        d
    }

    use crate::json::Document;
    use crate::validate::validate;

    #[test]
    fn validation_rules() {
        // The add-on's two shapes: WebP + PNG fallback, and WebP alone (required).
        assert!(validate(&doc(Some(0), Some(1), false)).is_ok());
        let r = validate(&doc(Some(0), None, true));
        assert!(
            r.is_ok() && !r.has("EXTENSION_NOT_REQUIRED_WITHOUT_FALLBACK"),
            "{r}"
        );
        // Without a fallback and not required: a warning.
        let r = validate(&doc(Some(0), None, false));
        assert!(
            r.is_ok() && r.has("EXTENSION_NOT_REQUIRED_WITHOUT_FALLBACK"),
            "{r}"
        );
        // Missing, dangling, or of another MIME type.
        assert!(validate(&doc(None, Some(1), false)).has("UNDEFINED_PROPERTY"));
        assert!(validate(&doc(Some(7), Some(1), false)).has("UNRESOLVED_REFERENCE"));
        assert!(validate(&doc(Some(1), Some(1), false)).has("IMAGE_MIME_TYPE_INVALID"));
    }
}
