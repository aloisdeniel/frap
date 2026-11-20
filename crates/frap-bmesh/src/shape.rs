// SPDX-License-Identifier: GPL-3.0-or-later
//! Shape keys in Edit Mode (WP 6.3): the `CD_SHAPEKEY` layers of a BMesh
//! ([`crate::types::ShapeLayer`]) built from the mesh's `Key` on entering Edit Mode, and the
//! write-back on leaving it (`BM_mesh_bm_to_me`'s shape-key branch: the active block takes the
//! edited positions, and when the basis changed every block relative to it is offset by the
//! basis delta — "edit-mode basis propagation"). `convert.rs` calls into this module once WP
//! 6.3 lands; until then meshes with shape keys cannot exist (M5+M6 W0 stub).
