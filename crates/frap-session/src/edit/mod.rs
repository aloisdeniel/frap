// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit Mode in the session (ARCHITECTURE §6.3, §13.2): the cached read-only view of an edit
//! BMesh, the `RNA_EDITMODE_WRITE` guard on mesh geometry writes, and the `edit_mesh` block of
//! responses.
//!
//! **Functional stubs** created by M2 W0 (WP 2.0) and already wired into the session (reads,
//! writes, RNA function calls, responses), keeping the M1 behaviour: the view is the original
//! mesh, nothing is refused, no `edit_mesh` block. WP 2.2 (Edit-mode integration, §22.4) owns
//! `src/edit/**` and implements them: reads through `EditMesh.view` keyed by `bm_gen` (one
//! `BMesh::to_mesh` per BMesh generation, no `gen` bump, `is_dirty` and eval fingerprints
//! unchanged), `RuntimeError` code `RNA_EDITMODE_WRITE` with a `mode_set(mode='OBJECT')` hint,
//! and the `edit_mesh` block (`select_mode`, selected/total counts per edit object).

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{FrapError, FuncDef, IdRef, MeshId, PropDef, RnaPtr};
use frap_scene::{EditState, HostRead, Main};
use serde_json::Value as J;

/// `Host::mesh_view` (§6.3): the edit-mode view of `mesh` while it is in Edit Mode, the
/// original otherwise.
pub(crate) fn mesh_view(main: &Main, edit: &EditState, mesh: IdRef<MeshId>) -> Arc<Mesh> {
    let _ = edit;
    main.meshes
        .get_arc(mesh)
        .expect("invariant: mesh_view of an existing mesh")
}

/// Refuses a write to mesh geometry (`vertices`, `edges`, `polygons`, `loops`, `attributes`,
/// `uv_layers[…].data`, …) of a mesh in Edit Mode (`RNA_EDITMODE_WRITE`, §6.3). `ptr` is the
/// struct written to, `prop` the property (or collection) written.
pub(crate) fn check_write(
    host: &dyn HostRead,
    ptr: &RnaPtr,
    prop: &'static PropDef,
) -> Result<(), FrapError> {
    let _ = (host, ptr, prop);
    Ok(())
}

/// Refuses an RNA function that rewrites mesh geometry (`from_pydata`, `clear_geometry`,
/// `transform`, …) on a mesh in Edit Mode (`RNA_EDITMODE_WRITE`).
pub(crate) fn check_func(
    host: &dyn HostRead,
    this: &RnaPtr,
    func: &'static FuncDef,
) -> Result<(), FrapError> {
    let _ = (host, this, func);
    Ok(())
}

/// The `edit_mesh` block of a response (§13.2), `None` outside Edit Mode.
pub(crate) fn edit_mesh_block(host: &dyn HostRead) -> Option<J> {
    let _ = host;
    None
}
