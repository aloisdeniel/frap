// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Armature` (M6: bones, collections; edit bones live in `EditState`). Header-only placeholder created by M1 W0 so `Main` has its final shape (§5.2).

crate::id::header_only_id!(
    /// `bpy.types.Armature` (M6: bones, collections; edit bones live in `EditState`).
    Armature,
    frap_spec_rna::ARMATURE,
    ArmatureId
);

/// `Object.pose` (pose channels of an armature object; M6 W0 completes it with `bones`,
/// `PoseBone` channels and constraints). M1 W0 placeholder so `Object` has its final shape.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Pose {}
