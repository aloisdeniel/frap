// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF skins (WP 6.8): armatures as joint hierarchies (`export_def_bones` deform-bone filter,
//! `export_hierarchy_flatten_bones`, `export_armature_object_remove`), inverse bind matrices,
//! the skinned mesh's `JOINTS_n`/`WEIGHTS_n` (`export/mesh/writers/skin.rs`, the vertex-group ↔
//! bone mapping and `export_influence_nb`), ported from io_scene_gltf2 5.2.41
//! `blender/exp/skins.py`. Joint nodes are recorded in `export::node_map::NodeMap::bones`.
//!
//! M5+M6 W0 stub: nothing is skinned until WP 6.8 lands (WP 6.8 also owns `export/nodes.rs` and
//! `export/vtree.rs`, reassigned, to gather bones as nodes).
