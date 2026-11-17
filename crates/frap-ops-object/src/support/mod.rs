// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the object operators (WP 1.21): object matrix bookkeeping with Blender's
//! "evaluated state" semantics (`xform`), mesh centers for `origin_set` (`center`), mesh
//! transforms for `transform_apply` (`mesh`), the loop of the transform clear operators
//! (`clear`), the shading attribute rules (`shade`), the 3D cursor and grid (`cursor`) and
//! object snapping (`snap`).

pub mod center;
pub mod clear;
pub mod cursor;
pub mod mesh;
pub mod shade;
pub mod snap;
pub mod xform;
