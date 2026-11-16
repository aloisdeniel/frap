// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-math` (layer L0): Blender's math conventions on top of `glam` (ARCHITECTURE §3.2).
//!
//! Euler matrices use Blender's `eul_to_mat3` (XYZ = Rz·Ry·Rx), never `glam::EulerRot`
//! [RUST §2]; quaternions are w-first at every API boundary; transcendentals come from `libm`
//! (rule 2.2.12; glam is built with its `libm` feature for the same reason).
//!
//! Module layout frozen by M1 W0 (WP 1.0); contents owned by WP 1.2.
#![deny(unsafe_code)]

pub mod euler;
pub mod fmt;
pub mod matrix;
pub mod quat;
pub mod rng;

pub use euler::{Euler, EulerOrder};
pub use matrix::decompose;
pub use rng::BliRng;

/// Re-export so dependants name one glam version.
pub use glam;
