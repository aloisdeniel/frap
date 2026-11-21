// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_variants` import (WP 7.5; M7 W0 stub): the add-on's
//! `BlenderGlTF.manage_material_variants` (the scene's variant list, the preference
//! `KHR_materials_variants_ui` turned on) and `blender/imp/mesh.py`'s per-slot mappings
//! (`Mesh.gltf2_variant_mesh_data`, `gltf2_variant_default_materials`; WP 7.5 owns `mesh.rs`
//! in M7). Until [`IMPLEMENTED`] the importer warns that the extension was ignored.

use frap_scene::{ChangeLog, Main};

use super::{Gltf, ImportError};

/// Whether variants are imported (gates the M1 WARNING of `Gltf::warn_unsupported`).
pub(crate) const IMPLEMENTED: bool = false;

/// Creates the scene's variant list; returns whether the file has variants (the operator then
/// turns `KHR_materials_variants_ui` on). The stub creates nothing and answers `false`.
pub(super) fn create(
    _cx: &mut Gltf,
    _main: &mut Main,
    _log: &mut ChangeLog,
) -> Result<bool, ImportError> {
    Ok(false)
}
