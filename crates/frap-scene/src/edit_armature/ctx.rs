// SPDX-License-Identifier: GPL-3.0-or-later
//! The bone members of `bpy.context` (WP 6.2, M56W0-12): `active_bone`, `active_pose_bone`,
//! `pose_object`, `selected_bones`, `selected_editable_bones`, `editable_bones`,
//! `visible_bones`, `selected_pose_bones`, `selected_pose_bones_from_active_object`,
//! `visible_pose_bones`. Ports of the `screen_ctx_*` callbacks
//! (blender/source/blender/editors/screen/screen_context.cc) and `BKE_object_pose_armature_get`
//! (blender/source/blender/blenkernel/intern/object.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! A member Blender answers with `CTX_RESULT_NO_DATA` is `None` (the edit-bone lists outside
//! armature Edit Mode, the pose-bone lists outside Pose Mode). Edit bones need the edit state,
//! which a [`crate::Context`] does not borrow, so these members are answered from a
//! [`HostRead`] (the session resolver delegates to [`bone_member`]).

use frap_rna::{ArmatureId, IdRef, ObjectId, Value};

use super::paths::{bone_ptr, edit_bone_ptr, pose_bone_ptr};
use crate::{ContextMode, HostRead, ObjectMode};

/// Every member answered here.
pub const MEMBERS: &[&str] = &[
    "active_bone",
    "active_pose_bone",
    "pose_object",
    "selected_bones",
    "selected_editable_bones",
    "editable_bones",
    "visible_bones",
    "selected_pose_bones",
    "selected_pose_bones_from_active_object",
    "visible_pose_bones",
];

fn list(items: Vec<Value>) -> Value {
    Value::Tuple(items)
}

/// The armature data of an armature object.
fn armature_of(host: &dyn HostRead, ob: IdRef<ObjectId>) -> Option<IdRef<ArmatureId>> {
    host.main().objects.get(ob)?.data?.typed::<ArmatureId>()
}

/// The edit-bone list members over the armatures in Edit Mode (`None` without an armature in
/// Edit Mode).
fn edit_list(host: &dyn HostRead, which: &str) -> Value {
    let ctx = host.ctx();
    if ctx.mode() != ContextMode::EditArmature {
        return Value::None;
    }
    let Some(active) = ctx.active_object() else {
        return Value::None;
    };
    if armature_of(host, active).is_none_or(|a| !host.edit().armatures.contains_key(&a.key())) {
        return Value::None;
    }
    let mut out = Vec::new();
    for ob in ctx.objects_in_mode_unique_data() {
        let Some(arm) = armature_of(host, ob) else {
            continue;
        };
        let (Some(a), Some(ea)) = (
            host.main().armatures.get(arm),
            host.edit().armatures.get(&arm.key()),
        ) else {
            continue;
        };
        let idx = match which {
            "visible_bones" => ea.visible_bones(a),
            "editable_bones" => ea.editable_bones(a),
            "selected_bones" => ea.selected_bones(a, false),
            _ => ea.selected_bones(a, true),
        };
        out.extend(
            idx.into_iter()
                .map(|i| Value::Ptr(Box::new(edit_bone_ptr(arm, &ea.edit_bones[i].name)))),
        );
    }
    list(out)
}

/// The pose bones of `ob` passing `f` (`FOREACH_PCHAN_*_IN_OBJECT`: visible = not hidden and
/// in a visible bone collection).
fn pose_bones(host: &dyn HostRead, ob: IdRef<ObjectId>, selected: bool) -> Vec<Value> {
    let main = host.main();
    let Some(o) = main.objects.get(ob) else {
        return Vec::new();
    };
    let (Some(pose), Some(a)) = (
        o.pose.as_ref(),
        armature_of(host, ob).and_then(|a| main.armatures.get(a)),
    ) else {
        return Vec::new();
    };
    pose.bones
        .iter()
        .filter(|pb| {
            let Some(b) = a.bone(&pb.name) else {
                return false;
            };
            !pb.hide && a.bone_collections_visible(b) && (!selected || pb.select)
        })
        .map(|pb| Value::Ptr(Box::new(pose_bone_ptr(ob, &pb.name))))
        .collect()
}

/// `bpy.context.<member>` for the bone members, `None` for any other name.
pub fn bone_member(host: &dyn HostRead, member: &str) -> Option<Value> {
    if !MEMBERS.contains(&member) {
        return None;
    }
    let ctx = host.ctx();
    let active = ctx.active_object();
    let pose_ob = ctx.pose_object();
    Some(match member {
        "active_bone" => {
            let Some(ob) = active else {
                return Some(Value::None);
            };
            let Some(arm) = armature_of(host, ob) else {
                return Some(Value::None);
            };
            match host.edit().armatures.get(&arm.key()) {
                Some(ea) => match ea.active.and_then(|i| ea.edit_bones.get(i)) {
                    Some(b) => Value::Ptr(Box::new(edit_bone_ptr(arm, &b.name))),
                    None => Value::None,
                },
                None => match host
                    .main()
                    .armatures
                    .get(arm)
                    .and_then(|a| a.active_bone.and_then(|i| a.bones.get(i)))
                {
                    Some(b) => Value::Ptr(Box::new(bone_ptr(arm, &b.name))),
                    None => Value::None,
                },
            }
        }
        "active_pose_bone" => match ctx.active_pose_bone() {
            Some(b) => Value::Ptr(Box::new(pose_bone_ptr(b.object, &b.bone))),
            None => Value::None,
        },
        "pose_object" => Value::Id(pose_ob.map(|o| o.any())),
        "selected_bones" | "selected_editable_bones" | "editable_bones" | "visible_bones" => {
            edit_list(host, member)
        }
        "selected_pose_bones" | "visible_pose_bones" => {
            if pose_ob.is_none() {
                return Some(Value::None);
            }
            let selected = member == "selected_pose_bones";
            let mut out = Vec::new();
            for ob in ctx.objects_in_mode() {
                let is_pose_armature =
                    host.main().objects.get(ob).is_some_and(|o| {
                        o.mode == ObjectMode::Pose && o.object_type() == "ARMATURE"
                    });
                if is_pose_armature {
                    out.extend(pose_bones(host, ob, selected));
                }
            }
            list(out)
        }
        _ => match pose_ob {
            Some(ob) => list(pose_bones(host, ob, true)),
            None => Value::None,
        },
    })
}
