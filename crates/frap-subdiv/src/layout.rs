// SPDX-License-Identifier: GPL-3.0-or-later
//! Output ordering pass (WP 4.4): reorders a [`crate::SubdivResult`] into the vertex, edge,
//! face and corner order of Blender's `subdiv_mesh.cc` / `subdiv_foreach.cc` (vertices of base
//! corners, then edge vertices, then inner vertices per ptex face; …), moving SUBSURF towards
//! T1. Stub until WP 4.4.
