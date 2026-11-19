// SPDX-License-Identifier: GPL-3.0-or-later
//! Export filters (M3, WP 3.7): `use_selection`, `use_visible`, `use_renderable`, `collection`,
//! `use_active_collection[_with_nested]` and the `glTF_not_exported` collection.
//!
//! They are applied while the export tree is built, as the add-on's `VExportTree.filter`
//! does, so they live with the tree in `vtree.rs` (`Filters`: `node_filter_inheritable_is_kept`
//! and `node_filter_not_inheritable_is_kept`; a dropped node hands its kept children to the
//! nearest kept ancestor or to the roots). WP 3.7 completed them against Blender 5.2.4:
//! `tests/golden/cases/gltf/attributes/filters.py` covers visibility (hidden object, hidden
//! collection), renderability, a collection export with nested collections, the active
//! collection with and without nested collections, the special collection and a selection
//! combined with visibility; `gltf/m1/use_selection.py` covers the selection alone.
//!
//! [`SPECIAL_COLLECTION`] re-exports the special collection's name for callers.

pub use crate::export::vtree::SPECIAL_COLLECTION;
