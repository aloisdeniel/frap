// SPDX-License-Identifier: GPL-3.0-or-later
//! The pose channels of one armature object as the `pose.*` operators of WP 8.3.11 read and
//! edit them: a copy of the armature (bones, collections, active bone) and of the pose, with
//! the channel ↔ bone index maps, Blender's visibility / selectability tests and the
//! selection writes (`animrig::bone_select` sets `POSE_SELECTED_ALL`).
//!
//! Ported from blender/source/blender/animrig/ANIM_armature.hh (`bone_is_visible`,
//! `bone_is_selected`, `bone_is_selectable`, `bone_select`, `bone_deselect`) and
//! blender/source/blender/blenkernel/intern/object.cc (`BKE_object_pose_array_get_ex`),
//! Blender 5.2.4, GPL-2.0-or-later.

use frap_op::prelude::*;
use frap_rna::{ArmatureId, IdRef, ObjectId};
use frap_scene::ObjectMode;
use frap_scene::id::{Armature, Pose, PoseBone};

/// `BKE_object_pose_array_get_unique` (`unique`) / `BKE_object_pose_array_get`: the armature
/// objects in Pose Mode, active first; empty when the active object is not in Pose Mode.
pub(crate) fn pose_objects(cx: &OpCtx, unique: bool) -> Vec<(IdRef<ObjectId>, IdRef<ArmatureId>)> {
    let c = cx.ctx();
    if c.pose_object().is_none() {
        return Vec::new();
    }
    let obs = if unique {
        c.objects_in_mode_unique_data()
    } else {
        c.objects_in_mode()
    };
    obs.into_iter()
        .filter_map(|ob| {
            let o = c.main.objects.get(ob)?;
            if o.mode != ObjectMode::Pose || o.pose.is_none() {
                return None;
            }
            Some((ob, o.data?.typed::<ArmatureId>()?))
        })
        .collect()
}

/// One armature object's pose channels, copied for editing.
#[derive(Clone, Debug)]
pub(crate) struct Chans {
    /// The armature object.
    pub(crate) ob: IdRef<ObjectId>,
    /// Its armature.
    pub(crate) arm_ref: IdRef<ArmatureId>,
    /// A copy of the armature (bones, collections, `act_bone`).
    pub(crate) arm: Armature,
    /// A copy of the pose (`chanbase`).
    pub(crate) pose: Pose,
    /// Channel index → bone index (`pchan->bone`).
    bone_of: Vec<Option<usize>>,
    /// Bone index → channel index.
    chan_of: Vec<Option<usize>>,
    /// The `act_bone` the copy was made with.
    active0: Option<usize>,
    /// The pose the copy was made with.
    pose0: Pose,
}

impl Chans {
    /// Copies the pose channels of `ob` (`None` without a pose or an armature).
    pub(crate) fn load(
        cx: &OpCtx,
        ob: IdRef<ObjectId>,
        arm_ref: IdRef<ArmatureId>,
    ) -> Option<Chans> {
        let main = cx.main();
        let pose = main.objects.get(ob)?.pose.clone()?;
        let arm = main.armatures.get(arm_ref)?.clone();
        let bone_of: Vec<Option<usize>> =
            pose.bones.iter().map(|p| arm.bone_index(&p.name)).collect();
        let mut chan_of = vec![None; arm.bones.len()];
        for (c, b) in bone_of.iter().enumerate() {
            if let Some(b) = *b
                && chan_of[b].is_none()
            {
                chan_of[b] = Some(c);
            }
        }
        Some(Chans {
            ob,
            arm_ref,
            active0: arm.active_bone,
            pose0: pose.clone(),
            arm,
            pose,
            bone_of,
            chan_of,
        })
    }

    /// The number of channels.
    pub(crate) fn len(&self) -> usize {
        self.pose.bones.len()
    }

    /// Channel `c`.
    pub(crate) fn pb(&self, c: usize) -> &PoseBone {
        &self.pose.bones[c]
    }

    /// Channel `c`, mutably.
    pub(crate) fn pb_mut(&mut self, c: usize) -> &mut PoseBone {
        &mut self.pose.bones[c]
    }

    /// The bone index of channel `c` (`pchan->bone`).
    pub(crate) fn bone(&self, c: usize) -> Option<usize> {
        self.bone_of.get(c).copied().flatten()
    }

    /// The channel of bone `b`.
    pub(crate) fn chan(&self, b: usize) -> Option<usize> {
        self.chan_of.get(b).copied().flatten()
    }

    /// The channel named `name` (`BKE_pose_channel_find_name`).
    pub(crate) fn find(&self, name: &str) -> Option<usize> {
        self.pose.bones.iter().position(|p| p.name == name)
    }

    /// `pchan->parent`: the channel of the parent bone.
    pub(crate) fn parent(&self, c: usize) -> Option<usize> {
        let b = self.bone(c)?;
        self.chan(self.arm.bones[b].parent?)
    }

    /// The channels of the bone's children, in `childbase` order.
    pub(crate) fn children(&self, c: usize) -> Vec<usize> {
        let Some(b) = self.bone(c) else {
            return Vec::new();
        };
        self.arm
            .children_of(Some(b))
            .into_iter()
            .filter_map(|k| self.chan(k))
            .collect()
    }

    /// `ANIM_bone_in_visible_collection` for channel `c`.
    pub(crate) fn in_visible_collection(&self, c: usize) -> bool {
        self.bone(c)
            .is_some_and(|b| self.arm.bone_collections_visible(&self.arm.bones[b]))
    }

    /// `bone_is_visible`: not hidden in Pose Mode (`PCHAN_DRAW_HIDDEN`), in a visible bone
    /// collection.
    pub(crate) fn visible(&self, c: usize) -> bool {
        !self.pose.bones[c].hide && self.in_visible_collection(c)
    }

    /// `bone_is_selected`: selected and visible.
    pub(crate) fn selected(&self, c: usize) -> bool {
        self.pose.bones[c].select && self.visible(c)
    }

    /// `BONE_UNSELECTABLE` (`Bone.hide_select`).
    pub(crate) fn unselectable(&self, c: usize) -> bool {
        self.bone(c).is_some_and(|b| self.arm.bones[b].hide_select)
    }

    /// `bone_is_selectable`: visible and not `hide_select`.
    pub(crate) fn selectable(&self, c: usize) -> bool {
        self.visible(c) && !self.unselectable(c)
    }

    /// `bone_is_connected` (`BONE_CONNECTED`).
    pub(crate) fn connected(&self, c: usize) -> bool {
        self.bone(c).is_some_and(|b| self.arm.bones[b].use_connect)
    }

    /// `animrig::bone_select` (`POSE_SELECTED_ALL`).
    pub(crate) fn select(&mut self, c: usize) {
        set_selection(&mut self.pose.bones[c], true);
    }

    /// `animrig::bone_deselect`.
    pub(crate) fn deselect(&mut self, c: usize) {
        set_selection(&mut self.pose.bones[c], false);
    }

    /// `pose_do_bone_select(…, SEL_SELECT)`: selects unless `hide_select`.
    pub(crate) fn do_select(&mut self, c: usize) {
        if !self.unselectable(c) {
            self.select(c);
        }
    }

    /// The channels of the visible bones (`FOREACH_PCHAN_VISIBLE_IN_OBJECT`; a channel
    /// without a bone is skipped).
    pub(crate) fn visible_chans(&self) -> Vec<usize> {
        (0..self.len())
            .filter(|&c| self.bone(c).is_some() && self.visible(c))
            .collect()
    }

    /// The selected visible channels (`FOREACH_PCHAN_SELECTED_IN_OBJECT`).
    pub(crate) fn selected_chans(&self) -> Vec<usize> {
        (0..self.len())
            .filter(|&c| self.bone(c).is_some() && self.selected(c))
            .collect()
    }

    /// Makes the bone of channel `c` the active bone (`None` clears it).
    pub(crate) fn set_active(&mut self, c: Option<usize>) {
        self.arm.active_bone = c.and_then(|c| self.bone(c));
    }

    /// Writes the pose and the active bone back when they changed.
    pub(crate) fn store(self, cx: &mut OpCtx) -> Result<(), OpError> {
        if self.pose != self.pose0 {
            cx.object_mut(self.ob)?.pose = Some(self.pose);
        }
        if self.arm.active_bone != self.active0 {
            let (main, log) = cx.main_mut();
            if let Some(a) = main.armatures.get_mut(self.arm_ref, log) {
                a.active_bone = self.arm.active_bone;
            }
        }
        Ok(())
    }
}

/// Sets or clears `POSE_SELECTED_ALL`.
pub(crate) fn set_selection(pb: &mut PoseBone, on: bool) {
    pb.select = on;
    pb.select_head = on;
    pb.select_tail = on;
}

/// Every pose object's channels ([`pose_objects`]).
pub(crate) fn load_all(cx: &OpCtx, unique: bool) -> Vec<Chans> {
    pose_objects(cx, unique)
        .into_iter()
        .filter_map(|(ob, arm)| Chans::load(cx, ob, arm))
        .collect()
}
