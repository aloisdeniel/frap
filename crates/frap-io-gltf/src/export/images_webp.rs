// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_texture_webp` export (WP 7.3): `export_image_format='WEBP'`, `export_image_add_webp`,
//! `export_image_webp_fallback` and WebP sources.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/material/texture.py` (`__gather_source`,
//! `__gather_extensions`, `__make_webp_image`), Apache-2.0, The glTF-Blender-IO authors
//! (`export_keep_originals` is not honoured by frap, so its branches are left out). Given
//! the image `image.gather_image` made for a texture (`source`, whose MIME type
//! `images::mime_for` chose):
//!
//! 1. `__gather_source`: unless `export_image_format='WEBP'`, a WebP `source` with
//!    `export_image_webp_fallback` gets a PNG fallback (the same image re-encoded), which
//!    becomes the texture's `source`; the WebP image moves to the extension.
//! 2. `__gather_extensions`:
//!    - `export_image_format='WEBP'`: the WebP `source` is the extension's, the texture has no
//!      `source`, the extension is required;
//!    - `export_image_add_webp` (another format) and a `source` that is not WebP: a WebP image
//!      is made from the same channels (its file bytes when the image is a WebP file, else
//!      re-encoded) and becomes the extension's `source`; the texture keeps its own;
//!    - a WebP `source` without fallback: the extension's, no texture `source`, required;
//!    - a WebP image and the fallback: the WebP image is the extension's.
//!
//! The rules run in this order, so with both `export_image_add_webp` and the fallback a WebP
//! source makes a second WebP image (verified on 5.2.4: in `GLTF_SEPARATE` the first one takes
//! `w.webp` without being written and the extension's is `w-1.webp`, its PNG fallback
//! `w.png`). Every image made takes its `GLTF_SEPARATE` file name in this order
//! ([`images::make_image`]).

use crate::export::ExportError;
use crate::export::ExportImageFormat;
use crate::export::gather::Gather;
use crate::export::images::{self, ExportImage, PNG, PendingImage, Target, WEBP};

/// The props this file implements.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_image_add_webp", "export_image_webp_fallback"];

/// The texture's own `source` and the `EXT_texture_webp` source (with whether the extension
/// is required) for the image `source` gathered from `ex` (see the module documentation).
#[allow(clippy::type_complexity)]
pub(crate) fn gather(
    g: &mut Gather,
    ex: &ExportImage,
    source: PendingImage,
) -> Result<(Option<PendingImage>, Option<(PendingImage, bool)>), ExportError> {
    let p = &g.settings.props;
    let (add_webp, fallback) = (
        p.export_image_add_webp.value,
        p.export_image_webp_fallback.value,
    );
    let webp_format = g.settings.export_image_format == ExportImageFormat::Webp;
    let mut source = source;
    let mut webp_image: Option<PendingImage> = None;

    // `__gather_source`.
    if !webp_format && source.mime == WEBP && fallback {
        // "We should not have empty data here, as we are calculating fallback, so the real
        // image should be ok already".
        if let Some(bytes) = images::encode(g, ex, Target::Png)? {
            let png = images::make_image(g, bytes, PNG, source.name.clone());
            // "We inverted the png & WebP image, to have the png as main source".
            webp_image = Some(std::mem::replace(&mut source, png));
        }
    }

    // `__gather_extensions`.
    let mut ext: Option<PendingImage> = None;
    let (mut remove_source, mut required) = (false, false);
    if webp_format {
        ext = Some(source.clone());
        remove_source = true;
        required = true;
    }
    if !webp_format && add_webp && source.mime != WEBP {
        let Some(bytes) = images::encode(g, ex, Target::Webp)? else {
            g.warn("Image data is empty, not exporting image");
            // `return None, False`: no extension, the texture keeps its source.
            return Ok((Some(source), None));
        };
        let w = images::make_image(g, bytes, WEBP, source.name.clone());
        webp_image = Some(w.clone());
        ext = Some(w);
    }
    if !webp_format && source.mime == WEBP && !fallback {
        ext = Some(source.clone());
        remove_source = true;
        required = true;
    }
    if !webp_format && fallback && webp_image.is_some() {
        ext = webp_image;
    }
    let source = (!remove_source).then_some(source);
    Ok((source, ext.map(|e| (e, required))))
}
