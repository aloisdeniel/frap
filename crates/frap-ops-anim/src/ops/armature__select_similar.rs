// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.select_similar(type='LENGTH', threshold=0.1)`, tier T0 (WP 8.3.10). Port
//! of `armature_select_similar_exec` and the `select_similar_*` helpers
//! (blender/source/blender/editors/armature/armature_select.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Selects the selectable bones similar to the active bone (an ERROR `Operation requires an
//! active bone` without one). Over every armature in Edit Mode: `LENGTH` (world-space length
//! within `threshold`, relative), `DIRECTION` (world-space direction within `threshold` × π),
//! `PREFIX` / `SUFFIX` (the name part before the first / from the last separator; nothing
//! when the active bone has none), `BONE_COLLECTION` (sharing a collection name), `COLOR`
//! (frap does not model bone colours, so every bone has the default colour and matches). On
//! the edit object's armature only: `CHILDREN`, `CHILDREN_IMMEDIATE`, `SIBLINGS`, `SHAPE`
//! (same pose-bone custom shape). Always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::select::{
    direction_world, length_squared_world, split_prefix, split_suffix,
};
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_anim::armature::{self, select_similar::Type};

use super::armature__select_all::common;

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_SELECT_SIMILAR
)]
fn exec(cx: &mut OpCtx, p: armature::SelectSimilar) -> OpResult {
    let arms = common::edit_armatures(cx);
    let Some(&(ob_act, arm_act)) = arms.first() else {
        return Ok(OpStatus::Cancelled);
    };
    let Some((_, ea_act)) = common::snapshot(cx, arm_act) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(act) = ea_act.active.filter(|&i| i < ea_act.edit_bones.len()) else {
        return Err(OpError::Runtime("Operation requires an active bone".into()));
    };
    let act_bone = ea_act.edit_bones[act].clone();
    let thresh = p.threshold.value;
    match p.r#type.value {
        Type::Children | Type::ChildrenImmediate => {
            let immediate = p.r#type.value == Type::ChildrenImmediate;
            common::with_edit(cx, arm_act, |ea, a| {
                ea.select_similar_children(a, act, immediate)
            });
        }
        Type::Siblings => {
            common::with_edit(cx, arm_act, |ea, a| ea.select_similar_siblings(a, act));
        }
        Type::Length => {
            let m = world_matrix_in(cx.main(), ob_act);
            let len = length_squared_world(&m, act_bone.head_v(), act_bone.tail_v());
            let len_min = len / (1.0 + (thresh - f32::EPSILON));
            let len_max = len * (1.0 + (thresh + f32::EPSILON));
            for (ob, arm) in arms {
                let m = world_matrix_in(cx.main(), ob);
                common::with_edit(cx, arm, |ea, a| {
                    ea.select_similar_length(a, &m, len_min, len_max)
                });
            }
        }
        Type::Direction => {
            let m = world_matrix_in(cx.main(), ob_act);
            let dir_act = direction_world(&m, act_bone.head_v(), act_bone.tail_v());
            for (ob, arm) in arms {
                let m = world_matrix_in(cx.main(), ob);
                common::with_edit(cx, arm, |ea, a| {
                    ea.select_similar_direction(a, &m, dir_act, thresh)
                });
            }
        }
        Type::Prefix | Type::Suffix => {
            let suffix = p.r#type.value == Type::Suffix;
            let part = if suffix {
                split_suffix(&act_bone.name)
            } else {
                split_prefix(&act_bone.name)
            }
            .to_owned();
            if !part.is_empty() {
                for (_, arm) in arms {
                    common::with_edit(cx, arm, |ea, a| {
                        ea.select_similar_affix(a, &part, suffix)
                    });
                }
            }
        }
        Type::BoneCollection => {
            let names = act_bone.collections.clone();
            for (_, arm) in arms {
                common::with_edit(cx, arm, |ea, a| ea.select_similar_collections(a, &names));
            }
        }
        Type::Color => {
            // Bone colours are not modelled: every bone has the active bone's (default) colour.
            for (_, arm) in arms {
                common::with_edit(cx, arm, |ea, a| ea.select_similar_where(a, |_| true));
            }
        }
        Type::Shape => {
            let shapes: Option<Vec<(smol_str::SmolStr, Option<frap_rna::IdRef<frap_rna::ObjectId>>)>> =
                cx.main().objects.get(ob_act).and_then(|o| o.pose.as_ref()).map(|pose| {
                    pose.bones
                        .iter()
                        .map(|pb| (pb.name.clone(), pb.custom_shape))
                        .collect()
                });
            let shape_of = |name: &str| -> Option<Option<frap_rna::IdRef<frap_rna::ObjectId>>> {
                shapes
                    .as_ref()?
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, s)| *s)
            };
            if let Some(active_shape) = shape_of(&act_bone.name) {
                common::with_edit(cx, arm_act, |ea, a| {
                    ea.select_similar_where(a, |b| shape_of(&b.name) == Some(active_shape))
                });
            }
        }
    }
    Ok(OpStatus::Finished)
}
