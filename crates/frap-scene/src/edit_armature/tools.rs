// SPDX-License-Identifier: GPL-3.0-or-later
//! The edit-bone tools behind the `armature.*` operators (WP 6.2), on one armature in Edit
//! Mode. The operators (`frap-ops-anim/src/ops/armature__*.rs`) loop over the armatures in
//! Edit Mode and do the pose-channel bookkeeping; everything about edit bones is here.
//!
//! Ported from blender/source/blender/editors/armature/armature_add.cc
//! (`armature_extrude_exec`, `armature_duplicate_selected`, `armature_symmetrize_exec`,
//! `armature_subdivide_exec`, `armature_bone_primitive_add_exec`),
//! blender/source/blender/editors/armature/armature_edit.cc (`ED_armature_ebone_roll_to_vector`,
//! `armature_calc_roll_exec`, `armature_roll_clear_exec`, `chains_find_tips`,
//! `armature_switch_direction_exec`, `armature_delete_selected_exec`),
//! blender/source/blender/editors/armature/armature_relations.cc (`armature_parent_set_exec`,
//! `armature_parent_clear_exec`), blender/source/blender/editors/armature/armature_select.cc
//! (`armature_de_select_all_exec`, `ED_armature_edit_deselect_all`) and
//! blender/source/blender/editors/armature/armature_utils.cc
//! (`ED_armature_ebone_transform_mirror_update`, `armature_select_mirrored`,
//! `armature_tag_select_mirrored`), Blender 5.2.4, GPL-2.0-or-later.

#![allow(clippy::needless_range_loop)] // index loops mirror Blender's edit-bone pointer walks

use glam::{Mat3, Vec3};

use super::math::{normalize, vec_roll_to_mat3_normalized};
use super::names::flip_side_name;
use super::{EditArmature, EditBone, ROOT, SEL, TIP};
use crate::id::Armature;

/// `armature.select_all(action=…)` / `pose.select_all` actions (`SEL_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SelectAction {
    /// `TOGGLE`
    Toggle,
    /// `SELECT`
    Select,
    /// `DESELECT`
    Deselect,
    /// `INVERT`
    Invert,
}

impl SelectAction {
    /// From the RNA identifier.
    pub fn from_identifier(s: &str) -> SelectAction {
        match s {
            "SELECT" => SelectAction::Select,
            "DESELECT" => SelectAction::Deselect,
            "INVERT" => SelectAction::Invert,
            _ => SelectAction::Toggle,
        }
    }
}

/// `project_v3_v3v3_normalized`.
fn project_normalized(p: Vec3, v_proj: Vec3) -> Vec3 {
    v_proj * p.dot(v_proj)
}

/// `angle_v3v3` (`angle_normalized_v3v3` of the normalized inputs).
fn angle_v3v3(a: Vec3, b: Vec3) -> f32 {
    let a = normalize(a).0;
    let b = normalize(b).0;
    // `angle_normalized_v3v3`: the stable formula on both sides of 90°.
    if a.dot(b) >= 0.0 {
        2.0 * ((b - a).length() / 2.0).clamp(-1.0, 1.0).asin()
    } else {
        std::f32::consts::PI - 2.0 * ((-b - a).length() / 2.0).clamp(-1.0, 1.0).asin()
    }
}

impl EditArmature {
    /// `ED_armature_edit_deselect_all`: clears every selection flag (visible or not); whether
    /// anything changed.
    pub fn deselect_all(&mut self) -> bool {
        let mut changed = false;
        for b in &mut self.edit_bones {
            if b.any_selected() {
                b.select = [false; 3];
                changed = true;
            }
        }
        changed
    }

    /// `context.visible_bones` of this armature (with the mirrored copies `use_mirror_x` adds
    /// for invisible mirror bones, as `screen_ctx_visible_or_editable_bones_` does).
    pub fn visible_bones(&self, arm: &Armature) -> Vec<usize> {
        let mut out = Vec::new();
        for i in 0..self.edit_bones.len() {
            if !self.is_visible(arm, i) {
                continue;
            }
            out.push(i);
            if arm.use_mirror_x
                && let Some(f) = self.mirrored(i)
                && !self.is_visible(arm, f)
            {
                out.push(f);
            }
        }
        out
    }

    /// `context.selected_bones` (`editable` = `selected_editable_bones`) of this armature,
    /// with the mirrored copies `use_mirror_x` adds.
    pub fn selected_bones(&self, arm: &Armature, editable: bool) -> Vec<usize> {
        let mut out = Vec::new();
        for i in 0..self.edit_bones.len() {
            if !(self.is_visible(arm, i) && self.edit_bones[i].select[SEL]) {
                continue;
            }
            if editable && !self.is_editable(i) {
                continue;
            }
            out.push(i);
            if arm.use_mirror_x
                && let Some(f) = self.mirrored(i)
                && !self.edit_bones[f].select[SEL]
            {
                out.push(f);
            }
        }
        out
    }

    /// `context.editable_bones` of this armature.
    pub fn editable_bones(&self, arm: &Armature) -> Vec<usize> {
        let mut out = Vec::new();
        for i in 0..self.edit_bones.len() {
            if !self.is_visible(arm, i) || !self.is_editable(i) {
                continue;
            }
            out.push(i);
            if arm.use_mirror_x
                && let Some(f) = self.mirrored(i)
                && !self.edit_bones[f].select[SEL]
            {
                out.push(f);
            }
        }
        out
    }

    /// `armature_de_select_all_exec` over the visible bones (`action` already resolved from
    /// `TOGGLE` by the caller with [`EditArmature::any_visible_selected`]).
    pub fn select_all(&mut self, arm: &Armature, action: SelectAction) {
        for i in self.visible_bones(arm) {
            let selectable = !self.edit_bones[i].hide_select;
            let connect_parent = self.edit_bones[i]
                .parent
                .filter(|_| self.edit_bones[i].use_connect);
            let select = |ea: &mut EditArmature| {
                ea.edit_bones[i].select = [true; 3];
                if let Some(p) = connect_parent {
                    ea.edit_bones[p].select[TIP] = true;
                }
            };
            match action {
                SelectAction::Select | SelectAction::Toggle => {
                    if selectable {
                        select(self);
                    }
                }
                SelectAction::Deselect => self.edit_bones[i].select = [false; 3],
                SelectAction::Invert => {
                    if self.edit_bones[i].select[SEL] {
                        self.edit_bones[i].select = [false; 3];
                    } else if selectable {
                        select(self);
                    }
                }
            }
        }
    }

    /// Whether any visible bone has any selection flag (the `TOGGLE` test of
    /// `armature.select_all`).
    pub fn any_visible_selected(&self, arm: &Armature) -> bool {
        self.visible_bones(arm)
            .into_iter()
            .any(|i| self.edit_bones[i].any_selected())
    }

    /// `armature_select_mirrored_ex(arm, flag)`.
    pub fn select_mirrored(&mut self, arm: &Armature, flags: [bool; 3]) {
        if !arm.use_mirror_x {
            return;
        }
        for i in 0..self.edit_bones.len() {
            if !self.is_visible(arm, i) {
                continue;
            }
            let sel = self.edit_bones[i].select;
            if !(0..3).any(|k| flags[k] && sel[k]) {
                continue;
            }
            if let Some(m) = self.mirrored(i) {
                for k in 0..3 {
                    if flags[k] && sel[k] {
                        self.edit_bones[m].select[k] = true;
                    }
                }
            }
        }
    }

    /// `armature_tag_select_mirrored`: mirrored bones of selected ones take their selection;
    /// returns the tagged bones (`BONE_DONE`) for [`EditArmature::tag_unselect`].
    pub fn tag_select_mirrored(&mut self, arm: &Armature) -> Vec<usize> {
        let mut done = Vec::new();
        if !arm.use_mirror_x {
            return done;
        }
        for i in 0..self.edit_bones.len() {
            if self.is_visible(arm, i)
                && self.edit_bones[i].any_selected()
                && let Some(m) = self.mirrored(i)
                && !self.edit_bones[m].select[SEL]
                && !done.contains(&m)
            {
                done.push(m);
            }
        }
        for &i in &done {
            if let Some(m) = self.mirrored(i) {
                let s = self.edit_bones[m].select;
                for k in 0..3 {
                    self.edit_bones[i].select[k] |= s[k];
                }
            }
        }
        done
    }

    /// `armature_tag_unselect`.
    pub fn tag_unselect(&mut self, done: &[usize]) {
        for &i in done {
            if let Some(b) = self.edit_bones.get_mut(i) {
                b.select = [false; 3];
            }
        }
    }

    /// `ED_armature_ebone_roll_to_vector`: the roll that turns bone `i`'s Z axis towards
    /// `align_axis` (unit), 0 for a zero-length or aligned bone.
    pub fn roll_to_vector(&self, i: usize, align_axis: Vec3, axis_only: bool) -> f32 {
        roll_to_vector(&self.edit_bones[i], align_axis, axis_only)
    }

    /// `ED_armature_ebone_transform_mirror_update`: mirrors bone `i`'s ends onto its X mirror.
    pub fn transform_mirror_update(&mut self, arm: &Armature, i: usize, check_select: bool) {
        let b = self.edit_bones[i].clone();
        if check_select && !(self.is_visible(arm, i) && (b.select[TIP] || b.select[ROOT])) {
            return;
        }
        let Some(f) = self.mirrored(i) else {
            return;
        };
        self.edit_bones[f].roll = -b.roll;
        if !check_select || b.select[TIP] {
            let fb = &mut self.edit_bones[f];
            fb.tail = [-b.tail[0], b.tail[1], b.tail[2]];
            fb.tail_radius = b.tail_radius;
            let ftail = fb.tail;
            for c in &mut self.edit_bones {
                if c.parent == Some(f) && c.use_connect {
                    c.head = ftail;
                    c.head_radius = b.tail_radius;
                }
            }
        }
        if !check_select || b.select[ROOT] {
            let fb = &mut self.edit_bones[f];
            fb.head = [-b.head[0], b.head[1], b.head[2]];
            fb.head_radius = b.head_radius;
            let (fhead, fparent, fconnect) = (fb.head, fb.parent, fb.use_connect);
            if let Some(p) = fparent.filter(|_| fconnect) {
                self.edit_bones[p].tail = fhead;
                self.edit_bones[p].tail_radius = b.head_radius;
            }
        }
        if !check_select || b.select[SEL] {
            let fb = &mut self.edit_bones[f];
            fb.envelope_distance = b.envelope_distance;
            fb.envelope_weight = b.envelope_weight;
            fb.bbone_segments = b.bbone_segments;
        }
    }

    /// `ED_armature_edit_transform_mirror_update`.
    pub fn transform_mirror_update_all(&mut self, arm: &Armature) {
        for i in 0..self.edit_bones.len() {
            self.transform_mirror_update(arm, i, true);
        }
    }

    /// `armature_extrude_exec` for this armature: returns the number of new bones (0: the
    /// armature is left as it was, apart from the root-selection fix-up).
    pub fn extrude(&mut self, arm: &Armature, forked: bool) -> usize {
        #[derive(PartialEq)]
        enum Point {
            Skip,
            Tip,
            Root,
        }
        let mut forked_iter = forked;
        // Root selection of a connected bone whose parent tip is selected is the parent's.
        for i in 0..self.edit_bones.len() {
            if self.is_visible(arm, i)
                && self.edit_bones[i].select[ROOT]
                && let Some(p) = self.edit_bones[i]
                    .parent
                    .filter(|_| self.edit_bones[i].use_connect)
                && self.edit_bones[p].select[TIP]
            {
                self.edit_bones[i].select[ROOT] = false;
            }
        }
        let first_new = self.edit_bones.len();
        let mut totbone = 0usize;
        let mut first: Option<usize> = None;
        let mut newbone: Option<usize> = None;
        let mut i = 0;
        while i < first_new {
            if !self.is_visible(arm, i) {
                i += 1;
                continue;
            }
            let sel = self.edit_bones[i].select;
            let mut point = Point::Skip;
            if sel[TIP] || sel[SEL] {
                point = Point::Tip;
            } else if sel[ROOT] {
                let parent_tip = self.edit_bones[i]
                    .parent
                    .is_some_and(|p| self.edit_bones[p].select[TIP]);
                if !parent_tip {
                    point = Point::Root;
                }
            }
            if point != Point::Skip {
                let mut flipbone: Option<usize> = None;
                if arm.use_mirror_x {
                    flipbone = self.mirrored(i);
                    if let Some(f) = flipbone {
                        forked_iter = false;
                        let fs = &mut self.edit_bones[f].select;
                        if fs.iter().any(|s| *s) {
                            *fs = [false; 3];
                        }
                    }
                    if flipbone.is_none() && forked_iter {
                        flipbone = Some(i);
                    }
                }
                for a in 0..2 {
                    let src = if a == 1 {
                        match flipbone {
                            Some(f) => f,
                            None => break,
                        }
                    } else {
                        i
                    };
                    totbone += 1;
                    let e = self.edit_bones[src].clone();
                    let mut nb = EditBone::new("");
                    nb.select = [false; 3];
                    if point == Point::Tip {
                        nb.head = e.tail;
                        nb.tail = e.tail;
                        nb.parent = Some(src);
                        nb.select[TIP] = e.select[TIP];
                        nb.use_relative_parent = e.use_relative_parent;
                        nb.use_connect = true;
                    } else {
                        nb.head = e.head;
                        nb.tail = e.head;
                        nb.parent = e.parent;
                        nb.select[TIP] = true;
                        nb.use_connect = e.parent.is_some() && e.use_connect;
                    }
                    nb.envelope_weight = e.envelope_weight;
                    nb.envelope_distance = e.envelope_distance;
                    nb.head_radius = e.tail_radius;
                    nb.tail_radius = e.tail_radius;
                    nb.bbone_segments = 1;
                    // The new bone's other flags are cleared (`newbone->flag` holds only the
                    // tip selection, relative parenting and connection), so it deforms,
                    // inherits rotation and uses local location.
                    nb.use_deform = true;
                    nb.use_inherit_rotation = true;
                    nb.use_local_location = true;
                    nb.inherit_scale = "FULL".into();
                    let mut name = e.name.to_string();
                    if flipbone.is_some()
                        && forked_iter
                        && name.len() < super::names::MAX_BONE_NAME - 1
                    {
                        name.push_str(if a == 0 { "_L" } else { "_R" });
                    }
                    nb.name = self.unique_name(&name, None);
                    nb.collections = e.collections.clone();
                    self.edit_bones.push(nb);
                    let idx = self.edit_bones.len() - 1;
                    if first.is_none() {
                        first = Some(idx);
                    }
                    newbone = Some(idx);
                }
            }
            self.edit_bones[i].select = [false; 3];
            i += 1;
        }
        if totbone == 1 && first.is_some() {
            self.active = first;
        } else {
            self.active = newbone;
        }
        if totbone > 0 {
            self.sync_selection();
        }
        totbone
    }

    /// `armature_duplicate_selected` for this armature: `(source, copy)` pairs, `None` when
    /// nothing was selected (the operator's `CTX_DATA_COUNT(selected_bones) == 0` test is the
    /// caller's).
    pub fn duplicate_selected(
        &mut self,
        arm: &Armature,
        do_flip_names: bool,
    ) -> Vec<(usize, usize)> {
        self.sync_selection();
        if arm.use_mirror_x {
            for i in 0..self.edit_bones.len() {
                if self.is_selected(arm, i)
                    && let Some(m) = self.mirrored(i)
                {
                    self.edit_bones[m].select[SEL] = true;
                }
            }
        }
        let n = self.edit_bones.len();
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        let mut temp: Vec<Option<usize>> = vec![None; n];
        for i in 0..n {
            if !self.is_selected(arm, i) {
                continue;
            }
            let src = self.edit_bones[i].clone();
            let mut name = src.name.to_string();
            if do_flip_names {
                let flip = flip_side_name(&src.name, false);
                if self.find(&flip).is_none() {
                    name = flip;
                }
            }
            let mut copy = src;
            copy.name = self.unique_name(&name, None);
            self.edit_bones.push(copy);
            let j = self.edit_bones.len() - 1;
            temp[i] = Some(j);
            pairs.push((i, j));
        }
        for &(i, j) in &pairs {
            match self.edit_bones[i].parent {
                None => self.edit_bones[j].parent = None,
                Some(p) => match temp[p] {
                    Some(pd) => self.edit_bones[j].parent = Some(pd),
                    None => {
                        self.edit_bones[j].parent = Some(p);
                        self.edit_bones[j].use_connect = false;
                    }
                },
            }
        }
        if let Some(a) = self.active
            && let Some(Some(d)) = temp.get(a)
        {
            self.active = Some(*d);
        }
        for i in 0..n {
            if self.is_visible(arm, i) {
                self.edit_bones[i].select = [false; 3];
            }
        }
        pairs
    }

    /// `armature_subdivide_exec` for the bones `bones` (`context.selected_editable_bones` of
    /// this armature, taken before cutting): each is cut into `cuts + 1` connected bones.
    pub fn subdivide(&mut self, bones: &[usize], cuts: i32) {
        for &b in bones {
            let last_before = self.edit_bones.len();
            let mut i = cuts + 1;
            while i > 1 {
                let cutratio = 1.0 / i as f32;
                let cutratio_i = 1.0 - cutratio;
                let e = self.edit_bones[b].clone();
                let mut nb = e.clone();
                let (v1, v2) = (e.head, e.tail);
                let v3 = [
                    v1[0] * cutratio + v2[0] * cutratio_i,
                    v1[1] * cutratio + v2[1] * cutratio_i,
                    v1[2] * cutratio + v2[2] * cutratio_i,
                ];
                nb.head = v3;
                nb.tail = e.tail;
                nb.head_radius = e.head_radius * cutratio + e.tail_radius * cutratio_i;
                nb.use_connect = true;
                self.edit_bones.push(nb);
                let ni = self.edit_bones.len() - 1;
                self.edit_bones[b].tail = v3;
                self.edit_bones[b].tail_radius = self.edit_bones[ni].head_radius;
                for t in 0..self.edit_bones.len() {
                    if self.edit_bones[t].parent == Some(b) {
                        self.edit_bones[t].parent = Some(ni);
                    }
                }
                self.edit_bones[ni].parent = Some(b);
                i -= 1;
            }
            for k in (last_before..self.edit_bones.len()).rev() {
                let name = self.edit_bones[k].name.clone();
                self.edit_bones[k].name = self.unique_name(&name, Some(k));
            }
        }
    }

    /// The bones `armature.delete` removes (visible and selected after the mirror selection),
    /// in list order.
    pub fn bones_to_delete(&mut self, arm: &Armature) -> Vec<usize> {
        self.select_mirrored(arm, [true, false, false]);
        (0..self.edit_bones.len())
            .filter(|&i| self.is_selected(arm, i))
            .collect()
    }

    /// Removes `bones` (`ED_armature_ebone_remove`, children re-parented and disconnected),
    /// then `ED_armature_edit_sync_selection`.
    pub fn delete_bones(&mut self, bones: &[usize]) {
        let mut sorted = bones.to_vec();
        sorted.sort_unstable();
        for &i in sorted.iter().rev() {
            self.remove_bone(i, true);
        }
        self.sync_selection();
    }

    /// `chains_find_tips`: one entry per chain, its tip bone.
    fn chains_find_tips(&self) -> Vec<usize> {
        let mut list: Vec<usize> = Vec::new();
        for cur in 0..self.edit_bones.len() {
            // Contained in an existing chain?
            let in_chain = list.iter().any(|&tip| {
                let mut e = Some(tip);
                while let Some(x) = e {
                    if x == cur {
                        return true;
                    }
                    e = self.edit_bones[x].parent;
                }
                false
            });
            if in_chain {
                continue;
            }
            // Is an existing chain part of this bone's chain?
            let mut replaced = false;
            let mut e = self.edit_bones[cur].parent;
            'outer: while let Some(x) = e {
                for tip in list.iter_mut() {
                    if *tip == x {
                        *tip = cur;
                        replaced = true;
                        break 'outer;
                    }
                }
                e = self.edit_bones[x].parent;
            }
            if replaced {
                continue;
            }
            list.push(cur);
        }
        list
    }

    /// `armature_switch_direction_exec` for this armature.
    pub fn switch_direction(&mut self, arm: &Armature) {
        let chains = self.chains_find_tips();
        if chains.is_empty() {
            return;
        }
        let tagged = self.tag_select_mirrored(arm);
        let mut done = vec![false; self.edit_bones.len()];
        for tip in chains {
            let mut child: Option<usize> = None;
            let mut cur = Some(tip);
            while let Some(e) = cur {
                let parent = self.edit_bones[e].parent;
                if !done[e] {
                    if self.is_visible(arm, e) && self.is_editable(e) {
                        let b = &mut self.edit_bones[e];
                        std::mem::swap(&mut b.head, &mut b.tail);
                        b.parent = child;
                        let connect = child
                            .is_some_and(|c| self.edit_bones[e].head == self.edit_bones[c].tail);
                        self.edit_bones[e].use_connect = connect;
                        child = Some(e);
                    } else {
                        if let Some(p) = parent
                            && self.is_visible(arm, p)
                            && self.is_editable(p)
                        {
                            self.edit_bones[e].parent = None;
                            self.edit_bones[e].use_connect = false;
                        }
                        child = None;
                    }
                    done[e] = true;
                }
                cur = parent;
            }
        }
        self.tag_unselect(&tagged);
    }

    /// `armature_parent_set_exec` (`connected` = `type='CONNECTED'`); `Err` when there is no
    /// active bone (`Operation requires an active bone`).
    pub fn parent_set(&mut self, arm: &Armature, connected: bool) -> Result<(), &'static str> {
        let Some(act) = self.active.filter(|&a| a < self.edit_bones.len()) else {
            return Err("Operation requires an active bone");
        };
        let actmirb = if arm.use_mirror_x {
            Some(self.mirrored(act).unwrap_or(act))
        } else {
            None
        };
        let mut only_active = false;
        if self.edit_bones[act].select[SEL] {
            only_active = (0..self.edit_bones.len()).all(|i| !self.is_editable(i) || i == act);
        }
        if only_active {
            if self.edit_bones[act].parent.is_some() {
                self.connect_to_existing_parent(act);
                if let Some(m) = actmirb
                    && self.edit_bones[m].parent.is_some()
                {
                    self.connect_to_existing_parent(m);
                }
            }
            return Ok(());
        }
        for i in 0..self.edit_bones.len() {
            if !self.is_editable(i) {
                continue;
            }
            if i != act {
                self.connect_to_new_parent(i, act, connected);
            }
            if let Some(am) = actmirb
                && let Some(m) = self.mirrored(i)
                && !self.edit_bones[m].select[SEL]
                && m != am
            {
                self.connect_to_new_parent(m, am, connected);
            }
        }
        Ok(())
    }

    /// `bone_connect_to_existing_parent`.
    fn connect_to_existing_parent(&mut self, i: usize) {
        let p = self.edit_bones[i].parent.expect("checked by the caller");
        let (tail, rad) = (self.edit_bones[p].tail, self.edit_bones[p].tail_radius);
        let b = &mut self.edit_bones[i];
        b.use_connect = true;
        b.head = tail;
        b.head_radius = rad;
    }

    /// `bone_connect_to_new_parent`.
    fn connect_to_new_parent(&mut self, sel: usize, act: usize, connected: bool) {
        if let Some(p) = self.edit_bones[sel]
            .parent
            .filter(|_| self.edit_bones[sel].use_connect)
        {
            self.edit_bones[p].select[TIP] = false;
        }
        self.edit_bones[sel].parent = Some(act);
        let mut e = self.edit_bones[act].parent;
        let mut guard = 0;
        while let Some(x) = e {
            if self.edit_bones[x].parent == Some(sel) {
                self.edit_bones[x].parent = None;
                self.edit_bones[x].use_connect = false;
            }
            e = self.edit_bones[x].parent;
            guard += 1;
            if guard > self.edit_bones.len() {
                break;
            }
        }
        if connected {
            self.edit_bones[sel].use_connect = true;
            let offset = self.edit_bones[act].tail_v() - self.edit_bones[sel].head_v();
            self.edit_bones[sel].head = self.edit_bones[act].tail;
            self.edit_bones[sel].head_radius = self.edit_bones[act].tail_radius;
            self.edit_bones[sel].tail = (self.edit_bones[sel].tail_v() + offset).to_array();
            for j in 0..self.edit_bones.len() {
                if self.is_child_recursive(sel, j) {
                    let b = &mut self.edit_bones[j];
                    b.head = (b.head_v() + offset).to_array();
                    b.tail = (b.tail_v() + offset).to_array();
                }
            }
        } else {
            self.edit_bones[sel].use_connect = false;
        }
    }

    /// `armature_parent_clear_exec` on `bones` (`selected_editable_bones`, taken first):
    /// `clear` = `type='CLEAR'` removes the parent, `DISCONNECT` only the connection.
    pub fn parent_clear(&mut self, bones: &[usize], clear: bool) {
        for &i in bones {
            if let Some(p) = self.edit_bones[i].parent {
                self.edit_bones[p].select[TIP] = false;
            }
            if clear {
                self.edit_bones[i].parent = None;
            }
            self.edit_bones[i].use_connect = false;
        }
    }

    /// Whether any bone is `EBONE_EDITABLE` (the `parent_clear` change test).
    pub fn any_editable(&self) -> bool {
        (0..self.edit_bones.len()).any(|i| self.is_editable(i))
    }

    /// `armature_roll_clear_exec` for this armature.
    pub fn roll_clear(&mut self, arm: &Armature, roll: f32) -> bool {
        let mut changed = false;
        for i in 0..self.edit_bones.len() {
            if self.is_visible(arm, i) && self.is_editable(i) {
                self.edit_bones[i].roll = roll;
                changed = true;
            }
        }
        if self.mirror_rolls(arm) {
            changed = true;
        }
        changed
    }

    /// The `ARM_MIRROR_EDIT` roll pass of the roll operators: unedited bones take the negated
    /// roll of an edited mirror.
    fn mirror_rolls(&mut self, arm: &Armature) -> bool {
        if !arm.use_mirror_x {
            return false;
        }
        let mut changed = false;
        for i in 0..self.edit_bones.len() {
            if self.is_visible(arm, i) && self.is_editable(i) {
                continue;
            }
            if let Some(m) = self.mirrored(i)
                && self.is_visible(arm, m)
                && self.is_editable(m)
            {
                self.edit_bones[i].roll = -self.edit_bones[m].roll;
                changed = true;
            }
        }
        changed
    }

    /// `armature_calc_roll_exec` for this armature. `ob_mat3` is the object's world 3×3,
    /// `cursor_local` the 3D cursor in object space, `view_z` the view's Z axis (world),
    /// `active_z` the Z axis of the active edit bone of the active armature.
    #[allow(clippy::too_many_arguments)]
    pub fn calc_roll(
        &mut self,
        arm: &Armature,
        ty: &str,
        axis_flip: bool,
        axis_only: bool,
        ob_mat3: &Mat3,
        cursor_local: Vec3,
        view_z: Vec3,
        active_z: Option<Vec3>,
    ) -> Result<(), &'static str> {
        let neg = ty.starts_with("NEG_") || ty.starts_with("GLOBAL_NEG_");
        let axis_flip = if matches!(ty, "ACTIVE" | "VIEW" | "CURSOR") {
            axis_flip
        } else {
            neg
        };
        let imat = super::math::invert_m3(ob_mat3);
        let editable = |ea: &EditArmature, i: usize| ea.is_visible(arm, i) && ea.is_editable(i);
        match ty {
            "CURSOR" => {
                for i in 0..self.edit_bones.len() {
                    if !editable(self, i) {
                        continue;
                    }
                    let mut rel = cursor_local - self.edit_bones[i].head_v();
                    if axis_flip {
                        rel = -rel;
                    }
                    let (n, len) = normalize(rel);
                    if len != 0.0 {
                        self.edit_bones[i].roll = self.roll_to_vector(i, n, axis_only);
                    }
                }
            }
            "POS_X" | "POS_Z" | "NEG_X" | "NEG_Z" => {
                let use_z = ty.ends_with('Z');
                for i in 0..self.edit_bones.len() {
                    let Some(par) = self.edit_bones[i].parent else {
                        continue;
                    };
                    let is_edit = editable(self, i);
                    let is_edit_parent = editable(self, par);
                    if !(is_edit || is_edit_parent) {
                        continue;
                    }
                    let dir_a =
                        normalize(self.edit_bones[i].tail_v() - self.edit_bones[i].head_v()).0;
                    let mut other = Some(par);
                    let mut vec = Vec3::ZERO;
                    let mut is_zero = true;
                    while let Some(o) = other {
                        let dir_b =
                            normalize(self.edit_bones[o].head_v() - self.edit_bones[o].tail_v()).0;
                        vec = if use_z {
                            dir_a.cross(dir_b)
                        } else {
                            dir_a + dir_b
                        };
                        let (n, len) = normalize(vec);
                        vec = n;
                        is_zero = len < 0.00001;
                        if !is_zero {
                            break;
                        }
                        other = self.edit_bones[o].parent;
                    }
                    if is_zero {
                        continue;
                    }
                    if axis_flip {
                        vec = -vec;
                    }
                    if is_edit {
                        self.edit_bones[i].roll = self.roll_to_vector(i, vec, axis_only);
                    }
                    if is_edit_parent && self.edit_bones[par].parent.is_none() {
                        self.edit_bones[par].roll = self.roll_to_vector(par, vec, axis_only);
                    }
                }
            }
            _ => {
                let mut vec = match ty {
                    "VIEW" => imat * view_z,
                    "ACTIVE" => match active_z {
                        Some(z) => z,
                        None => return Err("No active bone set"),
                    },
                    _ => {
                        let axis = match ty {
                            "GLOBAL_POS_X" | "GLOBAL_NEG_X" => 0,
                            "GLOBAL_POS_Y" | "GLOBAL_NEG_Y" => 1,
                            _ => 2,
                        };
                        let mut v = Vec3::ZERO;
                        // GLOBAL_NEG_* run as the positive axis with `axis_flip`.
                        v[axis] = 1.0;
                        normalize(imat * v).0
                    }
                };
                if axis_flip {
                    vec = -vec;
                }
                for i in 0..self.edit_bones.len() {
                    if editable(self, i) {
                        self.edit_bones[i].roll = self.roll_to_vector(i, vec, axis_only);
                    }
                }
            }
        }
        self.mirror_rolls(arm);
        Ok(())
    }

    /// `armature_symmetrize_exec` for this armature (`direction` -1 = `NEGATIVE_X`, +1 =
    /// `POSITIVE_X`): `(source, mirror)` pairs whose mirror is new (`true`) or existing.
    /// `collections` (the armature's, edited in place) gains the mirrored bone collections
    /// the new assignments need.
    pub fn symmetrize(
        &mut self,
        arm: &Armature,
        collections: &mut Vec<crate::id::BoneCollection>,
        direction: i32,
    ) -> Vec<(usize, usize, bool)> {
        const AXIS: usize = 0;
        self.sync_selection();
        let n = self.edit_bones.len();
        let mut temp: Vec<Option<usize>> = vec![None; n];
        for i in 0..n {
            if !(self.is_visible(arm, i) && self.edit_bones[i].select[SEL]) {
                continue;
            }
            let name = self.edit_bones[i].name.clone();
            let flip = flip_side_name(&name, false);
            if flip == name {
                self.edit_bones[i].select = [false; 3];
                continue;
            }
            let Some(m) = self.find(&flip) else {
                continue;
            };
            if self.edit_bones[m].select[SEL] {
                let mut d = self.edit_bones[m].head[AXIS] - self.edit_bones[i].head[AXIS];
                if d == 0.0 {
                    d = self.edit_bones[m].tail[AXIS] - self.edit_bones[i].tail[AXIS];
                    if d == 0.0 {
                        self.edit_bones[m].select = [false; 3];
                        self.edit_bones[i].select = [false; 3];
                        continue;
                    }
                }
                if (if d < 0.0 { -1 } else { 1 }) == direction {
                    self.edit_bones[i].select = [false; 3];
                    continue;
                }
                self.edit_bones[m].select = [false; 3];
            }
            temp[i] = Some(m);
        }
        let mut out: Vec<(usize, usize, bool)> = Vec::new();
        for i in 0..n {
            if !self.is_selected(arm, i) {
                continue;
            }
            if let Some(m) = temp[i] {
                self.edit_bones[m].inherit_scale = self.edit_bones[i].inherit_scale.clone();
                out.push((i, m, false));
                continue;
            }
            let flip = flip_side_name(&self.edit_bones[i].name, false);
            if flip != self.edit_bones[i].name {
                let mut copy = self.edit_bones[i].clone();
                copy.name = self.unique_name(&flip, None);
                self.edit_bones.push(copy);
                let j = self.edit_bones.len() - 1;
                temp[i] = Some(j);
                out.push((i, j, true));
            }
        }
        let temp_of =
            |ea: &EditArmature, temp: &[Option<usize>], b: Option<usize>| -> Option<usize> {
                let b = b?;
                if let Some(Some(t)) = temp.get(b) {
                    return Some(*t);
                }
                Some(ea.mirrored(b).unwrap_or(b))
            };
        for i in 0..n {
            let Some(m) = temp[i] else {
                continue;
            };
            let src = self.edit_bones[i].clone();
            {
                // Copy every flag except the selection.
                let d = &mut self.edit_bones[m];
                d.use_connect = src.use_connect;
                d.use_deform = src.use_deform;
                d.use_inherit_rotation = src.use_inherit_rotation;
                d.use_local_location = src.use_local_location;
                d.use_relative_parent = src.use_relative_parent;
                d.use_envelope_multiply = src.use_envelope_multiply;
                d.hide = src.hide;
                d.hide_select = src.hide_select;
                d.lock = src.lock;
            }
            match src.parent {
                None => {
                    self.edit_bones[m].parent = None;
                    self.edit_bones[m].use_connect = false;
                }
                Some(p) => {
                    let mp = temp_of(self, &temp, Some(p));
                    if mp == Some(p) && self.edit_bones[m].head[AXIS] != 0.0 {
                        self.edit_bones[m].use_connect = false;
                    }
                    self.edit_bones[m].parent = mp;
                }
            }
            // `mirror_bone_collection_assignments`: flippable collections of the source are
            // swapped for their mirrors (created next to them when missing).
            let mut unassign: Vec<smol_str::SmolStr> = Vec::new();
            let mut assign: Vec<smol_str::SmolStr> = Vec::new();
            for c in &src.collections {
                let f = flip_side_name(c, false);
                if f == c.as_str() {
                    continue;
                }
                if !collections.iter().any(|x| x.name == f) {
                    let parent = collections
                        .iter()
                        .position(|x| x.name == *c)
                        .and_then(|ci| collections[ci].parent);
                    collections.push(crate::id::BoneCollection {
                        name: f.as_str().into(),
                        parent,
                        is_visible: true,
                        is_solo: false,
                    });
                }
                unassign.push(c.clone());
                assign.push(f.into());
            }
            let target = &mut self.edit_bones[m].collections;
            target.retain(|c| !unassign.contains(c));
            for a in assign {
                if !target.contains(&a) {
                    target.push(a);
                }
            }
        }
        self.transform_mirror_update_all(arm);
        for i in 0..n {
            if self.is_visible(arm, i) {
                self.edit_bones[i].select = [false; 3];
            }
        }
        for i in 0..n {
            if let Some(m) = temp[i]
                && self.is_selectable(arm, m)
            {
                self.select_set(m, true);
            }
        }
        if let Some(a) = self.active
            && let Some(Some(t)) = temp.get(a)
        {
            self.active = Some(*t);
        }
        out
    }
}

/// `ED_armature_ebone_roll_to_vector` for one edit bone.
pub fn roll_to_vector(bone: &EditBone, align_axis: Vec3, axis_only: bool) -> f32 {
    let (nor, len) = normalize(bone.tail_v() - bone.head_v());
    if len <= f32::EPSILON || align_axis.dot(nor).abs() >= 1.0 - f32::EPSILON {
        return 0.0;
    }
    let mat = vec_roll_to_mat3_normalized(nor, 0.0);
    let vec = project_normalized(align_axis, nor);
    let mut proj = align_axis - vec;
    if axis_only && angle_v3v3(proj, mat.z_axis) > std::f32::consts::FRAC_PI_2 {
        proj = -proj;
    }
    let roll = angle_v3v3(proj, mat.z_axis);
    let vec = mat.z_axis.cross(proj);
    if vec.dot(nor) < 0.0 { -roll } else { roll }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> (Armature, EditArmature) {
        let arm = Armature::new("A");
        let mut ea = EditArmature::default();
        let a = ea.add_bone("Bone");
        ea.edit_bones[a].tail = [0.0, 0.0, 1.0];
        (arm, ea)
    }

    #[test]
    fn extrude_tip_makes_a_connected_child() {
        let (arm, mut ea) = chain();
        assert_eq!(ea.extrude(&arm, false), 1);
        assert_eq!(ea.edit_bones.len(), 2);
        let nb = &ea.edit_bones[1];
        assert_eq!(nb.name, "Bone.001");
        assert_eq!(nb.parent, Some(0));
        assert!(nb.use_connect);
        assert_eq!(nb.head, [0.0, 0.0, 1.0]);
        assert_eq!(ea.active, Some(1));
        // Original deselected; the new tip selected, its root follows the parent's tip.
        assert_eq!(ea.edit_bones[0].select, [false; 3]);
        assert_eq!(ea.edit_bones[1].select, [false, false, true]);
    }

    #[test]
    fn subdivide_names_in_order() {
        let (_arm, mut ea) = chain();
        ea.subdivide(&[0], 2);
        let names: Vec<&str> = ea.edit_bones.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Bone", "Bone.002", "Bone.001"]);
        assert!((ea.edit_bones[0].tail[2] - 1.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn roll_to_vector_aligns_z() {
        let mut b = EditBone::new("B");
        b.tail = [0.0, 0.0, 1.0];
        // Bone along +Z: its Z axis is -Y; aligning to +X needs a quarter turn.
        let r = roll_to_vector(&b, Vec3::X, false);
        assert!((r.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-6, "{r}");
    }

    /// `Bone` (0,0,0)→(0,0,1) with a connected child (0,0,1)→(0,1,1), both selected.
    fn two_bone_chain() -> (Armature, EditArmature) {
        let (arm, mut ea) = chain();
        let b = ea.add_bone("Bone");
        ea.edit_bones[b].head = [0.0, 0.0, 1.0];
        ea.edit_bones[b].tail = [0.0, 1.0, 1.0];
        ea.edit_bones[b].parent = Some(0);
        ea.edit_bones[b].use_connect = true;
        for e in &mut ea.edit_bones {
            e.select = [true; 3];
        }
        (arm, ea)
    }

    #[test]
    fn duplicate_copies_selection_and_parents() {
        let (arm, mut ea) = two_bone_chain();
        ea.active = Some(1);
        let pairs = ea.duplicate_selected(&arm, false);
        assert_eq!(pairs, vec![(0, 2), (1, 3)]);
        assert_eq!(ea.edit_bones[2].name, "Bone.002");
        assert_eq!(ea.edit_bones[3].name, "Bone.003");
        // The child copy is parented to the parent copy and stays connected.
        assert_eq!(ea.edit_bones[3].parent, Some(2));
        assert!(ea.edit_bones[3].use_connect);
        assert_eq!(ea.active, Some(3));
        assert_eq!(ea.edit_bones[0].select, [false; 3]);
        assert_eq!(ea.edit_bones[2].select, [true; 3]);
    }

    #[test]
    fn delete_reparents_children() {
        let (arm, mut ea) = two_bone_chain();
        ea.edit_bones[1].select = [false; 3];
        let doomed = ea.bones_to_delete(&arm);
        assert_eq!(doomed, vec![0]);
        ea.delete_bones(&doomed);
        assert_eq!(ea.edit_bones.len(), 1);
        assert_eq!(ea.edit_bones[0].parent, None);
        assert!(!ea.edit_bones[0].use_connect);
    }

    #[test]
    fn switch_direction_reverses_the_chain() {
        let (arm, mut ea) = two_bone_chain();
        ea.switch_direction(&arm);
        assert_eq!(ea.edit_bones[1].head, [0.0, 1.0, 1.0]);
        assert_eq!(ea.edit_bones[1].parent, None);
        assert_eq!(ea.edit_bones[0].head, [0.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[0].parent, Some(1));
        assert!(ea.edit_bones[0].use_connect);
    }

    #[test]
    fn parent_set_connects_and_moves_children() {
        let (arm, mut ea) = chain();
        let b = ea.add_bone("Arm");
        ea.edit_bones[b].head = [1.0, 0.0, 0.0];
        ea.edit_bones[b].tail = [2.0, 0.0, 0.0];
        ea.edit_bones[b].select = [true; 3];
        ea.edit_bones[0].select = [true; 3];
        ea.active = Some(0);
        ea.parent_set(&arm, true).expect("active bone");
        assert_eq!(ea.edit_bones[b].parent, Some(0));
        assert_eq!(ea.edit_bones[b].head, [0.0, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[b].tail, [1.0, 0.0, 1.0]);
        ea.active = None;
        assert!(ea.parent_set(&arm, true).is_err());
    }

    #[test]
    fn symmetrize_mirrors_side_bones() {
        let (arm, mut ea) = chain();
        let l = ea.add_bone("arm.L");
        ea.edit_bones[l].head = [0.5, 0.0, 1.0];
        ea.edit_bones[l].tail = [1.5, 0.2, 1.0];
        ea.edit_bones[l].roll = 0.4;
        ea.edit_bones[l].parent = Some(0);
        ea.edit_bones[l].select = [true; 3];
        let mut colls = arm.collections.clone();
        let out = ea.symmetrize(&arm, &mut colls, -1);
        assert_eq!(out.len(), 1);
        let r = ea.find("arm.R").expect("mirror");
        assert_eq!(ea.edit_bones[r].head, [-0.5, 0.0, 1.0]);
        assert_eq!(ea.edit_bones[r].tail, [-1.5, 0.2, 1.0]);
        assert_eq!(ea.edit_bones[r].roll, -0.4);
        assert_eq!(ea.edit_bones[r].parent, Some(0));
        assert_eq!(ea.edit_bones[r].select, [true; 3]);
        assert_eq!(ea.edit_bones[l].select, [false; 3]);
    }

    #[test]
    fn select_all_toggle_and_invert() {
        let (arm, mut ea) = two_bone_chain();
        assert!(ea.any_visible_selected(&arm));
        ea.select_all(&arm, SelectAction::Deselect);
        assert!(!ea.any_visible_selected(&arm));
        ea.edit_bones[0].select = [true; 3];
        ea.select_all(&arm, SelectAction::Invert);
        // The parent is deselected, then selecting its connected child selects its tip.
        assert_eq!(ea.edit_bones[0].select, [false, false, true]);
        assert_eq!(ea.edit_bones[1].select, [true; 3]);
    }

    #[test]
    fn calc_roll_global_axis() {
        let (arm, mut ea) = chain();
        ea.edit_bones[0].select = [true; 3];
        ea.calc_roll(
            &arm,
            "GLOBAL_POS_X",
            false,
            false,
            &Mat3::IDENTITY,
            Vec3::ZERO,
            Vec3::Z,
            None,
        )
        .expect("roll");
        assert!((ea.edit_bones[0].roll - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(
            ea.calc_roll(
                &arm,
                "ACTIVE",
                false,
                false,
                &Mat3::IDENTITY,
                Vec3::ZERO,
                Vec3::Z,
                None
            )
            .is_err()
        );
    }
}
