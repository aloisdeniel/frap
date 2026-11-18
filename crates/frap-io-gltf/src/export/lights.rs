// SPDX-License-Identifier: GPL-3.0-or-later
//! Lights → `KHR_lights_punctual` (M3, WP 3.4; stub created by M3 W0). Port target:
//! io_scene_gltf2 5.2.41 `blender/exp/light.py` and `io/com/lights_punctual.py` (W → cd / lux
//! with `export_import_convert_lighting_mode`, spot cone angles from `spot_size`/`spot_blend`,
//! AREA lights skipped with a WARNING, the extension in `extensionsRequired`), with
//! `export_lights` (default False) deciding whether LIGHT objects are kept by the vtree filter.
//!
//! W0 wiring: `vtree.rs` asks [`exported`], `nodes.rs` calls [`apply`] on every node it writes.

use frap_gltf::json::Node;
use frap_scene::id::Object;

use crate::export::gather::Gather;
use crate::export::{ExportError, ExportSettings};

/// Whether LIGHT objects are exported (`export_lights`). M1 behaviour until WP 3.4: never.
pub fn exported(settings: &ExportSettings) -> bool {
    let _ = settings;
    false
}

/// Adds the node's `KHR_lights_punctual` reference (and the light to the root extension) for a
/// LIGHT object; nothing for other objects.
pub fn apply(g: &mut Gather, ob: &Object, node: &mut Node) -> Result<(), ExportError> {
    let _ = (g, ob, node);
    Ok(())
}
