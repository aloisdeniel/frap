// SPDX-License-Identifier: GPL-3.0-or-later
//! Side-effect services that live outside the snapshot and are never rolled back
//! (ARCHITECTURE §10.1, CONTRACT frozen at M1 W0; `PathResolver`/`ImageCache` defined by RFC
//! 0001 F-REN-1). Implemented by `frap-session`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use frap_imbuf::DecodedImage;
use frap_render_scene::RenderService;
use frap_rna::{IdKey, IdRef, ImageId};

use crate::def::OpError;

/// Resolves user paths: `//` relative to the saved file (or the cwd when unsaved), relative
/// paths against the cwd.
#[derive(Clone, Debug)]
pub struct PathResolver {
    /// Working directory.
    pub cwd: PathBuf,
    /// Directory of `bpy.data.filepath`, when saved.
    pub blend_dir: Option<PathBuf>,
}

impl PathResolver {
    /// Absolute path for a user-supplied path.
    pub fn resolve(&self, p: &str) -> PathBuf {
        if let Some(rest) = p.strip_prefix("//") {
            return self.blend_dir.as_deref().unwrap_or(&self.cwd).join(rest);
        }
        let path = Path::new(p);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.cwd.join(path)
        }
    }
}

/// Decoded image pixels by `(IdKey, generation)`, outside the snapshot.
pub trait ImageCache {
    /// Cached pixels of an image at a generation.
    fn get(&self, image: IdRef<ImageId>, generation: u64) -> Option<Arc<DecodedImage>>;
    /// Stores decoded pixels.
    fn put(&mut self, image: IdRef<ImageId>, generation: u64, pixels: Arc<DecodedImage>);
    /// Drops every entry of an image (removed or replaced).
    fn evict(&mut self, key: IdKey);
}

/// Session services reachable from operators.
pub trait Services {
    /// Lazily created GPU render service; `OpError::Gpu` (`GPU_UNAVAILABLE`) when there is no
    /// adapter or the build has no `gpu` feature.
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError>;
    /// Decoded image cache.
    fn images(&mut self) -> &mut dyn ImageCache;
    /// Path resolution (`cwd`, `//` relative to `bpy.data.filepath`).
    fn paths(&self) -> &PathResolver;
}
