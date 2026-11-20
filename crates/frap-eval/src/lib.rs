// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-eval` (layer L8): depsgraph-lite. Evaluated objects and meshes (shape keys +
//! modifiers), world matrices, poses, bounds; animation applied at evaluation boundaries.
//! Implements `frap_scene::Evaluator` (ARCHITECTURE §7.3).
//!
//! Module layout frozen by M1 W0; contents owned by WP 1.13. M6 (M5+M6 W0): `pose.rs` (WP
//! 6.2), `key_eval.rs` (WP 6.3) and `animate.rs` (WP 6.4) are wired into the evaluation.
#![deny(unsafe_code)]

pub mod animate;
mod cache;
/// The modifier stack (M4 W0; WP 4.1).
mod geometry;
/// Shape-key mixing before the modifiers (M5+M6 W0 stub; WP 6.3).
mod key_eval;
pub mod pose;

pub use cache::{
    EvalCache, bound_box, dimensions, matrix_basis, object_transform, parent_matrix,
    scale_for_dimensions, world_matrix,
};
