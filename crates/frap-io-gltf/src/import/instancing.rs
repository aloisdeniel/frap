// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_mesh_gpu_instancing` import (WP 7.5; M7 W0 stub): one object per instance, as the
//! add-on's `blender/imp/vnode.py::manage_gpu_instancing` creates virtual child nodes. WP 7.5
//! owns `vnode.rs`, `nodes.rs` and `mesh.rs` in M7 (reassigned) and wires it there; until
//! [`IMPLEMENTED`] the importer warns that the extension was ignored.

/// Whether instancing is imported (gates the M1 WARNING of `Gltf::warn_unsupported`).
pub(crate) const IMPLEMENTED: bool = false;
