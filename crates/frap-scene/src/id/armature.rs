// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Armature` (rest bones and bone collections; edit bones live in
//! `EditState::armatures`, `crate::edit_armature`) and `Object.pose` (`Pose`, `PoseBone`).
//!
//! M5+M6 W0 contract (fields with Blender's RNA names and `armatures.new` /
//! `object.armature_add` defaults); WP 6.2 owns `id/armature*.rs`, including the RNA bindings
//! (only `ID.name` is bound until it lands).

use frap_anim::AnimData;
use frap_rna::{IdAny, IdHeader, ObjectId};
use smol_str::SmolStr;

/// `bpy.types.Armature`.
#[derive(Clone, Debug)]
pub struct Armature {
    /// ID header.
    pub header: IdHeader,
    /// `Armature.bones`, parents before children (Blender's `bonebase` depth-first order).
    pub bones: Vec<Bone>,
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
}

impl Armature {
    /// A new armature without bones (`bpy.data.armatures.new(name)`).
    pub fn new(name: impl Into<SmolStr>) -> Armature {
        Armature {
            header: IdHeader::new(name),
            bones: Vec::new(),
            collections: Vec::new(),
            display_type: "OCTAHEDRAL".into(),
            pose_position: "POSE".into(),
            show_names: false,
            show_axes: false,
            animation_data: None,
        }
    }

    /// The bone named `name`.
    pub fn bone(&self, name: &str) -> Option<&Bone> {
        self.bones.iter().find(|b| b.name == name)
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
    /// `hide`
    pub hide: bool,
    /// `select` / `select_head` / `select_tail`
    pub select: [bool; 3],
    /// Names of the bone collections the bone is assigned to.
    pub collections: Vec<SmolStr>,
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
        }
    }
}

crate::id::name_only_rna!(Armature, frap_spec_rna::ARMATURE, ArmatureId);

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
