// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF morph-target import (WP 6.8): shape keys from `targets` (POSITION / NORMAL deltas,
//! sparse accessors), the mesh's and node's default `weights`, `extras.targetNames` naming,
//! ported from io_scene_gltf2 5.2.41 `blender/imp/mesh.py` (`shape_keys` branch).
//!
//! M5+M6 W0: [`IMPLEMENTED`] is `false`, so the importer keeps its WARNING ("morph targets
//! (shape keys) are imported from M6").

/// Whether morph targets are imported (WP 6.8 sets it).
pub(crate) const IMPLEMENTED: bool = false;
