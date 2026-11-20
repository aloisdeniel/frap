// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Armature` (rest bones and bone collections; edit bones live in
//! `EditState::armatures`, `crate::edit_armature`) and `Object.pose` (`Pose`, `PoseBone`).
//!
//! M5+M6 W0 contract (fields with Blender's RNA names and `armatures.new` /
//! `object.armature_add` defaults); WP 6.2 owns `id/armature*.rs`, including the RNA bindings
//! (`armature/rna.rs`; the hooks that need the whole armature, the edit state or the pose
//! object are `hooks/{armature,bone,edit_bone,pose,object_pose}_*.rs`). The bone maths shared
//! with the evaluator and the operators live in `crate::edit_armature`.

use frap_anim::AnimData;
use frap_rna::{IdHeader, ObjectId};
use smol_str::SmolStr;

/// `bpy.types.Armature`.
#[derive(Clone, Debug)]
pub struct Armature {
    /// ID header.
    pub header: IdHeader,
    /// `Armature.bones`, parents before children (Blender's `bonebase` depth-first order).
    pub bones: Vec<Bone>,
    /// `Armature.bones.active` (`act_bone`, index into `bones`; the edit-mode active bone is
    /// `EditArmature::active` while in Edit Mode, copied back on leaving it).
    pub active_bone: Option<usize>,
    /// `Armature.collections_all`
    pub collections: Vec<BoneCollection>,
    /// `Armature.display_type` (`OCTAHEDRAL`, `STICK`, `BBONE`, `ENVELOPE`, `WIRE`)
    pub display_type: SmolStr,
    /// `Armature.pose_position` (`POSE`, `REST`)
    pub pose_position: SmolStr,
    /// `Armature.show_names`
    pub show_names: bool,
    /// `Armature.show_axes`
    pub show_axes: bool,
    /// `Armature.animation_data`
    pub animation_data: Option<AnimData>,
    /// `Armature.collections.active` (index into `collections`; WP 6.2)
    pub active_collection: Option<usize>,
    /// `Armature.use_mirror_x` (`ARM_MIRROR_EDIT`)
    pub use_mirror_x: bool,
    /// `Armature.show_bone_custom_shapes`
    pub show_bone_custom_shapes: bool,
    /// `Armature.show_bone_colors`
    pub show_bone_colors: bool,
    /// `Armature.axes_position`
    pub axes_position: f32,
    /// `Armature.relation_line_position` (`TAIL`, `HEAD`)
    pub relation_line_position: SmolStr,
}

impl Armature {
    /// A new armature without bones (`bpy.data.armatures.new(name)`).
    pub fn new(name: impl Into<SmolStr>) -> Armature {
        Armature {
            header: IdHeader::new(name),
            bones: Vec::new(),
            active_bone: None,
            collections: Vec::new(),
            display_type: "OCTAHEDRAL".into(),
            pose_position: "POSE".into(),
            show_names: false,
            show_axes: false,
            animation_data: None,
            active_collection: None,
            use_mirror_x: false,
            show_bone_custom_shapes: true,
            show_bone_colors: true,
            axes_position: 0.0,
            relation_line_position: "TAIL".into(),
        }
    }

    /// The bone named `name`.
    pub fn bone(&self, name: &str) -> Option<&Bone> {
        self.bones.iter().find(|b| b.name == name)
    }

    /// Index of the bone named `name`.
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    /// Indices of the direct children of bone `i` (or of the root bones for `None`), in
    /// `childbase` order.
    pub fn children_of(&self, i: Option<usize>) -> Vec<usize> {
        (0..self.bones.len())
            .filter(|&j| self.bones[j].parent == i)
            .collect()
    }

    /// `ANIM_bone_in_visible_collection` (with Blender's solo rule; `BONE_HIDDEN_P` is
    /// `PoseBone.hide` in 5.2, so this is the collection visibility only).
    pub fn bone_collections_visible(&self, b: &Bone) -> bool {
        crate::edit_armature::collections_visible(self, &b.collections)
    }
}

/// `bpy.types.Bone` (rest data, armature space).
#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    /// `name`
    pub name: SmolStr,
    /// `parent` (index into `Armature::bones`)
    pub parent: Option<usize>,
    /// `head_local` (armature space)
    pub head_local: [f32; 3],
    /// `tail_local` (armature space)
    pub tail_local: [f32; 3],
    /// `matrix_local` (armature space, row-major like `Matrix` reads)
    pub matrix_local: [[f32; 4]; 4],
    /// The edit-bone roll the rest matrix was built with (`ED_armature_from_edit`'s input).
    pub roll: f32,
    /// `use_connect`
    pub use_connect: bool,
    /// `use_deform`
    pub use_deform: bool,
    /// `use_inherit_rotation`
    pub use_inherit_rotation: bool,
    /// `inherit_scale` (`FULL`, `FIX_SHEAR`, `ALIGNED`, `AVERAGE`, `NONE`, `NONE_LEGACY`)
    pub inherit_scale: SmolStr,
    /// `use_local_location`
    pub use_local_location: bool,
    /// `use_relative_parent`
    pub use_relative_parent: bool,
    /// `envelope_distance`
    pub envelope_distance: f32,
    /// `envelope_weight`
    pub envelope_weight: f32,
    /// `use_envelope_multiply`
    pub use_envelope_multiply: bool,
    /// `head_radius`
    pub head_radius: f32,
    /// `tail_radius`
    pub tail_radius: f32,
    /// `bbone_segments`
    pub bbone_segments: i32,
    /// `hide` (edit/object visibility; pose-mode hiding is `PoseBone::hide`)
    pub hide: bool,
    /// `hide_select`
    pub hide_select: bool,
    /// The edit-bone `select` / `select_head` / `select_tail` kept across Edit Mode toggles
    /// (`BONE_SELECTED`/`BONE_ROOTSEL`/`BONE_TIPSEL`; not RNA on `Bone` since Blender 5.0, pose
    /// selection is `PoseBone::select`).
    pub select: [bool; 3],
    /// Names of the bone collections the bone is assigned to.
    pub collections: Vec<SmolStr>,
}

impl Bone {
    /// A rest bone with `ED_armature_ebone_add`'s values, from the origin to (0, 0, 1),
    /// unparented.
    pub fn new(name: impl Into<SmolStr>) -> Bone {
        Bone {
            name: name.into(),
            parent: None,
            head_local: [0.0; 3],
            tail_local: [0.0, 0.0, 1.0],
            matrix_local: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            roll: 0.0,
            use_connect: false,
            use_deform: true,
            use_inherit_rotation: true,
            inherit_scale: "FULL".into(),
            use_local_location: true,
            use_relative_parent: false,
            envelope_distance: 0.25,
            envelope_weight: 1.0,
            use_envelope_multiply: false,
            head_radius: 0.1,
            tail_radius: 0.05,
            bbone_segments: 1,
            hide: false,
            hide_select: false,
            select: [false; 3],
            collections: Vec::new(),
        }
    }

    /// The flags the bone parent transform reads.
    pub fn parent_flags(&self) -> crate::edit_armature::math::ParentFlags {
        crate::edit_armature::math::ParentFlags {
            inherit_rotation: self.use_inherit_rotation,
            local_location: self.use_local_location,
            inherit_scale: crate::edit_armature::math::InheritScale::from_identifier(
                &self.inherit_scale,
            ),
        }
    }

    /// `matrix_local` as a glam matrix (stored as RNA rows).
    pub fn arm_mat(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array_2d(&self.matrix_local).transpose()
    }
}

/// `bpy.types.BoneCollection`.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneCollection {
    /// `name`
    pub name: SmolStr,
    /// `parent` (index into `Armature::collections`)
    pub parent: Option<usize>,
    /// `is_visible`
    pub is_visible: bool,
    /// `is_solo`
    pub is_solo: bool,
}

/// `Object.pose`: the pose channels of an armature object.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Pose {
    /// `Pose.bones`, in the armature's bone order.
    pub bones: Vec<PoseBone>,
    /// `Pose.use_mirror_x` (`POSE_MIRROR_EDIT`)
    pub use_mirror_x: bool,
    /// `Pose.use_mirror_relative` (`POSE_MIRROR_RELATIVE`)
    pub use_mirror_relative: bool,
    /// `Pose.use_auto_ik` (`POSE_AUTO_IK`)
    pub use_auto_ik: bool,
}

/// `bpy.types.PoseBone` (local transform relative to the rest pose).
#[derive(Clone, Debug, PartialEq)]
pub struct PoseBone {
    /// `name` (the bone's)
    pub name: SmolStr,
    /// `location`
    pub location: [f32; 3],
    /// `rotation_mode` (`QUATERNION` for new bones)
    pub rotation_mode: SmolStr,
    /// `rotation_quaternion` (w, x, y, z)
    pub rotation_quaternion: [f32; 4],
    /// `rotation_euler`
    pub rotation_euler: [f32; 3],
    /// `rotation_axis_angle` (angle, x, y, z)
    pub rotation_axis_angle: [f32; 4],
    /// `scale`
    pub scale: [f32; 3],
    /// `lock_location`
    pub lock_location: [bool; 3],
    /// `lock_rotation`
    pub lock_rotation: [bool; 3],
    /// `lock_rotation_w`
    pub lock_rotation_w: bool,
    /// `lock_scale`
    pub lock_scale: [bool; 3],
    /// `custom_shape`
    pub custom_shape: Option<frap_rna::IdRef<ObjectId>>,
    /// `select` (pose-mode selection, stored on the pose bone since Blender 5.0)
    pub select: bool,
    /// `hide` (pose-mode hiding)
    pub hide: bool,
    /// `POSE_SELECTED_ROOT`: the head selection carried to Edit Mode (not RNA).
    pub select_head: bool,
    /// `POSE_SELECTED_TIP`: the tail selection carried to Edit Mode (not RNA).
    pub select_tail: bool,
    /// `lock_rotations_4d` (`OB_LOCK_ROT4D`, on for new channels)
    pub lock_rotations_4d: bool,
}

impl PoseBone {
    /// The channel `BKE_pose_channel_ensure` creates for a bone.
    pub fn new(name: impl Into<SmolStr>) -> PoseBone {
        PoseBone {
            name: name.into(),
            location: [0.0; 3],
            rotation_mode: "QUATERNION".into(),
            rotation_quaternion: [1.0, 0.0, 0.0, 0.0],
            rotation_euler: [0.0; 3],
            rotation_axis_angle: [0.0, 0.0, 1.0, 0.0],
            scale: [1.0; 3],
            lock_location: [false; 3],
            lock_rotation: [false; 3],
            lock_rotation_w: false,
            lock_scale: [false; 3],
            custom_shape: None,
            select: false,
            hide: false,
            select_head: false,
            select_tail: false,
            lock_rotations_4d: true,
        }
    }

    /// The local transform channels.
    pub fn channels(&self) -> crate::edit_armature::math::Channels {
        let a = self.rotation_axis_angle;
        crate::edit_armature::math::Channels {
            location: glam::Vec3::from_array(self.location),
            mode: frap_math::matrix::RotationMode::from_blender(&self.rotation_mode)
                .unwrap_or(frap_math::matrix::RotationMode::Quaternion),
            quat: self.rotation_quaternion,
            euler: self.rotation_euler,
            axis: [a[1], a[2], a[3]],
            angle: a[0],
            scale: glam::Vec3::from_array(self.scale),
        }
    }

    /// `rna_PoseChannel_rotation_mode_set`: converts the stored rotation
    /// (`BKE_rotMode_change_values`), then switches mode (an unknown mode changes nothing).
    pub fn set_rotation_mode(&mut self, new: &str) {
        use frap_math::matrix::{RotationMode, RotationValues, rot_mode_change_values};
        let old =
            RotationMode::from_blender(&self.rotation_mode).unwrap_or(RotationMode::Quaternion);
        let Some(newm) = RotationMode::from_blender(new) else {
            return;
        };
        let mut v = RotationValues {
            quaternion: self.rotation_quaternion,
            euler: self.rotation_euler,
            axis_angle: self.rotation_axis_angle,
        };
        rot_mode_change_values(&mut v, old, newm);
        self.rotation_quaternion = v.quaternion;
        self.rotation_euler = v.euler;
        self.rotation_axis_angle = v.axis_angle;
        self.rotation_mode = new.into();
    }

    /// Writes back the transform channels (the rotation mode is not changed).
    pub fn set_channels(&mut self, c: &crate::edit_armature::math::Channels) {
        self.location = c.location.to_array();
        self.rotation_quaternion = c.quat;
        self.rotation_euler = c.euler;
        self.rotation_axis_angle = [c.angle, c.axis[0], c.axis[1], c.axis[2]];
        self.scale = c.scale.to_array();
    }

    /// `BKE_pose_channel_copy_data` (what a duplicated bone's channel takes from its source):
    /// transform locks, rotation mode, custom shape.
    pub fn copy_data_from(&mut self, from: &PoseBone) {
        self.lock_location = from.lock_location;
        self.lock_rotation = from.lock_rotation;
        self.lock_rotation_w = from.lock_rotation_w;
        self.lock_rotations_4d = from.lock_rotations_4d;
        self.lock_scale = from.lock_scale;
        self.rotation_mode = from.rotation_mode.clone();
        self.custom_shape = from.custom_shape;
    }
}

impl Pose {
    /// The channel named `name`.
    pub fn bone(&self, name: &str) -> Option<&PoseBone> {
        self.bones.iter().find(|p| p.name == name)
    }

    /// The channel named `name`, mutably.
    pub fn bone_mut(&mut self, name: &str) -> Option<&mut PoseBone> {
        self.bones.iter_mut().find(|p| p.name == name)
    }

    /// `BKE_pose_channel_ensure`: the channel named `name`, created (appended) when missing.
    pub fn ensure(&mut self, name: &str) -> &mut PoseBone {
        let i = match self.bones.iter().position(|p| p.name == name) {
            Some(i) => i,
            None => {
                self.bones.push(PoseBone::new(name));
                self.bones.len() - 1
            }
        };
        &mut self.bones[i]
    }

    /// `BKE_pose_rebuild`: one channel per bone, in bone order (existing channels keep their
    /// values, new ones get `BKE_pose_channel_ensure`'s), channels without a bone removed.
    pub fn rebuild(pose: &mut Option<Pose>, arm: &Armature) {
        let p = pose.get_or_insert_with(Pose::default);
        let mut old = std::mem::take(&mut p.bones);
        for b in &arm.bones {
            let ch = match old.iter().position(|c| c.name == b.name) {
                Some(i) => old.swap_remove(i),
                None => PoseBone::new(b.name.clone()),
            };
            p.bones.push(ch);
        }
    }

    /// `flush_bone_selection_to_pose`: the rest bones' selection flags onto the channels.
    pub fn flush_selection_from(&mut self, arm: &Armature) {
        for pb in &mut self.bones {
            if let Some(b) = arm.bone(&pb.name) {
                pb.select = b.select[0];
                pb.select_head = b.select[1];
                pb.select_tail = b.select[2];
            }
        }
    }
}

/// RNA bindings of `Armature`, `Bone`, `BoneCollection`, `EditBone`, `Pose` and `PoseBone`.
mod rna;
