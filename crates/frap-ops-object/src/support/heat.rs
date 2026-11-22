// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone heat weighting for `object.parent_set(type='ARMATURE_AUTO')`, to be ported from
//! `blender/source/blender/editors/armature/meshlaplacian.cc` (`heat_bone_weighting`: the
//! mesh Laplacian with cotangent weights, the per-bone heat source over the vertices visible
//! from the bone (`heat_ray_source_visible`, through `frap_geom::bvh`), the sparse solve per
//! bone and the `Bone Heat Weighting: failed to find solution for one or more bones`
//! WARNING) and `armature_skinning.cc` (`add_verts_to_dgroups` with `heat = true`, X-mirror).
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.5 owns this file (and `heat/**`), with
//! `ops/parent_set.rs` and `support/deform.rs` reassigned; the sparse solver is `faer`'s.
