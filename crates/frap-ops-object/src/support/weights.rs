// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared parts of the M8 vertex-group and shape-key operators (`object.vertex_group_*`,
//! `vertex_weight_*`, `shape_key_*`, `update_shapes`; `editors/object/object_vgroup.cc`,
//! `object_shapekey.cc`): the vertex-group locks (`vgroup_validmap_get`, the subsets of
//! `group_select_mode` / `WT_VGROUP_*`), the mirror maps of `ED_vgroup_mirror` and the active
//! vertex of weight copying.
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.3.7 owns this file (and `weights/**`).
