// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.screen.keyframe_jump(next=True)`, tier T0 (WP 6.4). Port of `keyframe_jump_exec`
//! with `keylist_fallback_for_keyframe_jump` (the 3D Viewport's key list;
//! blender/source/blender/editors/screen/screen_ops.cc, editors/animation/keyframes_keylist.cc),
//! Blender 5.2.4, GPL-2.0-or-later: the key times of the scene's own animation and of the
//! active object (its action slot, its shape keys, its data and materials and their node
//! trees; channels of unselected pose bones skipped, `ADS_FILTER_ONLYSEL`) are collected, and
//! the frame moves to the first key after (or the last before) the current frame. No such key:
//! INFO "No more keyframes to jump to in this direction" and CANCELLED. A key on a fractional
//! frame lands on its whole frame (frap stores no sub-frame).

use frap_anim::ActionStrip;
use frap_eval::animate::anim_data;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{EmbeddedSlot, IdAny, IdCode, Root};
use frap_scene::Context;
use frap_spec_ops_anim::screen;

use super::screen__frame_offset::frame;

/// The roots whose keys the 3D Viewport's key list shows: the scene, then the active object
/// with its shape keys, data and materials (and their embedded node trees).
fn sources(c: &Context) -> Vec<Root> {
    let main = c.main;
    let mut out = vec![Root::Id(c.win.scene.any())];
    let Some(ob) = c.active_object() else {
        return out;
    };
    out.push(Root::Id(ob.any()));
    let Some(o) = main.objects.get(ob) else {
        return out;
    };
    let mut mats: Vec<IdAny> = o
        .material_slot_overrides
        .iter()
        .filter_map(|s| s.material.map(|m| m.any()))
        .collect();
    if let Some(d) = o.data {
        if d.code == IdCode::ME
            && let Some(me) = d.typed().and_then(|m| main.meshes.get(m))
        {
            if let Some(k) = me.shape_keys {
                out.push(Root::Id(k.any()));
            }
            mats.extend(me.materials.iter().flatten().map(|m| m.any()));
        }
        out.push(Root::Id(d));
    }
    for m in mats {
        let r = Root::Id(m);
        if !out.contains(&r) {
            out.push(r);
            out.push(Root::Embedded {
                owner: m,
                slot: EmbeddedSlot::NodeTree,
            });
        }
    }
    out
}

/// Whether the channel `path` of `root` belongs to an unselected pose bone.
fn unselected_bone(c: &Context, root: Root, path: &str) -> bool {
    let Root::Id(id) = root else {
        return false;
    };
    let Some(rest) = path.strip_prefix("pose.bones[\"") else {
        return false;
    };
    let Some(name) = rest.split("\"]").next() else {
        return false;
    };
    let Some(o) = id.typed().and_then(|o| c.main.objects.get(o)) else {
        return false;
    };
    o.pose
        .as_ref()
        .and_then(|p| p.bones.iter().find(|b| b.name == name))
        .is_some_and(|b| !b.select)
}

/// Every key time of the sources.
fn key_times(c: &Context) -> Vec<f32> {
    let main = c.main;
    let mut out = Vec::new();
    for root in sources(c) {
        let Some(adt) = anim_data(main, root) else {
            continue;
        };
        let Some(act) = adt.action.and_then(|a| main.actions.get(a)) else {
            continue;
        };
        for layer in &act.layers {
            for strip in &layer.strips {
                let ActionStrip::Keyframe(k) = strip else {
                    continue;
                };
                for bag in k
                    .channelbags
                    .iter()
                    .filter(|b| b.slot_handle == adt.action_slot_handle)
                {
                    for fc in &bag.fcurves {
                        if unselected_bone(c, root, &fc.data_path) {
                            continue;
                        }
                        out.extend(fc.keyframe_points.iter().map(|k| k.co[0]));
                    }
                }
            }
        }
    }
    out
}

#[operator(poll = poll::ALWAYS, tier = T0)]
fn exec(cx: &mut OpCtx, p: screen::KeyframeJump) -> OpResult {
    let cfra = frame::current(cx) as f32;
    let times = key_times(&cx.ctx());
    let target = if p.next.value {
        times
            .iter()
            .copied()
            .filter(|t| *t > cfra)
            .min_by(f32::total_cmp)
    } else {
        times
            .iter()
            .copied()
            .filter(|t| *t < cfra)
            .max_by(f32::total_cmp)
    };
    let Some(t) = target else {
        cx.report(
            ReportType::Info,
            "No more keyframes to jump to in this direction",
        );
        return Ok(OpStatus::Cancelled);
    };
    frame::set(cx, t.floor() as i32);
    Ok(OpStatus::Finished)
}
