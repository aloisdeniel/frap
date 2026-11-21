// SPDX-License-Identifier: GPL-3.0-or-later
//! WebP: lossless (`image` crate), lossy behind the `webp-lossy` feature (WP 5.4).
//!
//! Blender 5.2.4 (`format_webp.cc` → OIIO / libwebp) promotes grey + alpha to RGBA, writes
//! quality 100 lossless and anything else lossy at that quality (method 4). Blender cannot write
//! one-channel WebP (`render.render` with `color_mode='BW'` fails to save); this encoder still
//! accepts grey images for other callers.
//!
//! `--stmt-timeout` (ARCHITECTURE §12): neither libwebp nor `image-webp` polls frap's budget,
//! so under a deadline the encoder runs on a worker thread and the statement's thread waits
//! for it until the deadline, then unwinds with `Interrupted` (`TIMEOUT`) at once. The worker
//! is detached: it finishes encoding in the background and its bytes are dropped (no file is
//! written), costing that CPU time but not the statement's answer.

use super::raw8::color_type_8;
use crate::codec::{DecodedImage, ImageError};

pub(crate) fn encode_webp(
    image: &DecodedImage,
    lossless: bool,
    quality: u8,
) -> Result<Vec<u8>, ImageError> {
    use image::ImageEncoder;
    let promoted;
    let image = if image.channels == 2 {
        promoted = image.with_channels(4)?;
        &promoted
    } else {
        image
    };
    if !lossless {
        return encode_webp_lossy(image, quality);
    }
    let (data, width, height, channels) =
        (image.to_u8(), image.width, image.height, image.channels);
    cancellable(move || {
        let mut out = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut out)
            .write_image(&data, width, height, color_type_8(channels))
            .map_err(|e| ImageError::Encode(format!("WEBP: {e}")))?;
        Ok(out)
    })
}

/// Runs `encode` so that the statement's deadline is honoured (see the module docs): inline
/// without a deadline, else on a detached worker the caller waits for until the deadline.
pub(super) fn cancellable<F>(encode: F) -> Result<Vec<u8>, ImageError>
where
    F: FnOnce() -> Result<Vec<u8>, ImageError> + Send + 'static,
{
    use std::sync::mpsc::RecvTimeoutError;
    let Some(deadline) = frap_math::budget::deadline() else {
        return encode();
    };
    frap_math::budget::check_now();
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("frap-webp-encode".into())
        .spawn(move || {
            let _ = tx.send(encode());
        });
    if let Err(e) = spawned {
        return Err(ImageError::Encode(format!("WEBP: no encoder thread: {e}")));
    }
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(left) {
            Ok(r) => return r,
            Err(RecvTimeoutError::Timeout) => frap_math::budget::check_now(),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(ImageError::Encode("WEBP: the encoder panicked".into()));
            }
        }
    }
}

#[cfg(feature = "webp-lossy")]
fn encode_webp_lossy(image: &DecodedImage, quality: u8) -> Result<Vec<u8>, ImageError> {
    let img = match image.channels {
        1 => image.with_channels(3)?,
        2 => image.with_channels(4)?,
        _ => image.clone(),
    };
    let (data, width, height, rgba) = (img.to_u8(), img.width, img.height, img.channels == 4);
    cancellable(move || {
        let enc = if rgba {
            webp::Encoder::from_rgba(&data, width, height)
        } else {
            webp::Encoder::from_rgb(&data, width, height)
        };
        let mem = enc
            .encode_simple(false, f32::from(quality.min(100)))
            .map_err(|e| ImageError::Encode(format!("WEBP: {e:?}")))?;
        Ok(mem.to_vec())
    })
}

#[cfg(not(feature = "webp-lossy"))]
fn encode_webp_lossy(_image: &DecodedImage, _quality: u8) -> Result<Vec<u8>, ImageError> {
    Err(ImageError::Unsupported(
        "lossy WebP encoding needs frap built with the `webp-lossy` feature (lossless WebP \
         is always available)"
            .into(),
    ))
}
