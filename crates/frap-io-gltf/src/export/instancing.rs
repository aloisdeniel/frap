// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_mesh_gpu_instancing` export (WP 7.5; M7 W0 stub): `export_gpu_instances` with the
//! add-on's rule, a post-pass over the gathered scenes
//! (`blender/exp/exporter.py::manage_gpu_instancing_nodes`: children of an Empty sharing a mesh
//! become one node with per-instance TRANSLATION/ROTATION/SCALE accessors; their nodes, skin
//! joints and animation channels are removed or renumbered). `export()` calls [`apply`] after
//! the animations and skins. Until WP 7.5 nothing changes (the operator warns about the prop).

use crate::export::ExportError;
use crate::export::gather::Gather;

/// Collapses instanced nodes (see the module docs). The stub changes nothing.
pub fn apply(_g: &mut Gather) -> Result<(), ExportError> {
    Ok(())
}
