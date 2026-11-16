// SPDX-License-Identifier: GPL-3.0-or-later
//! Matrix helpers with Blender's conventions (`mat4_to_loc_rot_size`, shear loss on
//! decomposition). WP 1.2.

use glam::{Mat4, Quat, Vec3};

/// `mat4_to_loc_rot_size`: location, rotation (w-first quaternion semantics in `glam::Quat`)
/// and scale, as Blender decomposes `matrix_world` writes (shear is lost, like Blender).
pub fn decompose(m: &Mat4) -> (Vec3, Quat, Vec3) {
    let _ = m;
    todo!("WP 1.2: port mat4_to_loc_rot_size")
}
