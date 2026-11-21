// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_texture_basisu` (§17.1). WP 7.3 (M7 W0 stub: the typed model).
//!
//! On a texture: `source` is the image in the extension's format; the texture's own `source`,
//! when present, is the fallback image. The image is a KTX2 file with Basis Universal supercompression (`image/ktx2`).
//!
//! The JSON rules run in [`crate::validate`]; the rules of the KTX2 image itself (the
//! extension's "KTX v2 Images with Basis Universal Supercompression" section) run on its
//! bytes: in [`crate::validate_data`] for images stored in a buffer view or a `data:` URI
//! ([`validate_images`]), and through [`ktx2_issues`] for any other bytes (an image file
//! next to a `.gltf`, an exporter checking what it writes).

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
    if let Some(e) = cx.decode::<KhrTextureBasisu>(r) {
        super::ext_texture_webp::check_texture_source(cx, r, NAME, e.source, "image/ktx2");
    }
}

/// How a texture is sampled by the materials, which fixes the KTX2 colour space.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Usage {
    /// Colour data (base colour, emissive, sheen colour, specular colour, …): primaries
    /// `KHR_DF_PRIMARIES_BT709`, transfer function `KHR_DF_TRANSFER_SRGB`.
    Colour,
    /// Non-colour data (normal, metallic-roughness, occlusion, …): primaries
    /// `KHR_DF_PRIMARIES_UNSPECIFIED`, transfer function `KHR_DF_TRANSFER_LINEAR`.
    Data,
}

/// The texture-info members that hold colour (sRGB) data: core, `KHR_materials_sheen`,
/// `KHR_materials_specular`, `KHR_materials_pbrSpecularGlossiness`,
/// `KHR_materials_diffuse_transmission`. Every other `*Texture` member is non-colour data.
pub const COLOUR_TEXTURES: &[&str] = &[
    "baseColorTexture",
    "emissiveTexture",
    "sheenColorTexture",
    "specularColorTexture",
    "diffuseTexture",
    "specularGlossinessTexture",
    "diffuseTransmissionColorTexture",
];

/// `KHR_DF_MODEL_ETC1S`.
pub const DF_MODEL_ETC1S: u8 = 163;
/// `KHR_DF_MODEL_UASTC`.
pub const DF_MODEL_UASTC: u8 = 166;
/// `KHR_DF_PRIMARIES_UNSPECIFIED`.
pub const DF_PRIMARIES_UNSPECIFIED: u8 = 0;
/// `KHR_DF_PRIMARIES_BT709`.
pub const DF_PRIMARIES_BT709: u8 = 1;
/// `KHR_DF_TRANSFER_LINEAR`.
pub const DF_TRANSFER_LINEAR: u8 = 1;
/// `KHR_DF_TRANSFER_SRGB`.
pub const DF_TRANSFER_SRGB: u8 = 2;

const KTX2_IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// Byte offset of the first word of the DFD's basic descriptor block that holds
/// `colorModel`, `colorPrimaries`, `transferFunction` and `flags` (`dfdByteOffset` + the
/// 4-byte `dfdTotalSize` + the 8-byte block header), when the file is long enough to hold it.
pub fn ktx2_dfd_model_offset(bytes: &[u8]) -> Option<usize> {
    let dfd = usize::try_from(u32_at(bytes, 48)?).ok()?;
    let at = dfd.checked_add(12)?;
    (bytes.len() >= at.checked_add(4)?).then_some(at)
}

/// The key/value pairs of a KTX2 file (`kvdByteOffset`, `kvdByteLength`), values without
/// their terminating NUL.
fn ktx2_key_values(bytes: &[u8]) -> Vec<(&[u8], &[u8])> {
    let (Some(off), Some(len)) = (u32_at(bytes, 56), u32_at(bytes, 60)) else {
        return Vec::new();
    };
    let (off, len) = (off as usize, len as usize);
    let Some(kvd) = off.checked_add(len).and_then(|end| bytes.get(off..end)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(n) = u32_at(kvd, i) {
        let n = n as usize;
        let Some(e) = i.checked_add(4).and_then(|s| kvd.get(s..s.checked_add(n)?)) else {
            break;
        };
        if let Some(z) = e.iter().position(|&c| c == 0) {
            let v = &e[z + 1..];
            out.push((&e[..z], v.strip_suffix(&[0]).unwrap_or(v)));
        }
        i += 4 + n.div_ceil(4) * 4;
    }
    out
}

/// The rules of the extension's specification that a KTX2 image breaks, as `(code,
/// message)`: the KTX2 identifier, a 2D texture (`pixelHeight` > 0, `pixelDepth` 0, one
/// face, no array layers), `vkFormat` `VK_FORMAT_UNDEFINED`, `pixelWidth` and `pixelHeight`
/// multiples of 4, the DFD colour model ETC1S with BasisLZ supercompression or UASTC with
/// none or Zstandard, `KTXswizzle` `rgba` and `KTXorientation` `rd` when present, and, for a
/// known `usage`, the colour primaries and transfer function it requires.
pub fn ktx2_issues(bytes: &[u8], usage: Option<Usage>) -> Vec<(&'static str, String)> {
    let mut v: Vec<(&'static str, String)> = Vec::new();
    if bytes.get(..12) != Some(&KTX2_IDENTIFIER[..]) || bytes.len() < 80 {
        v.push(("KTX2_INVALID", "not a KTX2 file".into()));
        return v;
    }
    let h = |i: usize| u32_at(bytes, 12 + 4 * i).unwrap_or(0);
    let (vk, width, height, depth, layers, faces, scheme) =
        (h(0), h(2), h(3), h(4), h(5), h(6), h(8));
    if vk != 0 {
        v.push((
            "KTX2_FORMAT_INVALID",
            format!("vkFormat is {vk}; a Basis Universal image is VK_FORMAT_UNDEFINED (0)"),
        ));
    }
    if height == 0 || depth != 0 || faces != 1 || layers != 0 {
        v.push((
            "KTX2_NOT_2D",
            format!(
                "the image must be 2D (pixelHeight {height}, pixelDepth {depth}, faceCount \
                 {faces}, layerCount {layers})"
            ),
        ));
    }
    if !width.is_multiple_of(4) || !height.is_multiple_of(4) {
        v.push((
            "KTX2_DIMENSIONS_NOT_MULTIPLE_OF_4",
            format!("pixelWidth and pixelHeight must be multiples of 4, not {width}x{height}"),
        ));
    }
    match ktx2_dfd_model_offset(bytes) {
        None => v.push(("KTX2_INVALID", "the data format descriptor is missing".into())),
        Some(at) => {
            let (model, primaries, transfer) = (bytes[at], bytes[at + 1], bytes[at + 2]);
            match (model, scheme) {
                (DF_MODEL_ETC1S, 1) | (DF_MODEL_UASTC, 0 | 2) => {}
                (DF_MODEL_ETC1S, s) => v.push((
                    "KTX2_SUPERCOMPRESSION_INVALID",
                    format!("ETC1S needs BasisLZ supercompression (1), not {s}"),
                )),
                (DF_MODEL_UASTC, s) => v.push((
                    "KTX2_SUPERCOMPRESSION_INVALID",
                    format!("UASTC needs no supercompression (0) or Zstandard (2), not {s}"),
                )),
                (m, _) => v.push((
                    "KTX2_COLOR_MODEL_INVALID",
                    format!(
                        "DFD colorModel is {m}; it must be KHR_DF_MODEL_ETC1S (163) or \
                         KHR_DF_MODEL_UASTC (166)"
                    ),
                )),
            }
            let want = match usage {
                Some(Usage::Colour) => Some((DF_PRIMARIES_BT709, DF_TRANSFER_SRGB, "colour")),
                Some(Usage::Data) => Some((
                    DF_PRIMARIES_UNSPECIFIED,
                    DF_TRANSFER_LINEAR,
                    "non-colour",
                )),
                None => None,
            };
            if let Some((p, t, what)) = want {
                if primaries != p {
                    v.push((
                        "KTX2_COLOR_PRIMARIES_INVALID",
                        format!("DFD colorPrimaries is {primaries}; {what} data needs {p}"),
                    ));
                }
                if transfer != t {
                    v.push((
                        "KTX2_TRANSFER_FUNCTION_INVALID",
                        format!("DFD transferFunction is {transfer}; {what} data needs {t}"),
                    ));
                }
            }
        }
    }
    for (k, val) in ktx2_key_values(bytes) {
        let (key, want, code) = match k {
            b"KTXswizzle" => ("KTXswizzle", &b"rgba"[..], "KTX2_SWIZZLE_INVALID"),
            b"KTXorientation" => ("KTXorientation", &b"rd"[..], "KTX2_ORIENTATION_INVALID"),
            _ => continue,
        };
        if val != want {
            v.push((
                code,
                format!(
                    "{key} is {:?}; it must be {:?} or omitted",
                    String::from_utf8_lossy(val),
                    String::from_utf8_lossy(want)
                ),
            ));
        }
    }
    v
}

/// The usage of every texture the materials sample: [`Usage::Colour`] from a
/// [`COLOUR_TEXTURES`] member, [`Usage::Data`] from any other `*Texture` member, `None` when
/// one texture is sampled both ways.
pub fn texture_usages(doc: &crate::json::Document) -> Vec<Option<Option<Usage>>> {
    fn walk(v: &serde_json::Value, out: &mut [Option<Option<Usage>>]) {
        let serde_json::Value::Object(m) = v else {
            if let serde_json::Value::Array(a) = v {
                a.iter().for_each(|e| walk(e, out));
            }
            return;
        };
        for (k, e) in m {
            if k.ends_with("Texture")
                && let Some(i) = e.get("index").and_then(serde_json::Value::as_u64)
                && let Some(slot) = out.get_mut(i as usize)
            {
                let u = if COLOUR_TEXTURES.contains(&k.as_str()) {
                    Usage::Colour
                } else {
                    Usage::Data
                };
                *slot = match *slot {
                    None => Some(Some(u)),
                    Some(Some(o)) if o == u => Some(Some(u)),
                    _ => Some(None),
                };
            }
            walk(e, out);
        }
    }
    let mut out = vec![None; doc.textures.len()];
    if let Ok(m) = serde_json::to_value(&doc.materials) {
        walk(&m, &mut out);
    }
    out
}

/// [`ktx2_issues`] for the `KHR_texture_basisu` source of every texture whose image bytes are
/// in a buffer view or a `data:` URI, the colour space checked against the texture's usage
/// ([`texture_usages`]). Images that cannot be read are left to the other rules.
pub fn validate_images<B: AsRef<[u8]>>(
    doc: &crate::json::Document,
    buffers: &[B],
    r: &mut Report,
) {
    let usages = texture_usages(doc);
    // An image shared by several textures is checked once per usage.
    let mut seen: Vec<(Index, Option<Usage>)> = Vec::new();
    for (t, tex) in doc.textures.iter().enumerate() {
        let Ok(Some(e)) = tex.extensions.get::<KhrTextureBasisu>() else {
            continue;
        };
        let Some(s) = e.source else { continue };
        let usage = usages.get(t).copied().flatten().flatten();
        if seen.contains(&(s, usage)) {
            continue;
        }
        seen.push((s, usage));
        let Some(img) = doc.images.get(s as usize) else {
            continue;
        };
        let bytes: Vec<u8> = if let Some(bv) = img.buffer_view {
            match crate::accessor::buffer_view_bytes(doc, buffers, bv) {
                Ok(b) => b.to_vec(),
                Err(_) => continue,
            }
        } else if let Some(Some(Ok((_, b)))) =
            img.uri.as_deref().map(crate::accessor::decode_data_uri)
        {
            b
        } else {
            continue;
        };
        for (code, msg) in ktx2_issues(&bytes, usage) {
            r.error(
                code,
                format!("/images/{s}"),
                format!("KHR_texture_basisu image (texture {t}): {msg}"),
            );
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
        let v = serde_json::json!({"source": 3});
        let e: KhrTextureBasisu = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(e.source, Some(3));
        assert_eq!(serde_json::to_value(&e).unwrap(), v);
    }

    #[test]
    fn validation_rules() {
        use crate::json::{Document, Image, Texture};
        use crate::validate::validate;
        let doc = |src: Option<Index>, core: Option<Index>, required: bool| {
            let mut d = Document::default();
            for mime in ["image/ktx2", "image/png"] {
                d.images.push(Image {
                    mime_type: Some(mime.into()),
                    uri: Some("x".into()),
                    ..Default::default()
                });
            }
            let mut t = Texture {
                source: core,
                ..Default::default()
            };
            t.extensions.set(Some(KhrTextureBasisu {
                source: src,
                ..Default::default()
            }));
            d.textures.push(t);
            d.use_extension(NAME, required);
            d
        };
        assert!(validate(&doc(Some(0), Some(1), false)).is_ok());
        assert!(validate(&doc(Some(0), None, true)).is_ok());
        assert!(
            validate(&doc(Some(0), None, false)).has("EXTENSION_NOT_REQUIRED_WITHOUT_FALLBACK")
        );
        assert!(validate(&doc(Some(1), Some(1), false)).has("IMAGE_MIME_TYPE_INVALID"));
        assert!(validate(&doc(Some(9), None, true)).has("UNRESOLVED_REFERENCE"));
        assert!(validate(&doc(None, Some(1), false)).has("UNDEFINED_PROPERTY"));
    }

    /// A minimal KTX2 file: header, one level index entry, a DFD basic block word 2 and the
    /// key/value data `kv`.
    fn ktx(w: u32, h: u32, dfd: [u8; 3], scheme: u32, kv: &[(&str, &str)]) -> Vec<u8> {
        let mut kvd = Vec::new();
        for (k, v) in kv {
            let e = [k.as_bytes(), &[0], v.as_bytes(), &[0]].concat();
            kvd.extend_from_slice(&(e.len() as u32).to_le_bytes());
            kvd.extend_from_slice(&e);
            while !kvd.len().is_multiple_of(4) {
                kvd.push(0);
            }
        }
        let dfd_off = 80 + 24;
        let dfd_len = 16;
        let mut b = KTX2_IDENTIFIER.to_vec();
        for x in [0, 1, w, h, 0, 0, 1, 1, scheme] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        let kvd_off = dfd_off + dfd_len;
        for x in [dfd_off, dfd_len, kvd_off, kvd.len() as u32] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.extend_from_slice(&[0; 16 + 24]);
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&[0; 8]);
        b.extend_from_slice(&[dfd[0], dfd[1], dfd[2], 0]);
        b.extend_from_slice(&kvd);
        b
    }

    fn codes(v: &[(&'static str, String)]) -> Vec<&'static str> {
        v.iter().map(|i| i.0).collect()
    }

    #[test]
    fn ktx2_image_rules() {
        let colour = [DF_MODEL_UASTC, DF_PRIMARIES_BT709, DF_TRANSFER_SRGB];
        let data = [DF_MODEL_UASTC, DF_PRIMARIES_UNSPECIFIED, DF_TRANSFER_LINEAR];
        assert!(ktx2_issues(&ktx(8, 4, colour, 2, &[]), Some(Usage::Colour)).is_empty());
        assert!(ktx2_issues(&ktx(8, 4, data, 0, &[]), Some(Usage::Data)).is_empty());
        let etc1s = [DF_MODEL_ETC1S, DF_PRIMARIES_BT709, DF_TRANSFER_SRGB];
        assert!(ktx2_issues(&ktx(4, 4, etc1s, 1, &[]), None).is_empty());
        assert_eq!(
            codes(&ktx2_issues(&ktx(5, 3, colour, 2, &[]), None)),
            ["KTX2_DIMENSIONS_NOT_MULTIPLE_OF_4"]
        );
        // BT709 with a linear transfer: the primaries are wrong for data.
        let bt709_linear = [DF_MODEL_UASTC, DF_PRIMARIES_BT709, DF_TRANSFER_LINEAR];
        assert_eq!(
            codes(&ktx2_issues(&ktx(4, 4, bt709_linear, 2, &[]), Some(Usage::Data))),
            ["KTX2_COLOR_PRIMARIES_INVALID"]
        );
        assert_eq!(
            codes(&ktx2_issues(&ktx(4, 4, data, 2, &[]), Some(Usage::Colour))),
            [
                "KTX2_COLOR_PRIMARIES_INVALID",
                "KTX2_TRANSFER_FUNCTION_INVALID"
            ]
        );
        assert_eq!(
            codes(&ktx2_issues(&ktx(4, 4, etc1s, 2, &[]), None)),
            ["KTX2_SUPERCOMPRESSION_INVALID"]
        );
        assert_eq!(
            codes(&ktx2_issues(&ktx(4, 4, [1, 1, 2], 0, &[]), None)),
            ["KTX2_COLOR_MODEL_INVALID"]
        );
        let kv = [
            ("KTXwriter", "x"),
            ("KTXorientation", "ru"),
            ("KTXswizzle", "rrr1"),
        ];
        assert_eq!(
            codes(&ktx2_issues(&ktx(4, 4, colour, 2, &kv), None)),
            ["KTX2_ORIENTATION_INVALID", "KTX2_SWIZZLE_INVALID"]
        );
        let ok_kv = [("KTXorientation", "rd"), ("KTXswizzle", "rgba")];
        assert!(ktx2_issues(&ktx(4, 4, colour, 2, &ok_kv), None).is_empty());
        assert_eq!(codes(&ktx2_issues(b"not ktx", None)), ["KTX2_INVALID"]);
    }

    #[test]
    fn validate_data_checks_embedded_images_by_usage() {
        use crate::json::{Document, Image, Texture};
        let mut d = Document::default();
        // Texture 0 is the base colour, texture 1 the metallic-roughness data.
        let images = [
            ktx(4, 4, [DF_MODEL_UASTC, DF_PRIMARIES_BT709, DF_TRANSFER_SRGB], 2, &[]),
            ktx(6, 4, [DF_MODEL_UASTC, DF_PRIMARIES_BT709, DF_TRANSFER_LINEAR], 2, &[]),
        ];
        for b in &images {
            d.images.push(Image {
                mime_type: Some("image/ktx2".into()),
                uri: Some(crate::accessor::encode_data_uri("image/ktx2", b)),
                ..Default::default()
            });
        }
        for i in 0..2 {
            let mut t = Texture::default();
            t.extensions.set(Some(KhrTextureBasisu {
                source: Some(i),
                ..Default::default()
            }));
            d.textures.push(t);
        }
        d.use_extension(NAME, true);
        d.materials.push(
            serde_json::from_value(serde_json::json!({
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": 0},
                    "metallicRoughnessTexture": {"index": 1}
                }
            }))
            .unwrap(),
        );
        assert_eq!(
            texture_usages(&d),
            [Some(Some(Usage::Colour)), Some(Some(Usage::Data))]
        );
        let r = crate::validate::validate_data::<Vec<u8>>(&d, &[]);
        let found: Vec<(&str, &str)> = r
            .issues
            .iter()
            .map(|i| (i.code, i.pointer.as_str()))
            .collect();
        assert_eq!(
            found,
            [
                ("KTX2_DIMENSIONS_NOT_MULTIPLE_OF_4", "/images/1"),
                ("KTX2_COLOR_PRIMARIES_INVALID", "/images/1"),
            ],
            "{r}"
        );
    }
}
