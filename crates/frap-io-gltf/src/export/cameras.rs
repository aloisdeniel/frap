// SPDX-License-Identifier: GPL-3.0-or-later
//! Cameras → glTF `cameras[]` (M3, WP 3.4; stub created by M3 W0). Port target:
//! io_scene_gltf2 5.2.41 `blender/exp/camera.py` (perspective `yfov` from the sensor fit and
//! render aspect, orthographic `xmag`/`ymag`, `znear`/`zfar`), with `export_cameras` (default
//! False) deciding whether CAMERA objects are kept by the vtree filter
//! (`node_filter_not_inheritable_is_kept`).
//!
//! W0 wiring: `vtree.rs` asks [`exported`], `nodes.rs` calls [`gather`] for every kept object
//! (before its children, as the add-on's alphabetical member order puts `camera` first).

use frap_gltf::json::Index;
use frap_scene::id::Object;

use crate::export::gather::Gather;
use crate::export::{ExportError, ExportSettings};

/// Whether CAMERA objects are exported (`export_cameras`). M1 behaviour until WP 3.4: never.
pub fn exported(settings: &ExportSettings) -> bool {
    let _ = settings;
    false
}

/// The glTF camera of a CAMERA object (`None` for other objects).
pub fn gather(g: &mut Gather, ob: &Object) -> Result<Option<Index>, ExportError> {
    let _ = (g, ob);
    Ok(None)
}
