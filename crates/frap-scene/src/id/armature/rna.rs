// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of the armature types (WP 6.2), per Blender 5.2.4's `rna_armature.cc` and
//! `rna_pose.cc` (dumped in `spec/blender-5.2/rna/`).
//!
//! - `Armature` and its `Bone`s and `BoneCollection`s live in `bpy.data`: plain fields bind
//!   directly; values Blender derives from the whole armature (`Bone.head`, `matrix`, `length`,
//!   `parent`, `children`) and edit-state reads (`edit_bones`, `is_editmode`) are hooks.
//! - `EditBone`s live in `EditState`, which mutable RNA walks from `bpy.data` cannot reach:
//!   every writable edit-bone property is written by a set hook
//!   (`hooks/edit_bone_*.rs`), which also runs Blender's update callbacks (connected bones
//!   follow, hidden bones are deselected, X-mirror).
//! - `Object.pose` is a hook (`hooks/object_pose.rs`), so `Pose` and `PoseBone` writes are set
//!   hooks too (`hooks/pose_*.rs`); pose matrices are computed fresh on read (D-04).
//!
//! Properties frap does not model (bendy-bone shape, bone colours, display types, IK limits,
//! custom-shape transforms) stay unbound (`RNA_NOT_IMPLEMENTED`).

use frap_rna::{IdAny, IdHeader};

use super::{Armature, Bone, BoneCollection, Pose, PoseBone};
use crate::edit_armature::EditBone;

crate::id::id_rna! { Armature as frap_spec_rna::ARMATURE, "ARMATURE" {
    bones => coll(bones, item = frap_spec_rna::BONE, api = "ArmatureBones"),
    edit_bones => hook_coll,
    collections => hook_coll,
    collections_all => coll(collections, item = frap_spec_rna::BONE_COLLECTION),
    pose_position => pose_position,
    display_type => display_type,
    show_axes => show_axes,
    axes_position => axes_position,
    relation_line_position => relation_line_position,
    show_names => show_names,
    use_mirror_x => use_mirror_x,
    show_bone_custom_shapes => show_bone_custom_shapes,
    show_bone_colors => show_bone_colors,
    is_editmode => hook,
}}

/// `rna_Bone_hide_update`: a hidden or unselectable bone loses its selection.
fn hide_update(b: &mut Bone) {
    if b.hide || b.hide_select {
        b.select = [false; 3];
    }
}

frap_rna::rna_struct! { Bone as frap_spec_rna::BONE {
    parent => hook,
    children => hook_coll,
    collections => hook_coll,
    name => name (set = hook),
    use_connect => readonly(|b| b.use_connect),
    use_inherit_rotation => use_inherit_rotation,
    use_envelope_multiply => use_envelope_multiply,
    use_deform => use_deform,
    inherit_scale => inherit_scale,
    use_local_location => use_local_location,
    use_relative_parent => use_relative_parent,
    hide_select => get(|b| b.hide_select) set(|b, v: bool| { b.hide_select = v; hide_update(b) }),
    envelope_distance => envelope_distance,
    envelope_weight => envelope_weight,
    head_radius => head_radius,
    tail_radius => tail_radius,
    bbone_segments => bbone_segments,
    hide => get(|b| b.hide) set(|b, v: bool| { b.hide = v; hide_update(b) }),
    matrix => hook,
    matrix_local => readonly(|b| b.arm_mat()),
    tail => hook,
    tail_local => readonly(|b| b.tail_local),
    head => hook,
    head_local => readonly(|b| b.head_local),
    length => hook,
}}

frap_rna::rna_struct! { BoneCollection as frap_spec_rna::BONE_COLLECTION {
    name => name (set = hook),
    is_visible => is_visible (set = hook),
    is_solo => is_solo (set = hook),
    is_local_override => readonly(|_c| false),
    is_editable => readonly(|_c| true),
    bones => hook_coll,
    children => hook_coll,
    parent => hook,
    index => hook,
    is_visible_ancestors => hook,
    is_visible_effectively => hook,
}}

frap_rna::rna_struct! { EditBone as frap_spec_rna::EDIT_BONE {
    collections => hook_coll,
    parent => hook,
    roll => roll (set = hook),
    head => head (set = hook),
    tail => tail (set = hook),
    length => hook,
    name => name (set = hook),
    use_connect => use_connect (set = hook),
    use_inherit_rotation => use_inherit_rotation (set = hook),
    use_envelope_multiply => use_envelope_multiply (set = hook),
    use_deform => use_deform (set = hook),
    inherit_scale => inherit_scale (set = hook),
    use_local_location => use_local_location (set = hook),
    use_relative_parent => use_relative_parent (set = hook),
    hide_select => hide_select (set = hook),
    envelope_distance => envelope_distance (set = hook),
    envelope_weight => envelope_weight (set = hook),
    head_radius => head_radius (set = hook),
    tail_radius => tail_radius (set = hook),
    bbone_segments => bbone_segments (set = hook),
    hide => hide (set = hook),
    lock => lock (set = hook),
    select => hook,
    select_head => hook,
    select_tail => hook,
    matrix => hook,
}}

frap_rna::rna_struct! { Pose as frap_spec_rna::POSE {
    bones => coll(bones, item = frap_spec_rna::POSE_BONE),
    use_mirror_x => use_mirror_x (set = hook),
    use_mirror_relative => use_mirror_relative (set = hook),
    use_auto_ik => use_auto_ik (set = hook),
}}

frap_rna::rna_struct! { PoseBone as frap_spec_rna::POSE_BONE {
    name => name (set = hook),
    bone => hook,
    parent => hook,
    child => hook,
    location => location (set = hook),
    scale => scale (set = hook),
    rotation_quaternion => rotation_quaternion (set = hook),
    rotation_axis_angle => rotation_axis_angle (set = hook),
    rotation_euler => rotation_euler (tag = Euler(rotation_mode), set = hook),
    rotation_mode => rotation_mode (set = hook),
    matrix_channel => hook,
    matrix_basis => hook,
    matrix => hook,
    head => hook,
    tail => hook,
    length => hook,
    custom_shape => hook,
    hide => hide (set = hook),
    select => select (set = hook),
    lock_location => lock_location (set = hook),
    lock_rotation => lock_rotation (set = hook),
    lock_rotation_w => lock_rotation_w (set = hook),
    lock_rotations_4d => lock_rotations_4d (set = hook),
    lock_scale => lock_scale (set = hook),
}}

impl frap_rna::IdType for Armature {
    type Kind = frap_rna::ArmatureId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.into());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_match_rna() {
        frap_rna::testing::check_bindings::<Armature>();
        frap_rna::testing::check_bindings::<Bone>();
        frap_rna::testing::check_bindings::<BoneCollection>();
        frap_rna::testing::check_bindings::<EditBone>();
        frap_rna::testing::check_bindings::<Pose>();
        frap_rna::testing::check_bindings::<PoseBone>();
    }

    #[test]
    fn new_pose_bone_defaults() {
        let p = PoseBone::new("Bone");
        assert_eq!(p.rotation_mode, "QUATERNION");
        assert_eq!(p.rotation_axis_angle, [0.0, 0.0, 1.0, 0.0]);
        assert!(p.lock_rotations_4d);
    }
}
