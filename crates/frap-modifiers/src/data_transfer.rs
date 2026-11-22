// SPDX-License-Identifier: GPL-3.0-or-later
//! The data-transfer core, to be ported from `blender/source/blender/blenkernel/intern/
//! data_transfer.cc` and `mesh_remap.cc` (Blender 5.2.4): element mapping between a source
//! and a destination mesh (`BKE_mesh_remap_calc_{verts,edges,loops,polys}_from_mesh`, every
//! `vert_mapping` / `edge_mapping` / `loop_mapping` / `poly_mapping` mode, `mix_mode`,
//! `mix_factor`, islands precision) and the layer transfer of `BKE_object_data_transfer_ex`
//! (vertex groups, UV maps, colour attributes, custom normals, seams/sharp/crease/bevel
//! weights), shared by the DATA_TRANSFER modifier (`types/datatransfer.rs`) and the
//! `object.data_transfer` / `object.datalayout_transfer` operators.
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.3.13 owns this file (and `data_transfer/**`).
