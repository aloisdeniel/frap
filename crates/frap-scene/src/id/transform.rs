// SPDX-License-Identifier: GPL-3.0-or-later
//! Object transform channels and matrices (`matrix_basis`, `matrix_local`, `matrix_world`),
//! with Blender's composition and decomposition rules (`BKE_object_to_mat4`,
//! `BKE_object_apply_mat4_ex`, `BKE_object_where_is_calc` for `PAROBJECT`; ported through
//! `frap_math::matrix::ObjectTransform`).
//!
//! `matrix_world` is always computed fresh from the originals (deviation D-04): the parent
//! chain is followed in `bpy.data.objects`, `world = parent_world · matrix_parent_inverse ·
//! matrix_basis`. Only `parent_type = 'OBJECT'` exists in M1; other parent types (armature,
//! bone, vertex) are evaluated as `OBJECT` until M6 (`frap-eval` owns the full evaluation).

use frap_math::EulerOrder;
use frap_math::matrix::{ObjectTransform, RotationMode, RotationValues};
use frap_rna::{IdRef, ObjectId};
use glam::Mat4;

use crate::data::IdMap;
use crate::id::Object;

/// Deepest parent chain followed (a cyclic chain stops there).
pub const MAX_PARENT_DEPTH: usize = 1024;

impl Object {
    /// `rotation_mode` parsed (unknown strings read as `XYZ`, as Blender's RNA never stores one).
    pub fn rotation_mode_enum(&self) -> RotationMode {
        RotationMode::from_blender(&self.rotation_mode)
            .unwrap_or(RotationMode::Euler(EulerOrder::XYZ))
    }
    /// The transform channels as a `frap_math` transform.
    pub fn transform(&self) -> ObjectTransform {
        ObjectTransform {
            location: self.location,
            rotation_mode: self.rotation_mode_enum(),
            rotation: RotationValues {
                quaternion: self.rotation_quaternion,
                euler: self.rotation_euler,
                axis_angle: self.rotation_axis_angle,
            },
            scale: self.scale,
            delta_location: self.delta_location,
            delta_rotation_euler: self.delta_rotation_euler,
            delta_rotation_quaternion: self.delta_rotation_quaternion,
            delta_scale: self.delta_scale,
        }
    }
    /// Writes the transform channels back (location, rotation mode and values, scale).
    pub fn set_transform(&mut self, t: &ObjectTransform) {
        self.location = t.location;
        self.rotation_mode = t.rotation_mode.as_str().into();
        self.rotation_euler = t.rotation.euler;
        self.rotation_quaternion = t.rotation.quaternion;
        self.rotation_axis_angle = t.rotation.axis_angle;
        self.scale = t.scale;
    }
    /// `Object.matrix_basis` (`BKE_object_to_mat4`, deltas included).
    pub fn matrix_basis(&self) -> Mat4 {
        self.transform().to_mat4()
    }
    /// Writing `matrix_basis` (`BKE_object_apply_mat4(ob, m, false, false)`): decomposed into
    /// location, rotation (current mode) and scale; shear is lost.
    pub fn set_matrix_basis(&mut self, m: &Mat4) {
        let mut t = self.transform();
        t.apply_mat4(m, false);
        self.set_transform(&t);
    }
    /// `Object.matrix_local`: `matrix_parent_inverse · matrix_basis` when parented
    /// (`BKE_object_matrix_local_get`, fresh), else `matrix_basis`.
    pub fn matrix_local(&self) -> Mat4 {
        if self.parent.is_some() {
            self.matrix_parent_inverse * self.matrix_basis()
        } else {
            self.matrix_basis()
        }
    }
    /// Writing `matrix_local` (`rna_Object_matrix_local_set`): the parent inverse is undone,
    /// then the matrix is applied as `matrix_basis`.
    pub fn set_matrix_local(&mut self, m: &Mat4) {
        let local = if self.parent.is_some() {
            self.matrix_parent_inverse.inverse() * *m
        } else {
            *m
        };
        self.set_matrix_basis(&local);
    }
    /// Writing `matrix_world` given the parent's world matrix (`BKE_object_apply_mat4(ob, m,
    /// false, true)`): `basis = (parent_world · matrix_parent_inverse)⁻¹ · m`.
    pub fn set_matrix_world(&mut self, m: &Mat4, parent_world: Option<&Mat4>) {
        let local = match parent_world {
            Some(pw) => (*pw * self.matrix_parent_inverse).inverse() * *m,
            None => *m,
        };
        self.set_matrix_basis(&local);
    }
}

/// `Object.matrix_world` of `ob`, fresh: the parent chain composed with each object's
/// `matrix_parent_inverse` and `matrix_basis`. Unknown objects give the identity.
pub fn world_matrix(objects: &IdMap<Object>, ob: IdRef<ObjectId>) -> Mat4 {
    let mut chain = Vec::new();
    let mut cur = Some(ob);
    while let Some(r) = cur {
        let Some(o) = objects.get(r) else {
            break;
        };
        if chain.len() >= MAX_PARENT_DEPTH || chain.contains(&r) {
            break;
        }
        chain.push(r);
        cur = o.parent;
    }
    let mut m = Mat4::IDENTITY;
    for (i, r) in chain.iter().enumerate().rev() {
        let o = objects.get(*r).expect("invariant: in the chain");
        m = if i + 1 == chain.len() {
            o.matrix_basis()
        } else {
            m * o.matrix_parent_inverse * o.matrix_basis()
        };
    }
    m
}

/// `ob_parbone` (WP 6.5): the bone part of a `parent_type = 'BONE'` parent matrix, given the
/// parent armature and the armature-space pose matrix of `ob.parent_bone` — the pose matrix
/// moved to the bone's tail, or the bone's deform matrix (`pose_mat · arm_mat⁻¹`) with
/// `use_relative_parent`. Identity when the bone or its pose is missing (Blender only logs).
/// `parent_bone_head_tail_factor` is not stored by frap (its default 1, the tail, is used).
pub fn bone_parent_offset(ob: &Object, arm: &crate::id::Armature, pose_mat: Option<Mat4>) -> Mat4 {
    let (Some(i), Some(pose_mat)) = (arm.bone_index(&ob.parent_bone), pose_mat) else {
        return Mat4::IDENTITY;
    };
    let rest = crate::edit_armature::rest::rest_pose(arm);
    let Some(r) = rest.get(i) else {
        return Mat4::IDENTITY;
    };
    if arm.bones[i].use_relative_parent {
        pose_mat * crate::edit_armature::math::invert_m4(&r.arm_mat)
    } else {
        let mut m = pose_mat;
        m.w_axis += (m.y_axis.truncate() * r.length).extend(0.0);
        m
    }
}

/// The matrix a child's `matrix_parent_inverse · matrix_basis` is multiplied by
/// (`BKE_object_get_parent_matrix`), from the originals: the parent's `world` matrix, times
/// [`bone_parent_offset`] with the pose of the original channels for `parent_type = 'BONE'`
/// on an armature parent (WP 6.5; other parent types read as `OBJECT` here, `frap-eval`
/// evaluates them).
pub fn parent_matrix_in(main: &crate::Main, child: &Object, parent: &Object, world: Mat4) -> Mat4 {
    if child.parent_type != "BONE" {
        return world;
    }
    let Some(arm) = parent
        .data
        .and_then(|d| d.typed::<frap_rna::ArmatureId>())
        .and_then(|a| main.armatures.get(a))
    else {
        return world;
    };
    let pose_mat = arm.bone_index(&child.parent_bone).map(|i| {
        let rest = crate::edit_armature::rest::rest_pose(arm);
        let mats = crate::edit_armature::posing::where_is(
            arm,
            &rest,
            parent.pose.as_ref(),
            arm.pose_position == "REST",
        );
        mats[i].pose_mat
    });
    world * bone_parent_offset(child, arm, pose_mat)
}

/// [`world_matrix`] with bone parents ([`parent_matrix_in`]); `Object.matrix_world` reads it.
pub fn world_matrix_in(main: &crate::Main, ob: IdRef<ObjectId>) -> Mat4 {
    let objects = &main.objects;
    let mut chain = Vec::new();
    let mut cur = Some(ob);
    while let Some(r) = cur {
        let Some(o) = objects.get(r) else {
            break;
        };
        if chain.len() >= MAX_PARENT_DEPTH || chain.contains(&r) {
            break;
        }
        chain.push(r);
        cur = o.parent;
    }
    let mut m = Mat4::IDENTITY;
    for (i, r) in chain.iter().enumerate().rev() {
        let o = objects.get(*r).expect("invariant: in the chain");
        m = if i + 1 == chain.len() {
            o.matrix_basis()
        } else {
            let p = objects.get(chain[i + 1]).expect("invariant: in the chain");
            parent_matrix_in(main, o, p, m) * o.matrix_parent_inverse * o.matrix_basis()
        };
    }
    m
}

/// [`parent_world_matrix`] with bone parents: the parent matrix of `ob`, if it has a parent.
pub fn parent_world_matrix_in(main: &crate::Main, ob: IdRef<ObjectId>) -> Option<Mat4> {
    let o = main.objects.get(ob)?;
    let p = o.parent?;
    let po = main.objects.get(p)?;
    Some(parent_matrix_in(main, o, po, world_matrix_in(main, p)))
}

/// The world matrix of `ob`'s parent, if it has one.
pub fn parent_world_matrix(objects: &IdMap<Object>, ob: IdRef<ObjectId>) -> Option<Mat4> {
    let p = objects.get(ob)?.parent?;
    objects.get(p)?;
    Some(world_matrix(objects, p))
}

/// True when making `parent` the parent of `child` would close a cycle (`parent` is `child`
/// or one of its descendants).
pub fn would_cycle(
    objects: &IdMap<Object>,
    child: IdRef<ObjectId>,
    parent: IdRef<ObjectId>,
) -> bool {
    let mut cur = Some(parent);
    let mut n = 0;
    while let Some(r) = cur {
        if r == child {
            return true;
        }
        n += 1;
        if n > MAX_PARENT_DEPTH {
            return true;
        }
        cur = objects.get(r).and_then(|o| o.parent);
    }
    false
}
