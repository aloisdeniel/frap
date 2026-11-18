// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_apply=True` (M4, WP 4.7; stub created by M4 W0): the exported mesh of an object is
//! its viewport-evaluated mesh (`ExportSettings::evaluated_meshes`, filled by the operator), and
//! the add-on's sharing rules decide which objects still share one glTF mesh (an object with
//! modifiers gets its own; linked duplicates without modifiers keep sharing). WP 4.7 owns this
//! file and `mesh/gather.rs`, which asks [`evaluated`].

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdRef, ObjectId};

use crate::export::gather::Gather;

/// The evaluated mesh to export for `ob` instead of its data (`None`: export the original).
/// M1 behaviour until WP 4.7: always the original.
pub fn evaluated(g: &Gather, ob: IdRef<ObjectId>) -> Option<Arc<Mesh>> {
    let _ = (g, ob);
    None
}
