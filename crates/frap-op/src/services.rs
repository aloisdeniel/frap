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

/// What an `ed.*` undo operator asks the session for (M8 W0 contract, decision log M8W0-5;
/// WP 8.4 implements it). An undo operator changes nothing itself: it records the request
/// with [`Services::undo`], and the session applies it to its ring of committed states once
/// the statement has committed (an undo step is never rolled back into, and a failed or
/// CANCELLED statement is not a step).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UndoRequest {
    /// `ed.undo()`: back to the state before the last undo step.
    Undo,
    /// `ed.redo()`: forward again.
    Redo,
    /// `ed.undo_redo()`: undo the last step and redo it.
    UndoRedo,
    /// `ed.undo_push(message=…)`: a named step at the current state.
    Push {
        /// The step's name (`message`).
        message: String,
    },
}

/// What the session's undo ring allows right now, as the polls of `ed.undo`, `ed.redo` and
/// `ed.undo_redo` see it (WP 8.4: polls only receive the [`frap_scene::Context`], and the ring
/// lives outside the snapshot). The session publishes it with [`set_undo_status`] before and
/// after every statement; a thread without a session reads the default (nothing to undo).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct UndoStatus {
    /// `ed_undo_poll`: the active step has a previous one.
    pub can_undo: bool,
    /// `ed_redo_poll`: the active step has a next one.
    pub can_redo: bool,
    /// `ed_undo_redo_poll`: a last registered `UNDO` operator whose step can be undone.
    pub can_undo_redo: bool,
}

thread_local! {
    static UNDO_STATUS: std::cell::Cell<UndoStatus> = const {
        std::cell::Cell::new(UndoStatus {
            can_undo: false,
            can_redo: false,
            can_undo_redo: false,
        })
    };
}

/// The undo status the running session published (WP 8.4).
pub fn undo_status() -> UndoStatus {
    UNDO_STATUS.with(std::cell::Cell::get)
}

/// Publishes the session's undo status for the `ed.*` polls (WP 8.4).
pub fn set_undo_status(s: UndoStatus) {
    UNDO_STATUS.with(|c| c.set(s));
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
    /// Records an undo request for the session to apply after the statement (M8 W0 contract,
    /// [`UndoRequest`]). The default answers `NotImplementedError` (a host without an undo
    /// ring).
    fn undo(&mut self, req: UndoRequest) -> Result<(), OpError> {
        Err(OpError::NotImplemented(format!(
            "frap: this session keeps no undo history ({req:?})"
        )))
    }
}
