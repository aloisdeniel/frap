// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the object operators (WP 1.21): object matrix bookkeeping with Blender's
//! "evaluated state" semantics (`xform`), mesh centers for `origin_set` (`center`), mesh
//! transforms for `transform_apply` (`mesh`), the loop of the transform clear operators
//! (`clear`), the shading attribute rules (`shade`), the 3D cursor and grid (`cursor`), object
//! snapping (`snap`) and the deform modifiers of `parent_set` (`deform`).

pub mod center;
pub mod clear;
pub mod cursor;
pub mod deform;
/// The glTF add-on's variant operators (M8 W0).
pub mod gltf_variants;
/// Bone heat weighting (M8 W0 stub, WP 8.5).
pub mod heat;
pub mod mesh;
pub mod shade;
pub mod snap;
/// Vertex-group locks, mirror maps and weight copying (M8 W0 stub, WP 8.3.7).
pub mod weights;
pub mod xform;
