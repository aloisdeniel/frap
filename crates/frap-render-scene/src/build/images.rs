// SPDX-License-Identifier: GPL-3.0-or-later
//! Images the render needs (material textures, environment textures), decoded once per build
//! and keyed by `(IdKey, generation)`.
//!
//! Sources: a packed file is decoded from its bytes; a FILE image is read from `filepath`
//! (`//` is relative to the directory of `bpy.data.filepath`, or to the working directory for
//! an unsaved file; other relative paths are relative to the working directory); a GENERATED
//! image is filled with `generated_color` (linear, `generated_width × generated_height`).
//! An image that cannot be loaded renders as Blender's missing-texture magenta (1, 0, 1, 1).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use frap_imbuf::{DecodedImage, PixelData};
use frap_rna::{IdKey, IdRef, ImageId};
use frap_scene::Main;
use frap_scene::id::Image;
use indexmap::IndexMap;

use super::keys::image_key;
use crate::types::ImageKey;

/// Collects decoded images while the scene is built.
#[derive(Default)]
pub struct Images {
    /// Decoded images in first-use order.
    pub map: IndexMap<ImageKey, Arc<DecodedImage>>,
    by_id: HashMap<IdKey, ImageKey>,
}

/// Blender's missing-texture colour.
pub fn missing() -> DecodedImage {
    DecodedImage {
        width: 1,
        height: 1,
        channels: 4,
        pixels: PixelData::F32(vec![1.0, 0.0, 1.0, 1.0]),
    }
}

/// Where an image file is read from.
pub fn resolve_path(filepath: &str, blend_path: Option<&str>) -> PathBuf {
    if let Some(rel) = filepath.strip_prefix("//") {
        let base = blend_path
            .and_then(|p| Path::new(p).parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        return base.join(rel);
    }
    PathBuf::from(filepath)
}

/// Decodes `im` (see the module documentation); `None` when it cannot be loaded.
pub fn decode(im: &Image, blend_path: Option<&str>) -> Option<DecodedImage> {
    if let Some(p) = &im.packed_file {
        return frap_imbuf::decode(&p.bytes).ok();
    }
    match im.source.as_str() {
        "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if !im.filepath.is_empty() => {
            frap_imbuf::load(&resolve_path(&im.filepath, blend_path)).ok()
        }
        "GENERATED" => {
            let (w, h) = (
                im.generated_width.max(1) as u32,
                im.generated_height.max(1) as u32,
            );
            let n = (w as usize) * (h as usize);
            let mut px = Vec::with_capacity(n * 4);
            for _ in 0..n {
                px.extend_from_slice(&im.generated_color);
            }
            Some(DecodedImage {
                width: w,
                height: h,
                channels: 4,
                pixels: PixelData::F32(px),
            })
        }
        _ => None,
    }
}

impl Images {
    /// The key of `r`, decoding it on first use.
    pub fn key(&mut self, main: &Main, r: IdRef<ImageId>) -> ImageKey {
        if let Some(k) = self.by_id.get(&r.key()) {
            return *k;
        }
        let (k, img) = match main.images.get(r) {
            Some(im) => (
                image_key(r.key(), &im.header),
                decode(im, main.filepath.as_deref()).unwrap_or_else(missing),
            ),
            // A dangling reference: a stable key per IdKey, magenta pixels.
            None => (ImageKey(r.key().get() ^ 0xdead_beef_0000_0000), missing()),
        };
        self.by_id.insert(r.key(), k);
        self.map.entry(k).or_insert_with(|| Arc::new(img));
        k
    }
}
