// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF animation export (WP 6.7, `crates/frap-io-gltf/src/export/animation/**`): every
//! `export_animation_mode` (`ACTIONS`, `ACTIVE_ACTIONS`, `BROADCAST`, `NLA_TRACKS`, `SCENE`),
//! TRS and morph-weight channels, sampling (`export_force_sampling`, `export_frame_step`) vs
//! CUBICSPLINE vs STEP/LINEAR, the `fps_base` quirk, ported from io_scene_gltf2 5.2.41
//! `blender/exp/animation/**`. Channel targets resolve through `export::node_map::NodeMap`.
//!
//! M5+M6 W0: [`gather`] runs after the scenes (the add-on's `__gather_animations`); it writes
//! nothing until WP 6.7 lands.

use crate::export::ExportError;
use crate::export::gather::Gather;

/// Gathers `doc.animations` for the export (after `scenes::gather`).
pub fn gather(g: &mut Gather) -> Result<(), ExportError> {
    let _ = g;
    Ok(())
}
