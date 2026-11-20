// SPDX-License-Identifier: GPL-3.0-or-later
//! Armatures in Edit Mode: edit bones (`Armature.edit_bones`, `EditBone`), created from the
//! rest bones on entering Edit Mode and written back (`ED_armature_from_edit`: rest matrices
//! from head, tail and roll) on leaving it. M5+M6 W0 contract; WP 6.2 owns the file.

use smol_str::SmolStr;

/// Armature in Edit Mode (`EditState::armatures`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditArmature {
    /// `Armature.edit_bones`, in creation order.
    pub edit_bones: Vec<EditBone>,
    /// `edit_bones.active` (index)
    pub active: Option<usize>,
}

/// `bpy.types.EditBone`.
#[derive(Clone, Debug, PartialEq)]
pub struct EditBone {
    /// `name`
    pub name: SmolStr,
    /// `parent` (index into `EditArmature::edit_bones`)
    pub parent: Option<usize>,
    /// `head` (armature space)
    pub head: [f32; 3],
    /// `tail` (armature space)
    pub tail: [f32; 3],
    /// `roll`
    pub roll: f32,
    /// `use_connect`
    pub use_connect: bool,
    /// `use_deform`
    pub use_deform: bool,
    /// `use_inherit_rotation`
    pub use_inherit_rotation: bool,
    /// `inherit_scale`
    pub inherit_scale: SmolStr,
    /// `use_local_location`
    pub use_local_location: bool,
    /// `envelope_distance`
    pub envelope_distance: f32,
    /// `envelope_weight`
    pub envelope_weight: f32,
    /// `head_radius`
    pub head_radius: f32,
    /// `tail_radius`
    pub tail_radius: f32,
    /// `select` / `select_head` / `select_tail`
    pub select: [bool; 3],
    /// `hide`
    pub hide: bool,
    /// `hide_select`
    pub hide_select: bool,
    /// `lock`
    pub lock: bool,
    /// Names of the bone collections the bone is assigned to (`EditBone.collections`).
    pub collections: Vec<SmolStr>,
}
