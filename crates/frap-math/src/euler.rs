// SPDX-License-Identifier: GPL-3.0-or-later
//! Euler rotations with Blender's conventions (`BLI_math_rotation.h`). WP 1.2 fills in
//! `eul_to_mat3`, `mat3_to_compatible_eul` and friends.

use serde::{Deserialize, Serialize};

/// `Object.rotation_mode` Euler orders (Blender identifiers `'XYZ'`, `'XZY'`, …).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EulerOrder {
    /// `'XYZ'` (Blender default)
    #[default]
    XYZ,
    /// `'XZY'`
    XZY,
    /// `'YXZ'`
    YXZ,
    /// `'YZX'`
    YZX,
    /// `'ZXY'`
    ZXY,
    /// `'ZYX'`
    ZYX,
}

impl EulerOrder {
    /// Blender's identifier.
    pub const fn as_str(self) -> &'static str {
        match self {
            EulerOrder::XYZ => "XYZ",
            EulerOrder::XZY => "XZY",
            EulerOrder::YXZ => "YXZ",
            EulerOrder::YZX => "YZX",
            EulerOrder::ZXY => "ZXY",
            EulerOrder::ZYX => "ZYX",
        }
    }
    /// Parses Blender's identifier.
    pub fn from_blender(s: &str) -> Option<Self> {
        Some(match s {
            "XYZ" => EulerOrder::XYZ,
            "XZY" => EulerOrder::XZY,
            "YXZ" => EulerOrder::YXZ,
            "YZX" => EulerOrder::YZX,
            "ZXY" => EulerOrder::ZXY,
            "ZYX" => EulerOrder::ZYX,
            _ => return None,
        })
    }
}

/// An Euler rotation (radians) with its order (`mathutils.Euler`).
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Euler {
    /// Angles in radians, X Y Z.
    pub angles: [f32; 3],
    /// Rotation order.
    pub order: EulerOrder,
}
