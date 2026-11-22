// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit-bone chain tools of WP 8.3.10 (M8): `armature.dissolve`, `armature.split`,
//! `armature.fill` and `armature.align` (the search-and-replace copy of
//! `armature.duplicate_rename` is `EditArmature::duplicate_selected_ex`), on one armature in Edit Mode (the operators,
//! `frap-ops-anim/src/ops/armature__*.rs`, do the multi-object loops, the pose channels and
//! the reports).
//!
//! Ported from blender/source/blender/editors/armature/armature_edit.cc
//! (`armature_dissolve_selected_exec`, `armature_split_exec`, `fill_add_joint`,
//! `armature_fill_bones_exec`, `fix_connected_bone`, `fix_editbone_connected_children`,
//! `bone_align_to_bone`, `armature_align_bones_exec`),
//! blender/source/blender/editors/armature/armature_add.cc (`add_points_bone`) and
//! blender/source/blender/animrig/intern/bone_collections.cc
//! (`ANIM_armature_bonecoll_assign_from_other_editbone`), Blender 5.2.4, GPL-2.0-or-later.

#![allow(clippy::needless_range_loop)] // index loops mirror Blender's edit-bone list walks

use glam::Vec3;
use smol_str::SmolStr;

use super::math::normalize;
use super::{EditArmature, EditBone, ROOT, SEL, TIP};
use crate::id::Armature;

/// `EditBonePoint` of the fill tool: a selected joint and the bones using it as head / tail.
#[derive(Clone, Debug, PartialEq)]
pub struct JointPoint {
    /// The joint position (armature space).
    pub vec: Vec3,
    /// The bone whose head is this joint.
    pub head_owner: Option<usize>,
    /// The bone whose tail is this joint.
    pub tail_owner: Option<usize>,
}

/// What `armature.fill` did, or the ERROR it reports.
#[derive(Clone, Debug, PartialEq)]
pub enum FillOutcome {
    /// A bone was added (index).
    Added(usize),
    /// Two head (or two tail) joints with no rule to order them: nothing added, `FINISHED`.
    Nothing,
}

/// `ANIM_armature_bonecoll_assign_from_other_editbone`: `dst` joins every collection of
/// `src` it is not in yet.
fn assign_from_other(dst: &mut Vec<SmolStr>, src: &[SmolStr]) {
    for c in src {
        if !dst.contains(c) {
            dst.push(c.clone());
        }
    }
}

impl EditArmature {
    /// `armature_dissolve_selected_exec` for this armature: each selected bone whose single
    /// connected child (with a selected head) is selected too merges that child into itself
    /// (tail, tail radius and tail selection taken from the child, which is removed; its
    /// children move up, keeping their connection). Returns the removed bones' names (their
    /// pose channels go too) and whether anything changed.
    pub fn dissolve(&mut self, arm: &Armature) -> (Vec<SmolStr>, bool) {
        let n = self.edit_bones.len();
        // `ebone_flag_orig`: the selection flags `armature_select_mirrored_ex` adds.
        let mut added: Option<Vec<[bool; 3]>> = None;
        if arm.use_mirror_x {
            let orig: Vec<[bool; 3]> = self.edit_bones.iter().map(|b| b.select).collect();
            self.select_mirrored(arm, [true; 3]);
            added = Some(
                self.edit_bones
                    .iter()
                    .zip(&orig)
                    .map(|(b, o)| [0, 1, 2].map(|k| b.select[k] && !o[k]))
                    .collect(),
            );
        }
        // `temp.ebone`: the single connected child (`Some(self)` while ignored).
        let mut temp: Vec<Option<usize>> = vec![None; n];
        for i in 0..n {
            let b = &self.edit_bones[i];
            if let Some(p) = b.parent.filter(|_| b.use_connect) {
                if temp[p] == Some(p) {
                    // ignore
                } else if temp[p].is_some() {
                    temp[p] = Some(p);
                } else {
                    temp[p] = Some(i);
                }
            }
        }
        for i in 0..n {
            if temp[i] == Some(i) {
                temp[i] = None;
            }
        }
        for i in 0..n {
            let f = self.selectflag_get(i);
            let visible = self.is_visible(arm, i);
            // Break the connections of unseen bones.
            if !(visible && (f[TIP] || f[SEL])) {
                temp[i] = None;
            }
            if !(visible && (f[ROOT] || f[SEL])) {
                let b = &self.edit_bones[i];
                if let Some(p) = b.parent.filter(|_| b.use_connect) {
                    temp[p] = None;
                }
            }
        }
        let mut done: Vec<bool> = (0..n)
            .map(|i| {
                self.edit_bones[i]
                    .parent
                    .is_some_and(|p| temp[p] == Some(i))
            })
            .collect();
        let names: Vec<SmolStr> = (0..n)
            .filter(|&i| done[i])
            .map(|i| self.edit_bones[i].name.clone())
            .collect();
        let mut has_temp: Vec<bool> = temp.iter().map(|t| t.is_some()).collect();
        let mut changed = false;
        let mut i = 0;
        while i < self.edit_bones.len() {
            if !done[i] {
                i += 1;
                continue;
            }
            let Some(p) = self.edit_bones[i].parent else {
                i += 1;
                continue;
            };
            let (tail, rad, tip) = {
                let b = &self.edit_bones[i];
                (b.tail, b.tail_radius, b.select[TIP])
            };
            let pb = &mut self.edit_bones[p];
            pb.tail = tail;
            pb.tail_radius = rad;
            pb.select[TIP] = tip;
            self.remove_bone(i, false);
            done.remove(i);
            has_temp.remove(i);
            if let Some(a) = added.as_mut() {
                a.remove(i);
            }
            changed = true;
        }
        if changed {
            for i in 0..self.edit_bones.len() {
                let b = &self.edit_bones[i];
                if let Some(p) = b.parent.filter(|&p| has_temp[p] && b.use_connect) {
                    let r = self.edit_bones[p].tail_radius;
                    self.edit_bones[i].head_radius = r;
                }
            }
            if let Some(a) = &added {
                for (b, f) in self.edit_bones.iter_mut().zip(a) {
                    for k in 0..3 {
                        b.select[k] &= !f[k];
                    }
                }
            }
            self.sync_selection();
        }
        (names, changed)
    }

    /// `armature_split_exec` for this armature: bones whose selection differs from their
    /// parent's lose the parent (and connection); then every bone's three flags are set from
    /// its body selection.
    pub fn split(&mut self) {
        for i in 0..self.edit_bones.len() {
            if let Some(p) = self.edit_bones[i].parent
                && self.edit_bones[i].select[SEL] != self.edit_bones[p].select[SEL]
            {
                let b = &mut self.edit_bones[i];
                b.parent = None;
                b.use_connect = false;
            }
        }
        for i in 0..self.edit_bones.len() {
            let s = self.edit_bones[i].select[SEL];
            self.select_set(i, s);
        }
    }

    /// `fill_add_joint`: adds bone `i`'s head (or tail) to `points`, merged with an existing
    /// point at the same position that it continues (its parent's tail, its child's head).
    pub fn fill_add_joint(&self, i: usize, tail: bool, points: &mut Vec<JointPoint>) {
        let b = &self.edit_bones[i];
        let vec = if tail { b.tail_v() } else { b.head_v() };
        for p in points.iter_mut() {
            if p.vec != vec {
                continue;
            }
            if tail {
                if p.head_owner
                    .is_some_and(|h| self.edit_bones[h].parent == Some(i))
                {
                    p.tail_owner = Some(i);
                    return;
                }
            } else if p.tail_owner.is_some() && b.parent == p.tail_owner {
                p.head_owner = Some(i);
                return;
            }
        }
        points.push(JointPoint {
            vec,
            head_owner: (!tail).then_some(i),
            tail_owner: tail.then_some(i),
        });
    }

    /// The joints `armature.fill` reads from this armature's `visible_bones` (heads of
    /// unconnected bones with a selected head, selected tails); whether any bone gave one.
    pub fn fill_joints(&self, arm: &Armature, points: &mut Vec<JointPoint>) -> bool {
        let mut any = false;
        for i in self.visible_bones(arm) {
            let b = &self.edit_bones[i];
            let mut check = false;
            if !b.use_connect && b.select[ROOT] {
                self.fill_add_joint(i, false, points);
                check = true;
            }
            if b.select[TIP] {
                self.fill_add_joint(i, true, points);
                check = true;
            }
            any |= check;
        }
        any
    }

    /// `add_points_bone`: a new `Bone` (unique name, tail selected) from `head` to `tail`.
    fn add_points_bone(&mut self, head: Vec3, tail: Vec3) -> usize {
        let i = self.add_bone("Bone");
        self.edit_bones[i].head = head.to_array();
        self.edit_bones[i].tail = tail.to_array();
        i
    }

    /// `armature_fill_bones_exec` with one or two joints (`points`, from
    /// [`EditArmature::fill_joints`]) of this armature: one joint makes a bone from it to
    /// `cursor_local` (the 3D cursor in object space); two make a bone between them, its
    /// direction from the active bone, the joint kinds or the cursor distance, parented to the
    /// bone owning the head joint (connected when a tail joint is involved). The new bone takes
    /// the owners' bone collections, becomes active and is the only selection (its tail).
    /// `Err` is the operator's ERROR (`Same bone selected...`).
    pub fn fill(
        &mut self,
        points: &[JointPoint],
        cursor_local: Vec3,
    ) -> Result<FillOutcome, String> {
        let owner_colls = |ea: &EditArmature, p: &JointPoint| -> Vec<SmolStr> {
            p.head_owner
                .or(p.tail_owner)
                .map(|o| ea.edit_bones[o].collections.clone())
                .unwrap_or_default()
        };
        let newbone = match points {
            [ebp] => {
                let colls = owner_colls(self, ebp);
                let nb = self.add_points_bone(ebp.vec, cursor_local);
                assign_from_other(&mut self.edit_bones[nb].collections, &colls);
                nb
            }
            [a, b] => {
                if (a.head_owner == b.tail_owner && a.head_owner.is_some())
                    || (a.tail_owner == b.head_owner && a.tail_owner.is_some())
                {
                    return Err("Same bone selected...".into());
                }
                let owns = |p: &JointPoint, x: Option<usize>| {
                    x.is_some() && (p.head_owner == x || p.tail_owner == x)
                };
                let mut headtail = 0;
                if (a.head_owner.is_some() && b.head_owner.is_some())
                    || (a.tail_owner.is_some() && b.tail_owner.is_some())
                {
                    if owns(a, self.active) {
                        headtail = 1;
                    } else if owns(b, self.active) {
                        headtail = 2;
                    } else {
                        let da = (a.vec - cursor_local).length_squared();
                        let db = (b.vec - cursor_local).length_squared();
                        headtail = if da < db { 2 } else { 1 };
                    }
                } else if a.head_owner.is_some() {
                    headtail = 1;
                } else if b.head_owner.is_some() {
                    headtail = 2;
                }
                let (head, tail, from) = match headtail {
                    2 => (a.vec, b.vec, a),
                    1 => (b.vec, a.vec, b),
                    _ => return Ok(FillOutcome::Nothing),
                };
                let (ca, cb) = (owner_colls(self, a), owner_colls(self, b));
                let nb = self.add_points_bone(head, tail);
                self.edit_bones[nb].parent = from.tail_owner.or(from.head_owner);
                if a.tail_owner.is_some() || b.tail_owner.is_some() {
                    self.edit_bones[nb].use_connect = true;
                }
                assign_from_other(&mut self.edit_bones[nb].collections, &ca);
                assign_from_other(&mut self.edit_bones[nb].collections, &cb);
                nb
            }
            _ => return Ok(FillOutcome::Nothing),
        };
        self.deselect_all();
        self.active = Some(newbone);
        self.edit_bones[newbone].select[TIP] = true;
        Ok(FillOutcome::Added(newbone))
    }

    /// `fix_connected_bone`: a connected bone whose head left its parent's tail moves back to
    /// it (head and tail translated).
    fn fix_connected_bone(&mut self, i: usize) {
        let b = &self.edit_bones[i];
        let Some(p) = b.parent.filter(|_| b.use_connect) else {
            return;
        };
        let ptail = self.edit_bones[p].tail_v();
        if ptail == b.head_v() {
            return;
        }
        let diff = ptail - b.head_v();
        let b = &mut self.edit_bones[i];
        b.head = (b.head_v() + diff).to_array();
        b.tail = (b.tail_v() + diff).to_array();
    }

    /// `fix_editbone_connected_children` (recursive, list order).
    fn fix_connected_children(&mut self, i: usize, depth: usize) {
        if depth > self.edit_bones.len() {
            return;
        }
        for c in 0..self.edit_bones.len() {
            if self.edit_bones[c].parent == Some(i) && self.edit_bones[c].use_connect {
                self.fix_connected_bone(c);
                self.fix_connected_children(c, depth + 1);
            }
        }
    }

    /// `bone_align_to_bone`: bone `sel` keeps its head and length, takes the direction and roll
    /// of `act` (`act_bone`, which may come from another armature), and its connected
    /// descendants follow (`fix_children`: only when `sel` is in this armature's list, as
    /// Blender fixes the children in the active armature only).
    pub fn align_to(&mut self, sel: usize, act: &EditBone, fix_children: bool) {
        let axis = normalize(act.tail_v() - act.head_v()).0;
        let b = &mut self.edit_bones[sel];
        let length = (b.tail_v() - b.head_v()).length();
        b.tail = (b.head_v() + axis * length).to_array();
        b.roll = act.roll;
        if fix_children {
            self.fix_connected_children(sel, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connected chain `A` → `B` → `C` along +Z.
    fn chain() -> (Armature, EditArmature) {
        let arm = Armature::new("A");
        let mut ea = EditArmature::default();
        for (k, n) in ["A", "B", "C"].into_iter().enumerate() {
            let i = ea.add_bone(n);
            ea.edit_bones[i].head = [0.0, 0.0, k as f32];
            ea.edit_bones[i].tail = [0.0, 0.0, k as f32 + 1.0];
            ea.edit_bones[i].tail_radius = 0.01 * (k + 1) as f32;
            ea.edit_bones[i].select = [false; 3];
            if k > 0 {
                ea.edit_bones[i].parent = Some(k - 1);
                ea.edit_bones[i].use_connect = true;
            }
        }
        (arm, ea)
    }

    #[test]
    fn dissolve_merges_the_selected_chain() {
        let (arm, mut ea) = chain();
        for i in 0..3 {
            ea.select_set(i, true);
        }
        let (names, changed) = ea.dissolve(&arm);
        assert!(changed);
        assert_eq!(names, ["B", "C"]);
        assert_eq!(ea.edit_bones.len(), 1);
        assert_eq!(ea.edit_bones[0].tail, [0.0, 0.0, 3.0]);
        assert_eq!(ea.edit_bones[0].tail_radius, 0.03);
    }

    #[test]
    fn dissolve_needs_two_selected_bones() {
        let (arm, mut ea) = chain();
        ea.select_set(1, true);
        ea.edit_bones[0].select = [false; 3];
        // `C`'s head is `B`'s selected tail, so `B` absorbs `C`; `A` is not selected.
        let (names, changed) = ea.dissolve(&arm);
        assert!(changed);
        assert_eq!(names, ["C"]);
        assert_eq!(ea.edit_bones.len(), 2);
        assert_eq!(ea.edit_bones[1].tail, [0.0, 0.0, 3.0]);
        let (arm, mut ea) = chain();
        ea.select_set(2, true);
        ea.edit_bones[1].select[TIP] = false;
        let (_, changed) = ea.dissolve(&arm);
        assert!(!changed);
    }

    #[test]
    fn split_disconnects_selection_borders() {
        let (_, mut ea) = chain();
        ea.select_set(1, true);
        ea.split();
        assert_eq!(ea.edit_bones[1].parent, None);
        assert!(!ea.edit_bones[1].use_connect);
        assert_eq!(ea.edit_bones[2].parent, None);
        assert_eq!(ea.edit_bones[0].select, [false; 3]);
        assert_eq!(ea.edit_bones[1].select, [true; 3]);
    }

    #[test]
    fn fill_between_two_tails_and_to_cursor() {
        let (arm, mut ea) = chain();
        let d = ea.add_bone("D");
        ea.edit_bones[d].head = [1.0, 0.0, 0.0];
        ea.edit_bones[d].tail = [1.0, 0.0, 1.0];
        ea.edit_bones[d].select = [false, false, true];
        ea.edit_bones[0].select = [false, false, true];
        let mut pts = Vec::new();
        assert!(ea.fill_joints(&arm, &mut pts));
        assert_eq!(pts.len(), 2);
        let out = ea.fill(&pts, Vec3::new(5.0, 0.0, 0.0)).expect("fill");
        let FillOutcome::Added(nb) = out else {
            panic!("no bone")
        };
        // Both tails, no active bone: the joint nearer the cursor is the head.
        assert_eq!(ea.edit_bones[nb].head, [1.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[nb].tail, [0.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[nb].parent, Some(d));
        assert!(ea.edit_bones[nb].use_connect);
        assert_eq!(ea.active, Some(nb));
        let mut one = vec![pts[0].clone()];
        one[0].head_owner = None;
        let out = ea.fill(&one, Vec3::new(0.0, 5.0, 0.0)).expect("fill");
        let FillOutcome::Added(nb) = out else {
            panic!("no bone")
        };
        assert_eq!(ea.edit_bones[nb].tail, [0.0, 5.0, 0.0]);
        assert_eq!(ea.edit_bones[nb].parent, None);
    }

    #[test]
    fn align_keeps_length_and_fixes_children() {
        let (_, mut ea) = chain();
        let act = ea.add_bone("X");
        ea.edit_bones[act].tail = [2.0, 0.0, 0.0];
        ea.edit_bones[act].roll = 0.5;
        let a = ea.edit_bones[act].clone();
        ea.align_to(1, &a, true);
        assert_eq!(ea.edit_bones[1].tail, [1.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[1].roll, 0.5);
        assert_eq!(ea.edit_bones[2].head, [1.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[2].tail, [1.0, 0.0, 2.0]);
    }

    #[test]
    fn duplicate_rename_replaces_every_occurrence() {
        let (arm, mut ea) = chain();
        ea.edit_bones[0].name = "AxA".into();
        ea.select_set(0, true);
        ea.edit_bones[1].select = [false; 3];
        ea.edit_bones[0].select[TIP] = true;
        let pairs = ea.duplicate_selected_ex(&arm, false, "A", "Q");
        assert_eq!(pairs.len(), 1);
        assert_eq!(ea.edit_bones[pairs[0].1].name, "QxQ");
    }
}
