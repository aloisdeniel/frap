// SPDX-License-Identifier: GPL-3.0-or-later
//! The scene state the exporter samples: a copy-on-write clone of `bpy.data` that the
//! animation exporter changes as the add-on changes Blender's (assigning actions and slots,
//! muting NLA tracks, `use_nla`, resetting pose bones and shape keys) and on which
//! `scene.frame_set(frame)` writes the animation of the frame into the originals, as
//! Blender's depsgraph flush does (§7.3, §18: the exporter's sampler works on a per-export
//! clone, never on the session's originals).
//!
//! `frame_set` evaluates every animated object and shape-key datablock with
//! `frap_anim::eval::evaluate_anim_for` (NLA, then the assigned slot) and writes the channels
//! the glTF exporter reads: object transforms (`location`, the rotations, `scale` and their
//! `delta_*` variants), pose-bone transforms (`pose.bones["…"].…`) and shape-key values
//! (`key_blocks["…"].value`, clamped to the slider range as `rna_ShapeKey_value_set` does).
//! Other animated properties do not change TRS or morph weights and are not written (material,
//! light and camera properties are `KHR_animation_pointer`'s, WP 6.9).
//!
//! World matrices are `BKE_object_where_is_calc` through `frap_eval` (every parent type, bone
//! parents from the pose of the frame), pose matrices `BKE_pose_where_is`.

use frap_anim::eval::props::{PathSeg, parse_path, root_struct};
use frap_anim::eval::{ActionLookup, AnimChannel, evaluate_anim_for};
use frap_anim::{Action, AnimData};
use frap_math::glam::Mat4;
use frap_rna::{ActionId, ArmatureId, IdRef, KeyId, MeshId, ObjectId};
use frap_scene::edit_armature::posing::where_is;
use frap_scene::edit_armature::rest::rest_pose;
use frap_scene::id::{Key, Object, PoseBone};
use frap_scene::{ChangeLog, EvaluatedPose, Main};
use smol_str::SmolStr;

/// `bpy.data.actions` for the evaluator.
pub(crate) struct Actions<'a>(pub &'a Main);

impl ActionLookup for Actions<'_> {
    fn action(&self, r: IdRef<ActionId>) -> Option<&Action> {
        self.0.actions.get(r)
    }
}

/// The working copy of the scene state.
pub(crate) struct World {
    /// The clone (shares every datablock until written).
    pub main: Main,
    log: ChangeLog,
}

impl World {
    /// A working copy of `main`.
    pub(crate) fn new(main: &Main) -> World {
        World {
            main: main.clone(),
            log: ChangeLog::default(),
        }
    }

    /// The object `r`.
    pub(crate) fn ob(&self, r: IdRef<ObjectId>) -> Option<&Object> {
        self.main.objects.get(r)
    }

    /// The object `r`, writable.
    pub(crate) fn ob_mut(&mut self, r: IdRef<ObjectId>) -> Option<&mut Object> {
        self.main.objects.get_mut(r, &mut self.log)
    }

    /// The shape-key datablock of a mesh object (`ob.data.shape_keys`).
    pub(crate) fn key_of(&self, r: IdRef<ObjectId>) -> Option<IdRef<KeyId>> {
        let ob = self.ob(r)?;
        let me = ob.data?.typed::<MeshId>()?;
        self.main.meshes.get(me)?.shape_keys
    }

    /// The shape keys of `r`.
    pub(crate) fn key(&self, r: IdRef<ObjectId>) -> Option<&Key> {
        self.main.shape_keys.get(self.key_of(r)?)
    }

    /// The shape keys of `r`, writable.
    pub(crate) fn key_mut(&mut self, r: IdRef<ObjectId>) -> Option<&mut Key> {
        let k = self.key_of(r)?;
        self.main.shape_keys.get_mut(k, &mut self.log)
    }

    /// The object's `AnimData`, writable (`None` without one).
    pub(crate) fn adt_mut(&mut self, r: IdRef<ObjectId>) -> Option<&mut AnimData> {
        self.ob_mut(r)?.animation_data.as_mut()
    }

    /// The shape keys' `AnimData`, writable.
    pub(crate) fn key_adt_mut(&mut self, r: IdRef<ObjectId>) -> Option<&mut AnimData> {
        self.key_mut(r)?.animation_data.as_mut()
    }

    /// `scene.frame_set(frame)`: writes the animation of `frame` of every animated object and
    /// shape-key datablock into the working originals.
    pub(crate) fn frame_set(&mut self, frame: i32) {
        let f = frame as f32;
        let mut obs: Vec<(IdRef<ObjectId>, Vec<AnimChannel>)> = Vec::new();
        let mut keys: Vec<(IdRef<KeyId>, Vec<AnimChannel>)> = Vec::new();
        {
            let acts = Actions(&self.main);
            for (r, ob) in self.main.objects.iter() {
                if let Some(adt) = &ob.animation_data
                    && (adt.action.is_some() || !adt.nla_tracks.is_empty())
                {
                    let ch = evaluate_anim_for(adt, &acts, f, root_struct("OBJECT"));
                    if !ch.is_empty() {
                        obs.push((r, ch));
                    }
                }
            }
            for (r, key) in self.main.shape_keys.iter() {
                if let Some(adt) = &key.animation_data
                    && (adt.action.is_some() || !adt.nla_tracks.is_empty())
                {
                    let ch = evaluate_anim_for(adt, &acts, f, root_struct("KEY"));
                    if !ch.is_empty() {
                        keys.push((r, ch));
                    }
                }
            }
        }
        for (r, ch) in obs {
            if let Some(ob) = self.main.objects.get_mut(r, &mut self.log) {
                for c in &ch {
                    write_object_channel(ob, &c.data_path, c.array_index, c.value);
                }
            }
        }
        for (r, ch) in keys {
            if let Some(k) = self.main.shape_keys.get_mut(r, &mut self.log) {
                for c in &ch {
                    write_key_channel(k, &c.data_path, c.array_index, c.value);
                }
            }
        }
    }

    /// `Object.matrix_world` of the current state.
    pub(crate) fn matrix_world(&self, r: IdRef<ObjectId>) -> Mat4 {
        self.world_rec(r, 0)
    }

    fn world_rec(&self, r: IdRef<ObjectId>, depth: usize) -> Mat4 {
        let Some(ob) = self.ob(r) else {
            return Mat4::IDENTITY;
        };
        let parent = ob
            .parent
            .filter(|p| self.main.objects.contains(*p) && depth < self.main.objects.len());
        let Some(p) = parent else {
            return frap_eval::world_matrix(ob, None);
        };
        let pw = self.world_rec(p, depth + 1);
        let pob = self.ob(p);
        let par = match ob.parent_type.as_str() {
            "BONE" => {
                let arm = pob
                    .and_then(|o| o.data?.typed::<ArmatureId>())
                    .and_then(|a| self.main.armatures.get(a));
                let pose = self.pose(p);
                frap_eval::bone_parent_matrix(ob, &pw, pose.as_ref(), arm)
            }
            _ => {
                let mesh = pob
                    .and_then(|o| o.data?.typed::<MeshId>())
                    .and_then(|m| self.main.meshes.get(m));
                frap_eval::parent_matrix(ob, &pw, mesh)
            }
        };
        frap_eval::world_matrix(ob, Some(&par))
    }

    /// The evaluated pose of armature object `r` (`PoseBone.matrix` per bone, armature
    /// space); `None` for other objects.
    pub(crate) fn pose(&self, r: IdRef<ObjectId>) -> Option<EvaluatedPose> {
        let ob = self.ob(r)?;
        let arm = self.main.armatures.get(ob.data?.typed::<ArmatureId>()?)?;
        let rest = rest_pose(arm);
        let mats = where_is(arm, &rest, ob.pose.as_ref(), arm.pose_position == "REST");
        Some(EvaluatedPose {
            bones: arm
                .bones
                .iter()
                .zip(mats)
                .map(|(b, m)| (b.name.clone(), m.pose_mat))
                .collect(),
        })
    }

    /// `PoseBone.matrix_basis = Matrix()` for every bone (`reset_bone_matrix`).
    pub(crate) fn reset_pose(&mut self, r: IdRef<ObjectId>) {
        let names: Vec<SmolStr> = {
            let Some(ob) = self.ob(r) else {
                return;
            };
            let Some(arm) = ob
                .data
                .and_then(|d| d.typed::<ArmatureId>())
                .and_then(|a| self.main.armatures.get(a))
            else {
                return;
            };
            arm.bones.iter().map(|b| b.name.clone()).collect()
        };
        let Some(ob) = self.ob_mut(r) else {
            return;
        };
        for n in names {
            let pb = pose_bone_mut(ob, &n);
            pb.location = [0.0; 3];
            pb.rotation_quaternion = [1.0, 0.0, 0.0, 0.0];
            pb.rotation_euler = [0.0; 3];
            pb.rotation_axis_angle = [0.0, 0.0, 1.0, 0.0];
            pb.scale = [1.0; 3];
        }
    }
}

/// The pose channel `name` of `ob`, created as `BKE_pose_ensure` would when missing.
fn pose_bone_mut<'a>(ob: &'a mut Object, name: &str) -> &'a mut PoseBone {
    let pose = ob.pose.get_or_insert_with(frap_scene::id::Pose::default);
    let i = match pose.bones.iter().position(|b| b.name == name) {
        Some(i) => i,
        None => {
            pose.bones.push(PoseBone::new(name));
            pose.bones.len() - 1
        }
    };
    &mut pose.bones[i]
}

fn set(slot: &mut [f32], index: i32, value: f32) {
    if let Some(v) = usize::try_from(index).ok().and_then(|i| slot.get_mut(i)) {
        *v = value;
    }
}

/// Writes one evaluated channel of an object (`animsys_write_orig_anim_rna` for the
/// transform properties); other paths are ignored.
pub(crate) fn write_object_channel(ob: &mut Object, path: &str, index: i32, value: f32) {
    let Some(segs) = parse_path(path) else {
        return;
    };
    match segs.as_slice() {
        [PathSeg::Prop(p)] => match p.as_str() {
            "location" => set(&mut ob.location, index, value),
            "rotation_euler" => set(&mut ob.rotation_euler, index, value),
            "rotation_quaternion" => set(&mut ob.rotation_quaternion, index, value),
            "rotation_axis_angle" => set(&mut ob.rotation_axis_angle, index, value),
            "scale" => set(&mut ob.scale, index, value),
            "delta_location" => set(&mut ob.delta_location, index, value),
            "delta_rotation_euler" => set(&mut ob.delta_rotation_euler, index, value),
            "delta_rotation_quaternion" => set(&mut ob.delta_rotation_quaternion, index, value),
            "delta_scale" => set(&mut ob.delta_scale, index, value),
            _ => {}
        },
        [
            PathSeg::Prop(pose),
            PathSeg::Prop(bones),
            PathSeg::Key(name),
            PathSeg::Prop(p),
        ] if pose == "pose" && bones == "bones" => {
            if !ob
                .pose
                .as_ref()
                .is_some_and(|ps| ps.bones.iter().any(|b| b.name == *name))
            {
                // `BKE_pose_ensure` builds a channel per bone; a channel that does not exist
                // stays unresolved (the F-curve is skipped).
                if ob.pose.is_some() {
                    return;
                }
            }
            let pb = pose_bone_mut(ob, name);
            match p.as_str() {
                "location" => set(&mut pb.location, index, value),
                "rotation_quaternion" => set(&mut pb.rotation_quaternion, index, value),
                "rotation_euler" => set(&mut pb.rotation_euler, index, value),
                "rotation_axis_angle" => set(&mut pb.rotation_axis_angle, index, value),
                "scale" => set(&mut pb.scale, index, value),
                _ => {}
            }
        }
        _ => {}
    }
}

/// Writes one evaluated channel of a shape-key datablock (`key_blocks[…].value`).
pub(crate) fn write_key_channel(key: &mut Key, path: &str, _index: i32, value: f32) {
    let Some(segs) = parse_path(path) else {
        return;
    };
    let [PathSeg::Prop(kb), sel, PathSeg::Prop(p)] = segs.as_slice() else {
        return;
    };
    if kb != "key_blocks" || p != "value" {
        return;
    }
    let block = match sel {
        PathSeg::Key(name) => key.key_blocks.iter_mut().find(|b| b.name == *name),
        PathSeg::Index(i) => usize::try_from(*i)
            .ok()
            .and_then(|i| key.key_blocks.get_mut(i)),
        PathSeg::Prop(_) => None,
    };
    if let Some(b) = block {
        b.set_value(value);
    }
}

/// `get_sk_exported(key_blocks)`: the shape keys the exporter writes as morph targets (not
/// the first, not muted, not relative to themselves), as indices.
pub(crate) fn exported_shape_keys(key: &Key) -> Vec<usize> {
    let first = key.key_blocks.first().map(|b| b.name.clone());
    key.key_blocks
        .iter()
        .enumerate()
        .filter(|(i, b)| !(b.relative_key == *i || b.mute || Some(&b.name) == first.as_ref()))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_channels_are_written() {
        let mut ob = Object::new("O", None);
        write_object_channel(&mut ob, "location", 1, 2.0);
        write_object_channel(&mut ob, "rotation_quaternion", 3, 0.5);
        write_object_channel(&mut ob, "modifiers[\"x\"].levels", 0, 3.0);
        assert_eq!(ob.location, [0.0, 2.0, 0.0]);
        assert_eq!(ob.rotation_quaternion[3], 0.5);
        write_object_channel(&mut ob, "pose.bones[\"B\"].location", 0, 1.5);
        let pb = &ob.pose.as_ref().unwrap().bones[0];
        assert_eq!((pb.name.as_str(), pb.location[0]), ("B", 1.5));
    }

    #[test]
    fn shape_key_values_clamp_to_the_slider() {
        let mut k = Key::new("Key");
        k.key_blocks
            .push(frap_scene::id::ShapeKey::new("Basis", vec![]));
        k.key_blocks
            .push(frap_scene::id::ShapeKey::new("A", vec![]));
        write_key_channel(&mut k, "key_blocks[\"A\"].value", 0, 3.0);
        assert_eq!(k.key_blocks[1].value, 1.0);
        write_key_channel(&mut k, "key_blocks[1].value", 0, 0.25);
        assert_eq!(k.key_blocks[1].value, 0.25);
        assert_eq!(exported_shape_keys(&k), vec![1]);
    }
}
