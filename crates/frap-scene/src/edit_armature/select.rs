// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit-bone selection and visibility tools of WP 8.3.10 (M8): `armature.select_more`,
//! `select_less`, `select_linked`, `select_hierarchy`, `select_mirror`, `select_similar`,
//! `hide` and `reveal`, on one armature in Edit Mode. The operators
//! (`frap-ops-anim/src/ops/armature__select_*.rs`, `armature__{hide,reveal}.rs`) loop over the
//! armatures in Edit Mode; the cross-armature parts of `select_similar` (the active bone's
//! length, direction, prefix, …) are computed by the operator and passed in.
//!
//! Ported from blender/source/blender/editors/armature/armature_select.cc
//! (`armature_select_more`, `armature_select_less`, `armature_select_more_less`,
//! `armature_select_linked_impl`, `armature_select_linked_exec`,
//! `armature_select_hierarchy_exec`, `armature_select_mirror_exec`, `select_similar_*`),
//! blender/source/blender/editors/armature/armature_edit.cc (`armature_hide_exec`,
//! `armature_reveal_exec`), blender/source/blender/editors/armature/armature_utils.cc
//! (`ED_armature_ebone_selectflag_get/enable/disable`) and
//! blender/source/blender/blenlib/intern/string_utils.cc (`BLI_string_split_prefix`,
//! `BLI_string_split_suffix`), Blender 5.2.4, GPL-2.0-or-later.

#![allow(clippy::needless_range_loop)] // index loops mirror Blender's edit-bone list walks

use glam::{Mat4, Vec3};
use smol_str::SmolStr;

use super::math::{mul_m4_v3, mul_mat3_m4_v3, normalize};
use super::{EditArmature, ROOT, SEL, TIP};
use crate::id::Armature;

/// `TOUCH` of `armature_select_linked_impl`: the bone has been walked over.
const TOUCH: u8 = 1 << 0;
/// `LINK`: when touched, the bone is linked to the selection.
const LINK: u8 = 1 << 1;

fn or3(a: [bool; 3], b: [bool; 3]) -> [bool; 3] {
    [a[0] || b[0], a[1] || b[1], a[2] || b[2]]
}

fn and_not3(a: [bool; 3], b: [bool; 3]) -> [bool; 3] {
    [a[0] && !b[0], a[1] && !b[1], a[2] && !b[2]]
}

/// `is_char_sep` of `string_utils.cc`.
fn is_char_sep(c: u8) -> bool {
    matches!(c, b'.' | b' ' | b'-' | b'_')
}

/// `BLI_string_split_prefix`: the prefix up to and including the first separator after the
/// first byte (`L_arm` → `L_`), empty without one.
pub fn split_prefix(name: &str) -> &str {
    let b = name.as_bytes();
    for i in 1..b.len() {
        if is_char_sep(b[i]) {
            return name.get(..=i).unwrap_or("");
        }
    }
    ""
}

/// `BLI_string_split_suffix`: the suffix from the last separator after the first byte
/// (`arm.L` → `.L`), empty without one.
pub fn split_suffix(name: &str) -> &str {
    let b = name.as_bytes();
    for i in (1..b.len()).rev() {
        if is_char_sep(b[i]) {
            return name.get(i..).unwrap_or("");
        }
    }
    ""
}

/// `bone_length_squared_worldspace_get`: the squared length of the bone with both ends
/// transformed by the object's 3×3 (no translation).
pub fn length_squared_world(ob_mat: &Mat4, head: Vec3, tail: Vec3) -> f32 {
    let v1 = mul_mat3_m4_v3(ob_mat, head);
    let v2 = mul_mat3_m4_v3(ob_mat, tail);
    (v1 - v2).length_squared()
}

/// `bone_direction_worldspace_get`: the normalized head − tail in world space.
pub fn direction_world(ob_mat: &Mat4, head: Vec3, tail: Vec3) -> Vec3 {
    let v1 = mul_m4_v3(ob_mat, head);
    let v2 = mul_m4_v3(ob_mat, tail);
    normalize(v1 - v2).0
}

/// `angle_v3v3` (`angle_normalized_v3v3` of the normalized inputs).
pub fn angle_v3v3(a: Vec3, b: Vec3) -> f32 {
    let a = normalize(a).0;
    let b = normalize(b).0;
    if a.dot(b) >= 0.0 {
        2.0 * ((b - a).length() / 2.0).clamp(-1.0, 1.0).asin()
    } else {
        std::f32::consts::PI - 2.0 * ((-b - a).length() / 2.0).clamp(-1.0, 1.0).asin()
    }
}

impl EditArmature {
    /// `ED_armature_ebone_selectflag_get`: a connected bone's head selection is its parent's
    /// tail selection.
    pub fn selectflag_get(&self, i: usize) -> [bool; 3] {
        let b = &self.edit_bones[i];
        match b.parent.filter(|_| b.use_connect) {
            Some(p) => [b.select[SEL], self.edit_bones[p].select[TIP], b.select[TIP]],
            None => b.select,
        }
    }

    /// `ED_armature_ebone_selectflag_enable`.
    pub fn selectflag_enable(&mut self, i: usize, flag: [bool; 3]) {
        let f = or3(self.edit_bones[i].select, flag);
        self.selectflag_set(i, f);
    }

    /// `ED_armature_ebone_selectflag_disable`.
    pub fn selectflag_disable(&mut self, i: usize, flag: [bool; 3]) {
        let f = and_not3(self.edit_bones[i].select, flag);
        self.selectflag_set(i, f);
    }

    /// `armature_select_more_less(ob, more)`: grows (or shrinks) the selection by one bone
    /// along connected chains, from the selection before the call.
    pub fn select_more_less(&mut self, arm: &Armature, more: bool) {
        self.sync_selection();
        let prev: Vec<[bool; 3]> = (0..self.edit_bones.len())
            .map(|i| self.selectflag_get(i))
            .collect();
        for i in 0..self.edit_bones.len() {
            if !self.is_visible(arm, i) {
                continue;
            }
            let connected_parent = self.edit_bones[i]
                .parent
                .filter(|_| self.edit_bones[i].use_connect);
            if more {
                if (prev[i][ROOT] || prev[i][TIP]) && self.is_selectable(arm, i) {
                    self.select_set(i, true);
                }
                if let Some(p) = connected_parent {
                    if prev[i][ROOT] && self.is_selectable(arm, p) {
                        self.selectflag_enable(p, [true; 3]);
                    }
                    if prev[p][TIP] && self.is_selectable(arm, i) {
                        self.selectflag_enable(i, [true, true, false]);
                    }
                }
            } else {
                if !(prev[i][ROOT] && prev[i][TIP]) {
                    self.select_set(i, false);
                }
                if let Some(p) = connected_parent {
                    if !prev[i][SEL] {
                        self.selectflag_disable(p, [true, false, true]);
                    }
                    if !prev[p][SEL] {
                        self.selectflag_disable(i, [true, true, false]);
                    }
                }
            }
        }
        if !more {
            for i in 0..self.edit_bones.len() {
                if self.is_visible(arm, i) && self.edit_bones[i].select[SEL] {
                    self.select_set(i, true);
                }
            }
        }
        self.sync_selection();
    }

    /// `armature_select_linked_exec` for this armature: the visible bones with any selection
    /// flag are the seeds (`BONE_DONE`); whether anything was selected.
    pub fn select_linked(&mut self, arm: &Armature, all_forks: bool) -> bool {
        let done: Vec<bool> = (0..self.edit_bones.len())
            .map(|i| self.is_visible(arm, i) && self.edit_bones[i].any_selected())
            .collect();
        if !done.iter().any(|d| *d) {
            return false;
        }
        self.select_linked_impl(arm, &done, true, all_forks)
    }

    /// `armature_select_linked_impl`: flood-fills from the `done` bones over connected
    /// parent links (`CHECK_PARENT`: connected to a selectable parent); `all_forks` also
    /// walks down every fork of the parents' chains.
    pub fn select_linked_impl(
        &mut self,
        arm: &Armature,
        done: &[bool],
        select: bool,
        all_forks: bool,
    ) -> bool {
        let n = self.edit_bones.len();
        let mut temp = vec![0u8; n];
        let mut changed = false;
        let check_parent = |ea: &EditArmature, i: usize| -> Option<usize> {
            let b = &ea.edit_bones[i];
            b.parent
                .filter(|&p| b.use_connect && ea.is_selectable(arm, p))
        };
        // Select parents.
        for i in 0..n {
            if temp[i] & TOUCH != 0 || !done[i] {
                continue;
            }
            temp[i] |= TOUCH | LINK;
            let mut e = Some(i);
            let mut guard = 0;
            while let Some(x) = e {
                self.select_set(x, select);
                changed = true;
                temp[x] |= if all_forks { TOUCH | LINK } else { TOUCH };
                // Don't walk onto links (messes up the `all_forks` logic).
                if self.edit_bones[x]
                    .parent
                    .is_some_and(|p| temp[p] & LINK != 0)
                {
                    break;
                }
                e = check_parent(self, x);
                guard += 1;
                if guard > n {
                    break;
                }
            }
        }
        // Select children.
        for i in 0..n {
            if check_parent(self, i).is_none() || temp[i] & TOUCH != 0 {
                continue;
            }
            let mut touched_parent = None;
            let mut e = Some(i);
            let mut guard = 0;
            while let Some(x) = e {
                if temp[x] & TOUCH != 0 {
                    touched_parent = Some(x);
                    break;
                }
                temp[x] |= TOUCH;
                e = check_parent(self, x);
                guard += 1;
                if guard > n {
                    break;
                }
            }
            if let Some(tp) = touched_parent
                && temp[tp] & LINK != 0
            {
                let mut x = i;
                let mut guard = 0;
                while x != tp {
                    if temp[x] & LINK == 0 {
                        temp[x] |= LINK;
                        self.select_set(x, select);
                        changed = true;
                    }
                    match self.edit_bones[x].parent {
                        Some(p) if guard <= n => x = p,
                        _ => break,
                    }
                    guard += 1;
                }
            }
        }
        if changed {
            self.sync_selection();
        }
        changed
    }

    /// `armature_select_hierarchy_exec` (`parent` = `direction='PARENT'`): moves the active
    /// bone (and the selection, or adds to it with `extend`) to its parent, or to its first
    /// selectable child (connected children first); whether it changed.
    pub fn select_hierarchy(&mut self, arm: &Armature, parent: bool, extend: bool) -> bool {
        let Some(act) = self.active.filter(|&a| a < self.edit_bones.len()) else {
            return false;
        };
        let target = if parent {
            self.edit_bones[act]
                .parent
                .filter(|&p| self.is_selectable(arm, p))
        } else {
            let mut child = None;
            for pass in 0..2 {
                child = (0..self.edit_bones.len()).find(|&i| {
                    self.is_selectable(arm, i)
                        && self.edit_bones[i].parent == Some(act)
                        && (pass == 1 || self.edit_bones[i].use_connect)
                });
                if child.is_some() {
                    break;
                }
            }
            child
        };
        let Some(t) = target else {
            return false;
        };
        self.active = Some(t);
        if !extend {
            self.select_set(act, false);
        }
        self.select_set(t, true);
        self.sync_selection();
        true
    }

    /// `armature_select_mirror_exec` for this armature: every selectable bone takes the
    /// selection of its visible X mirror (added to its own with `extend`); `only_active`
    /// limits it to the active bone and its mirror; the active bone moves to its mirror.
    pub fn select_mirror(&mut self, arm: &Armature, only_active: bool, extend: bool) {
        let prev: Vec<[bool; 3]> = (0..self.edit_bones.len())
            .map(|i| self.selectflag_get(i))
            .collect();
        let mut mirror_act = None;
        for i in 0..self.edit_bones.len() {
            if !self.is_selectable(arm, i) {
                continue;
            }
            let mut flag_new = if extend { prev[i] } else { [false; 3] };
            if let Some(m) = self.mirrored(i)
                && self.is_visible(arm, m)
            {
                flag_new = or3(flag_new, prev[m]);
                if self.active == Some(i) {
                    mirror_act = Some(m);
                }
                if only_active && self.active != Some(i) && self.active != Some(m) {
                    continue;
                }
            }
            self.selectflag_set(i, flag_new);
        }
        if let Some(m) = mirror_act {
            self.active = Some(m);
        }
        self.sync_selection();
    }

    /// `select_similar_length`: selects the selectable bones whose world-space squared length
    /// lies strictly between `len_min` and `len_max`.
    pub fn select_similar_length(
        &mut self,
        arm: &Armature,
        ob_mat: &Mat4,
        len_min: f32,
        len_max: f32,
    ) -> bool {
        self.select_similar_where(arm, |b| {
            let l = length_squared_world(ob_mat, b.head_v(), b.tail_v());
            l > len_min && l < len_max
        })
    }

    /// `select_similar_direction`: selects the selectable bones whose world-space direction
    /// is within `thresh` (a fraction of π) of `dir_act`.
    pub fn select_similar_direction(
        &mut self,
        arm: &Armature,
        ob_mat: &Mat4,
        dir_act: Vec3,
        thresh: f32,
    ) -> bool {
        self.select_similar_where(arm, |b| {
            let dir = direction_world(ob_mat, b.head_v(), b.tail_v());
            angle_v3v3(dir_act, dir) / std::f32::consts::PI < thresh + f32::EPSILON
        })
    }

    /// `select_similar_prefix` / `select_similar_suffix`: selects the selectable bones whose
    /// name prefix (suffix) equals `part`.
    pub fn select_similar_affix(&mut self, arm: &Armature, part: &str, suffix: bool) -> bool {
        self.select_similar_where(arm, |b| {
            let p = if suffix {
                split_suffix(&b.name)
            } else {
                split_prefix(&b.name)
            };
            p == part
        })
    }

    /// `select_similar_bone_collection`: selects the selectable bones in a collection named
    /// in `names`.
    pub fn select_similar_collections(&mut self, arm: &Armature, names: &[SmolStr]) -> bool {
        self.select_similar_where(arm, |b| b.collections.iter().any(|c| names.contains(c)))
    }

    /// `select_similar_children` (`immediate` = `CHILDREN_IMMEDIATE`) of bone `act`.
    pub fn select_similar_children(&mut self, arm: &Armature, act: usize, immediate: bool) {
        for i in 0..self.edit_bones.len() {
            let hit = if immediate {
                self.edit_bones[i].parent == Some(act)
            } else {
                self.is_child_recursive(act, i)
            };
            if hit && self.is_selectable(arm, i) {
                self.select_set(i, true);
            }
        }
    }

    /// `select_similar_siblings` of bone `act` (nothing for a root bone).
    pub fn select_similar_siblings(&mut self, arm: &Armature, act: usize) {
        let Some(p) = self.edit_bones[act].parent else {
            return;
        };
        for i in 0..self.edit_bones.len() {
            if self.edit_bones[i].parent == Some(p) && self.is_selectable(arm, i) {
                self.select_set(i, true);
            }
        }
    }

    /// Selects every selectable bone passing `f` (`ED_armature_ebone_select_set(…, true)`);
    /// whether any was.
    pub fn select_similar_where(
        &mut self,
        arm: &Armature,
        f: impl Fn(&super::EditBone) -> bool,
    ) -> bool {
        let mut changed = false;
        for i in 0..self.edit_bones.len() {
            if self.is_selectable(arm, i) && f(&self.edit_bones[i]) {
                self.select_set(i, true);
                changed = true;
            }
        }
        changed
    }

    /// `armature_hide_exec` for this armature: hides the visible selected bones (the
    /// unselected ones with `unselected`) and clears their selection; whether any changed.
    pub fn hide(&mut self, arm: &Armature, unselected: bool) -> bool {
        let mut changed = false;
        for i in 0..self.edit_bones.len() {
            if self.is_visible(arm, i) && self.edit_bones[i].select[SEL] != unselected {
                let b = &mut self.edit_bones[i];
                b.select = [false; 3];
                b.hide = true;
                changed = true;
            }
        }
        if changed {
            self.sync_selection();
        }
        changed
    }

    /// `armature_reveal_exec` for this armature: hidden bones in visible collections are shown
    /// and (unless unselectable) get their three selection flags set to `select`.
    pub fn reveal(&mut self, arm: &Armature, select: bool) -> bool {
        let mut changed = false;
        for i in 0..self.edit_bones.len() {
            let visible_colls = super::collections_visible(arm, &self.edit_bones[i].collections);
            let b = &mut self.edit_bones[i];
            if visible_colls && b.hide {
                if !b.hide_select {
                    b.select = [select; 3];
                }
                b.hide = false;
                changed = true;
            }
        }
        if changed {
            self.sync_selection();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connected chain `A` → `B` → `C` along +Z, plus a root `D`.
    fn chain() -> (Armature, EditArmature) {
        let arm = Armature::new("A");
        let mut ea = EditArmature::default();
        for (k, n) in ["A", "B", "C"].into_iter().enumerate() {
            let i = ea.add_bone(n);
            ea.edit_bones[i].head = [0.0, 0.0, k as f32];
            ea.edit_bones[i].tail = [0.0, 0.0, k as f32 + 1.0];
            ea.edit_bones[i].select = [false; 3];
            if k > 0 {
                ea.edit_bones[i].parent = Some(k - 1);
                ea.edit_bones[i].use_connect = true;
            }
        }
        let d = ea.add_bone("D");
        ea.edit_bones[d].tail = [1.0, 0.0, 0.0];
        ea.edit_bones[d].select = [false; 3];
        (arm, ea)
    }

    #[test]
    fn affixes_split_like_blender() {
        assert_eq!(split_prefix("L_arm"), "L_");
        assert_eq!(split_prefix("arm"), "");
        assert_eq!(split_prefix("_x"), "");
        assert_eq!(split_suffix("arm.L"), ".L");
        assert_eq!(split_suffix("arm.L.001"), ".001");
        assert_eq!(split_suffix("arm"), "");
    }

    #[test]
    fn more_then_less_round_trips() {
        let (arm, mut ea) = chain();
        ea.select_set(1, true);
        ea.select_more_less(&arm, true);
        assert!(ea.edit_bones[0].select[SEL] && ea.edit_bones[2].select[SEL]);
        assert!(!ea.edit_bones[3].select[SEL]);
        // A fully selected chain has no boundary; without `A`'s head, `A` is one (deselected,
        // and its child, whose parent was not selected after the sync, too).
        ea.select_more_less(&arm, false);
        assert!((0..3).all(|i| ea.edit_bones[i].select[SEL]));
        ea.edit_bones[0].select[ROOT] = false;
        ea.select_more_less(&arm, false);
        assert!(!ea.edit_bones[0].select[SEL] && !ea.edit_bones[1].select[SEL]);
        assert!(ea.edit_bones[2].select[SEL]);
    }

    #[test]
    fn linked_selects_the_chain() {
        let (arm, mut ea) = chain();
        ea.edit_bones[2].select = [false, false, true];
        assert!(ea.select_linked(&arm, false));
        assert!((0..3).all(|i| ea.edit_bones[i].select[SEL]));
        assert!(!ea.edit_bones[3].select[SEL]);
    }

    #[test]
    fn hierarchy_walks_parent_and_child() {
        let (arm, mut ea) = chain();
        ea.active = Some(1);
        ea.select_set(1, true);
        assert!(ea.select_hierarchy(&arm, true, false));
        assert_eq!(ea.active, Some(0));
        assert!(ea.edit_bones[0].select[SEL] && !ea.edit_bones[1].select[SEL]);
        assert!(ea.select_hierarchy(&arm, false, true));
        assert_eq!(ea.active, Some(1));
        assert!(ea.edit_bones[0].select[SEL] && ea.edit_bones[1].select[SEL]);
        ea.active = Some(3);
        assert!(!ea.select_hierarchy(&arm, true, false));
    }

    #[test]
    fn mirror_swaps_sides() {
        let arm = Armature::new("A");
        let mut ea = EditArmature::default();
        let l = ea.add_bone("arm.L");
        let r = ea.add_bone("arm.R");
        ea.edit_bones[l].select = [true; 3];
        ea.edit_bones[r].select = [false; 3];
        ea.active = Some(l);
        ea.select_mirror(&arm, false, false);
        assert_eq!(ea.edit_bones[l].select, [false; 3]);
        assert_eq!(ea.edit_bones[r].select, [true; 3]);
        assert_eq!(ea.active, Some(r));
    }

    #[test]
    fn hide_and_reveal() {
        let (arm, mut ea) = chain();
        ea.select_set(3, true);
        assert!(ea.hide(&arm, false));
        assert!(ea.edit_bones[3].hide && !ea.edit_bones[3].select[SEL]);
        assert!(ea.reveal(&arm, true));
        assert!(!ea.edit_bones[3].hide && ea.edit_bones[3].select[SEL]);
        assert!(!ea.reveal(&arm, true));
    }

    #[test]
    fn direction_angle_is_symmetric() {
        let a = Vec3::new(0.0, 0.0, 1.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert!((angle_v3v3(a, b) - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((angle_v3v3(a, -a) - std::f32::consts::PI).abs() < 1e-6);
    }
}
