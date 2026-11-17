// SPDX-License-Identifier: GPL-3.0-or-later
//! Applying animation to originals through `Host::set_path` at evaluation boundaries (§7.3,
//! §18). WP 1.13 provides the boundary API (no-op until M6).
//!
//! Blender applies animation only when the depsgraph evaluates after a time change: after
//! `scene.frame_current = 6` an animated `location` keeps its old value until the next operator,
//! `view_layer.update()`, `scene.frame_set()` or `evaluated_depsgraph_get()` (verified). The
//! `frame_current` setter therefore only marks time dirty (`Host::mark_time_dirty`), and each
//! of those boundaries calls [`apply_pending_animation`] with that flag. Render and export call
//! [`apply_animation_at`] on a COW clone of the state (`Host::snapshot_at`), never on the
//! session's originals.
//!
//! Until M6 (WP 6.4) there are no F-curves to evaluate: both functions leave the host untouched
//! (no write, no report, no generation bump) and report zero channels written.

use frap_rna::RnaError;
use frap_scene::Host;

/// What an evaluation boundary wrote.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AnimationApplied {
    /// Animated channels written into originals through `Host::set_path`.
    pub channels: usize,
}

/// An evaluation boundary (operator pipeline step 4, `view_layer.update()`,
/// `scene.frame_set()`, `context.evaluated_depsgraph_get()` / `evaluated_get()`): when time is
/// dirty, writes the animation of the context scene's `frame_current` into the originals.
/// The caller clears its time-dirty flag after an `Ok`.
pub fn apply_pending_animation(
    host: &mut dyn Host,
    time_dirty: bool,
) -> Result<AnimationApplied, RnaError> {
    if !time_dirty {
        return Ok(AnimationApplied::default());
    }
    let frame = host
        .main()
        .scenes
        .get(host.win().scene)
        .map_or(0, |s| s.frame_current);
    apply_animation_at(host, frame as f32)
}

/// Writes the animation of every animated ID at `frame` into `host`'s originals (a session
/// boundary, or a render/export clone). No-op until M6.
pub fn apply_animation_at(host: &mut dyn Host, frame: f32) -> Result<AnimationApplied, RnaError> {
    // M6 (WP 6.4): evaluate each `AnimData` (action slot, NLA) at `frame` and write every
    // channel through `host.set_path(owner, data_path, index, value)`.
    let _ = (host, frame);
    Ok(AnimationApplied::default())
}
