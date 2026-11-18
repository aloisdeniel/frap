// SPDX-License-Identifier: GPL-3.0-or-later
//! Adding a primitive while a mesh is in Edit Mode (`make_prim_init`/`make_prim_finish` in
//! `editors/mesh/editmesh_add.cc`): Blender builds the new geometry **into the edit BMesh** of
//! the edit object (transformed into its local space), deselects everything else and selects
//! only the new elements; no object is added.
//!
//! **Functional stub** created by M2 W0 (WP 2.0): it keeps the M1 answer
//! (`OP_NOT_IMPLEMENTED`). WP 2.2 (Edit-mode integration, ARCHITECTURE §22.4) implements it;
//! `helpers::add_primitive_or_merge` already routes the Edit Mode case here.

use frap_mesh::Mesh;
use frap_rna::{IdRef, ObjectId};

use crate::ctx::{OpCtx, Placement};
use crate::def::OpError;

/// Merges `mesh` (built with the creation matrix of `at`) into the edit BMesh of `edit_ob`,
/// selecting only the new geometry; returns the edit object.
pub fn merge_into_edit_mesh(
    cx: &mut OpCtx,
    edit_ob: IdRef<ObjectId>,
    mesh: Mesh,
    at: Placement,
) -> Result<IdRef<ObjectId>, OpError> {
    let _ = (cx, edit_ob, mesh, at);
    Err(OpError::NotImplemented(
        "frap: adding a primitive to a mesh in Edit Mode (merged into the edit mesh) arrives in M2"
            .into(),
    ))
}
