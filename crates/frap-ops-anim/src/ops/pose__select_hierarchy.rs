// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.pose.select_hierarchy(direction='PARENT', extend=False)`, tier T0 (WP 8.3.11).
//! Port of `pose_select_hierarchy_exec`, `pose_select_parents` and `pose_select_children`
//! (blender/source/blender/editors/armature/pose_select.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! On every armature in Pose Mode (unique data): `PARENT` selects the parents of the selected
//! visible bones and moves the active bone to its parent; `CHILD` selects the direct children
//! of the selected bones and moves the active bone to its first connected child (the first
//! child when none is connected). Without `extend` the previously selected bones are
//! deselected first. An armature whose selected bones have no parent (`PARENT`) or no child
//! (`CHILD`) is left alone; `CANCELLED` when nothing was selected anywhere (the statement is
//! rolled back; Blender keeps the deselect, D-162).
//!
//! [`family`] holds the parent / children / siblings selection that `pose.select_grouped`
//! shares; [`chans`] the pose-channel copy every WP 8.3.11 `pose.*` operator edits.

pub(crate) mod chans;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_anim::pose::{self, select_hierarchy::Direction};

/// `pose_select_parents`, `pose_select_children`, `pose_select_siblings`.
pub(crate) mod family {
    use frap_op::prelude::*;

    use super::chans::{Chans, load_all};

    /// The selected visible channels of `ch` (`get_selected_pose_bones`).
    fn selected_set(ch: &Chans) -> Vec<usize> {
        (0..ch.len())
            .filter(|&c| ch.bone(c).is_some() && ch.selected(c))
            .collect()
    }

    /// `pose_select_parents(C, extend, stop_at_root, modify_active)`.
    pub(crate) fn select_parents(
        cx: &mut OpCtx,
        extend: bool,
        stop_at_root: bool,
        modify_active: bool,
    ) -> Result<bool, OpError> {
        let mut changed_any = false;
        for mut ch in load_all(cx, true) {
            let active = ch.arm.active_bone;
            let selected = selected_set(&ch);
            if stop_at_root && !selected.iter().any(|&c| ch.parent(c).is_some()) {
                continue;
            }
            if !extend {
                for &c in &selected {
                    ch.deselect(c);
                }
            }
            for &c in &selected {
                let Some(p) = ch.parent(c) else {
                    continue;
                };
                if !ch.selectable(p) {
                    continue;
                }
                ch.do_select(p);
                if modify_active && ch.bone(c) == active {
                    ch.set_active(Some(p));
                }
                changed_any = true;
            }
            ch.store(cx)?;
        }
        Ok(changed_any)
    }

    /// `get_new_active_child`: the first connected child, else the first child.
    fn new_active_child(ch: &Chans, parent: usize) -> Option<usize> {
        let kids = ch.children(parent);
        kids.iter()
            .copied()
            .find(|&k| ch.connected(k))
            .or_else(|| kids.first().copied())
    }

    /// `pose_select_children(C, all, extend, stop_at_leaf, modify_active)`.
    pub(crate) fn select_children(
        cx: &mut OpCtx,
        all: bool,
        extend: bool,
        stop_at_leaf: bool,
        modify_active: bool,
    ) -> Result<bool, OpError> {
        let mut changed_any = false;
        for mut ch in load_all(cx, true) {
            let active = ch.arm.active_bone;
            let selected = selected_set(&ch);
            if stop_at_leaf && !selected.iter().any(|&c| !ch.children(c).is_empty()) {
                continue;
            }
            if !extend {
                for &c in &selected {
                    ch.deselect(c);
                }
            }
            for c in 0..ch.len() {
                if ch.bone(c).is_none() || !ch.selectable(c) {
                    continue;
                }
                let hit = if all {
                    // `pose_bone_is_below_one_of`: any ancestor (not the bone itself).
                    let mut p = ch.parent(c);
                    let mut depth = 0;
                    let mut found = false;
                    while let Some(i) = p {
                        if selected.contains(&i) {
                            found = true;
                            break;
                        }
                        depth += 1;
                        if depth > ch.len() {
                            break;
                        }
                        p = ch.parent(i);
                    }
                    found
                } else {
                    ch.parent(c).is_some_and(|p| selected.contains(&p))
                };
                if !hit {
                    continue;
                }
                ch.do_select(c);
                if let Some(p) = ch.parent(c)
                    && modify_active
                    && ch.bone(p) == active
                {
                    let next = new_active_child(&ch, p);
                    ch.set_active(next);
                }
                changed_any = true;
            }
            ch.store(cx)?;
        }
        Ok(changed_any)
    }

    /// `pose_select_siblings(C, extend)`: without `extend` the **parents** of the selected
    /// bones are deselected (Blender's code, kept); then every selectable, unselected bone
    /// whose parent is the parent of a selected bone gets selected (root bones have no
    /// siblings).
    pub(crate) fn select_siblings(cx: &mut OpCtx, extend: bool) -> Result<bool, OpError> {
        let mut changed_any = false;
        for mut ch in load_all(cx, true) {
            let parents: Vec<Option<usize>> = selected_set(&ch)
                .into_iter()
                .map(|c| ch.parent(c))
                .collect();
            if !extend {
                for p in parents.iter().flatten() {
                    ch.deselect(*p);
                }
            }
            for c in 0..ch.len() {
                if ch.bone(c).is_none() || !ch.selectable(c) {
                    continue;
                }
                let Some(p) = ch.parent(c) else {
                    continue;
                };
                if !parents.contains(&Some(p)) {
                    continue;
                }
                if !ch.selected(c) {
                    ch.do_select(c);
                    changed_any = true;
                }
            }
            ch.store(cx)?;
        }
        Ok(changed_any)
    }
}

#[operator(poll = poll::POSEMODE, tier = T0, spec = pose::OPSPEC_POSE_OT_SELECT_HIERARCHY)]
fn exec(cx: &mut OpCtx, p: pose::PoseSelectHierarchy) -> OpResult {
    let extend = p.extend.value;
    let changed = match p.direction.value {
        Direction::Parent => family::select_parents(cx, extend, true, true)?,
        Direction::Child => family::select_children(cx, false, extend, true, true)?,
    };
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
