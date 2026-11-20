// SPDX-License-Identifier: GPL-3.0-or-later
//! Shape-key evaluation (WP 6.3): `BKE_key_evaluate_object` for relative keys — the basis plus
//! every unmuted block's `value × (block − its relative_key)`, weighted by the block's vertex
//! group, `show_only_shape_key` / `active_shape_key_index` honoured; absolute keys by
//! `eval_time`. Runs before the modifier stack (`cache/mesh.rs`).
//!
//! M5+M6 W0: [`apply`] is called for every evaluated mesh object; without a key (or without
//! blocks) it returns the base mesh unchanged, which is all it does until WP 6.3 lands.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_scene::HostRead;
use frap_scene::id::Object;

/// The base mesh of `ob` with its shape keys mixed in.
pub(crate) fn apply(host: &dyn HostRead, ob: &Object, base: Arc<Mesh>) -> Arc<Mesh> {
    let Some(key) = base.shape_keys.and_then(|k| host.main().shape_keys.get(k)) else {
        return base;
    };
    if key.key_blocks.is_empty() {
        return base;
    }
    let _ = ob;
    todo!("M6 WP 6.3: shape-key mixing")
}
