// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_variants` export (WP 7.5; M7 W0 stub), the add-on's
//! `blender/exp/primitives.py::__gather_extensions` and
//! `blender/exp/material/extensions/variants.py`: while the add-on preference
//! `KHR_materials_variants_ui` is on and `bpy.data.scenes[0]` has variants, each primitive gets
//! the `mappings` of its material slot (`Mesh.gltf2_variant_mesh_data`) and the root the
//! variants it uses. `mesh/gather.rs` calls [`primitive`] for every primitive it builds;
//! `export()` calls [`finish`] after the scenes. The add-on's `variants_reset_to_original`
//! (default materials restored on the slots before gathering) is the operator's
//! (`frap-ops-io` `export_scene__gltf.rs`, reassigned to WP 7.5).

use frap_gltf::json::Primitive;
use frap_mesh::Mesh;

use crate::export::ExportError;
use crate::export::gather::Gather;

/// Adds the `KHR_materials_variants` mappings of `mesh`'s material slot `slot` to `prim`.
/// The stub adds nothing.
pub fn primitive(
    _g: &mut Gather,
    _mesh: &Mesh,
    _slot: usize,
    _prim: &mut Primitive,
) -> Result<(), ExportError> {
    Ok(())
}

/// Writes the root variant list (after every primitive). The stub writes nothing.
pub fn finish(_g: &mut Gather) -> Result<(), ExportError> {
    Ok(())
}
