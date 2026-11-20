// SPDX-License-Identifier: GPL-3.0-or-later
//! Armatures in Edit Mode: edit bones (`Armature.edit_bones`, `EditBone`), created from the
//! rest bones on entering Edit Mode and written back (`ED_armature_from_edit`: rest matrices
//! from head, tail and roll) on leaving it. M5+M6 W0 contract; WP 6.2 owns the file.
//!
//! WP 6.2 adds the bone maths every armature crate shares ([`math`], [`rest`], [`posing`]),
//! the edit-bone tools the `armature.*` operators run ([`tools`]) and Blender's bone naming
//! rules ([`names`]).
//!
//! Ported from blender/source/blender/editors/armature/armature_utils.cc (`make_boneList`,
//! `ED_armature_from_edit`, `ED_armature_edit_sync_selection`,
//! `ED_armature_ebone_selectflag_set`, `ED_armature_ebone_remove_ex`,
//! `ED_armature_ebone_to_mat3/4`, `ED_armature_ebone_from_mat3/4`) and
//! blender/source/blender/editors/armature/armature_add.cc (`ED_armature_ebone_add`), Blender
//! 5.2.4, GPL-2.0-or-later.

pub mod ctx;
pub mod family;
pub mod math;
pub mod names;
pub mod paths;
pub mod posing;
pub mod rename;
pub mod rest;
pub mod tools;

use glam::{Mat3, Mat4, Vec3};
use smol_str::SmolStr;

use crate::id::{Armature, Bone};

/// Armature in Edit Mode (`EditState::armatures`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EditArmature {
    /// `Armature.edit_bones`, in creation order.
    pub edit_bones: Vec<EditBone>,
    /// `edit_bones.active` (index)
    pub active: Option<usize>,
}

/// `bpy.types.EditBone`.
#[derive(Clone, Debug, PartialEq)]
pub struct EditBone {
    /// `name`
    pub name: SmolStr,
    /// `parent` (index into `EditArmature::edit_bones`)
    pub parent: Option<usize>,
    /// `head` (armature space)
    pub head: [f32; 3],
    /// `tail` (armature space)
    pub tail: [f32; 3],
    /// `roll`
    pub roll: f32,
    /// `use_connect`
    pub use_connect: bool,
    /// `use_deform`
    pub use_deform: bool,
    /// `use_inherit_rotation`
    pub use_inherit_rotation: bool,
    /// `inherit_scale`
    pub inherit_scale: SmolStr,
    /// `use_local_location`
    pub use_local_location: bool,
    /// `envelope_distance`
    pub envelope_distance: f32,
    /// `envelope_weight`
    pub envelope_weight: f32,
    /// `head_radius`
    pub head_radius: f32,
    /// `tail_radius`
    pub tail_radius: f32,
    /// `select` / `select_head` / `select_tail`
    pub select: [bool; 3],
    /// `hide`
    pub hide: bool,
    /// `hide_select`
    pub hide_select: bool,
    /// `lock`
    pub lock: bool,
    /// Names of the bone collections the bone is assigned to (`EditBone.collections`).
    pub collections: Vec<SmolStr>,
    /// `use_relative_parent` (WP 6.2)
    pub use_relative_parent: bool,
    /// `use_envelope_multiply` (WP 6.2)
    pub use_envelope_multiply: bool,
    /// `bbone_segments` (WP 6.2)
    pub bbone_segments: i32,
    /// The `EditBone::length` DNA field (not RNA: `EditBone.length` reads the head-tail
    /// distance): the rest length on entering Edit Mode, 0 for new bones, updated by
    /// transforms, which reset the radii of bones still at 0 (`recalcData_edit_armature`).
    pub length_stored: f32,
}

/// Index of `select` for `BONE_SELECTED`.
pub const SEL: usize = 0;
/// Index of `select` for `BONE_ROOTSEL` (`select_head`).
pub const ROOT: usize = 1;
/// Index of `select` for `BONE_TIPSEL` (`select_tail`).
pub const TIP: usize = 2;

impl EditBone {
    /// `ED_armature_ebone_add`'s bone: tail selected, deforming, envelope 0.25, radii 0.1 /
    /// 0.05, one segment, head and tail at the origin.
    pub fn new(name: impl Into<SmolStr>) -> EditBone {
        EditBone {
            name: name.into(),
            parent: None,
            head: [0.0; 3],
            tail: [0.0; 3],
            roll: 0.0,
            use_connect: false,
            use_deform: true,
            use_inherit_rotation: true,
            inherit_scale: "FULL".into(),
            use_local_location: true,
            envelope_distance: 0.25,
            envelope_weight: 1.0,
            head_radius: 0.1,
            tail_radius: 0.05,
            select: [false, false, true],
            hide: false,
            hide_select: false,
            lock: false,
            collections: Vec::new(),
            use_relative_parent: false,
            use_envelope_multiply: false,
            bbone_segments: 1,
            length_stored: 0.0,
        }
    }

    /// `head` as a vector.
    pub fn head_v(&self) -> Vec3 {
        Vec3::from_array(self.head)
    }

    /// `tail` as a vector.
    pub fn tail_v(&self) -> Vec3 {
        Vec3::from_array(self.tail)
    }

    /// `length` (`len_v3v3(head, tail)`).
    pub fn length(&self) -> f32 {
        (self.tail_v() - self.head_v()).length()
    }

    /// Any of the three selection flags.
    pub fn any_selected(&self) -> bool {
        self.select.iter().any(|s| *s)
    }
}

impl EditArmature {
    /// `make_boneList` (`ED_armature_to_edit`): one edit bone per rest bone, in bone order,
    /// with Blender's selection fix-ups for connected bones.
    pub fn from_armature(arm: &Armature) -> EditArmature {
        let mut edit_bones: Vec<EditBone> = arm.bones.iter().map(edit_bone_of).collect();
        for i in 0..edit_bones.len() {
            let (parent, connected) = (edit_bones[i].parent, edit_bones[i].use_connect);
            if edit_bones[i].select[SEL] {
                edit_bones[i].select[TIP] = true;
                if let Some(p) = parent.filter(|_| connected) {
                    edit_bones[p].select[TIP] = true;
                }
                edit_bones[i].select[ROOT] = true;
            } else if parent.is_some() && connected {
                edit_bones[i].select[ROOT] = false;
            }
        }
        EditArmature {
            edit_bones,
            active: arm.active_bone.filter(|&a| a < arm.bones.len()),
        }
    }

    /// `ED_armature_from_edit`: rebuilds `arm.bones` (depth-first: roots in edit order, each
    /// followed by its children in edit order) and their rest matrices, and the active bone.
    /// (Almost) zero-length bones are elongated to 2e-6 along their direction (+Z without one)
    /// and their children disconnected, as Blender 5.x does instead of deleting them.
    pub fn apply_to(&self, arm: &mut Armature) {
        let mut ebones = self.edit_bones.clone();
        const THRESHOLD: f32 = 0.000001;
        const THRESHOLD_SQ: f32 = THRESHOLD * THRESHOLD;
        const ADJUSTED: f32 = 2.0 * THRESHOLD;
        for i in 0..ebones.len() {
            let (h, t) = (ebones[i].head_v(), ebones[i].tail_v());
            let len_sq = (t - h).length_squared();
            if len_sq > THRESHOLD_SQ {
                continue;
            }
            let offset = if len_sq == 0.0 {
                Vec3::new(0.0, 0.0, ADJUSTED)
            } else {
                let (n, _) = math::normalize(t - h);
                n * ADJUSTED
            };
            ebones[i].tail = (h + offset).to_array();
            for c in ebones.iter_mut() {
                if c.parent == Some(i) {
                    c.use_connect = false;
                }
            }
        }
        let order = depth_first(&ebones);
        let mut new_index = vec![0usize; ebones.len()];
        for (n, &i) in order.iter().enumerate() {
            new_index[i] = n;
        }
        let mut bones: Vec<Bone> = Vec::with_capacity(order.len());
        let mut rest: Vec<rest::RestBone> = Vec::with_capacity(order.len());
        for &i in &order {
            let e = &ebones[i];
            let parent = e.parent.map(|p| new_index[p]);
            let r = rest::finalize_bone(e.head_v(), e.tail_v(), e.roll, parent.map(|p| &rest[p]));
            bones.push(bone_of(e, parent, &r));
            rest.push(r);
        }
        arm.bones = bones;
        arm.active_bone = self
            .active
            .filter(|&a| a < ebones.len())
            .map(|a| new_index[a]);
    }

    /// The edit bone named `name`.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.edit_bones.iter().position(|b| b.name == name)
    }

    /// `ED_armature_ebone_unique_name` (`skip` is the bone being renamed, whose own name does
    /// not count).
    pub fn unique_name(&self, name: &str, skip: Option<usize>) -> SmolStr {
        frap_rna::naming::unique_subname(
            &|n| {
                self.edit_bones
                    .iter()
                    .enumerate()
                    .any(|(i, b)| Some(i) != skip && b.name == n)
            },
            name,
            names::MAX_BONE_NAME,
            "Bone",
        )
    }

    /// `ED_armature_ebone_add`: appends a new bone with a unique name.
    pub fn add_bone(&mut self, name: &str) -> usize {
        let name = self.unique_name(name, None);
        self.edit_bones.push(EditBone::new(name));
        self.edit_bones.len() - 1
    }

    /// `ED_armature_ebone_remove_ex`: children are re-parented to the removed bone's parent
    /// (and disconnected when `clear_connected`); indices above `i` shift down.
    pub fn remove_bone(&mut self, i: usize, clear_connected: bool) {
        let parent = self.edit_bones[i].parent;
        for b in &mut self.edit_bones {
            if b.parent == Some(i) {
                b.parent = parent;
                if clear_connected {
                    b.use_connect = false;
                }
            }
        }
        self.edit_bones.remove(i);
        for b in &mut self.edit_bones {
            if let Some(p) = b.parent.as_mut()
                && *p > i
            {
                *p -= 1;
            }
        }
        self.active = match self.active {
            Some(a) if a == i => None,
            Some(a) if a > i => Some(a - 1),
            a => a,
        };
    }

    /// `ED_armature_edit_sync_selection`: a connected bone's head follows its parent's tail,
    /// and `select` is set exactly when both ends are.
    pub fn sync_selection(&mut self) {
        for i in 0..self.edit_bones.len() {
            if self.edit_bones[i].hide_select {
                continue;
            }
            if self.edit_bones[i].use_connect
                && let Some(p) = self.edit_bones[i].parent
            {
                let tip = self.edit_bones[p].select[TIP];
                self.edit_bones[i].select[ROOT] = tip;
            }
            let b = &mut self.edit_bones[i];
            b.select[SEL] = b.select[TIP] && b.select[ROOT];
        }
    }

    /// `ED_armature_ebone_selectflag_set` with `select` ⇒ all three flags.
    pub fn select_set(&mut self, i: usize, select: bool) {
        self.selectflag_set(i, [select; 3]);
    }

    /// `ED_armature_ebone_selectflag_set`: a connected bone's head selection is its parent's
    /// tail selection.
    pub fn selectflag_set(&mut self, i: usize, flag: [bool; 3]) {
        let b = &self.edit_bones[i];
        if let Some(p) = b.parent.filter(|_| b.use_connect) {
            self.edit_bones[p].select[TIP] = flag[ROOT];
        }
        self.edit_bones[i].select = flag;
    }

    /// `animrig::bone_is_visible` for an edit bone (`BONE_HIDDEN_A` and bone collections).
    pub fn is_visible(&self, arm: &Armature, i: usize) -> bool {
        let b = &self.edit_bones[i];
        !b.hide && collections_visible(arm, &b.collections)
    }

    /// `animrig::bone_is_selected`: visible and `BONE_SELECTED`.
    pub fn is_selected(&self, arm: &Armature, i: usize) -> bool {
        self.edit_bones[i].select[SEL] && self.is_visible(arm, i)
    }

    /// `EBONE_EDITABLE`: selected and not locked.
    pub fn is_editable(&self, i: usize) -> bool {
        let b = &self.edit_bones[i];
        b.select[SEL] && !b.lock
    }

    /// `EBONE_SELECTABLE`: visible and not `hide_select`.
    pub fn is_selectable(&self, arm: &Armature, i: usize) -> bool {
        self.is_visible(arm, i) && !self.edit_bones[i].hide_select
    }

    /// `ED_armature_ebone_get_mirrored`: the bone with the flipped side name.
    pub fn mirrored(&self, i: usize) -> Option<usize> {
        let name = &self.edit_bones[i].name;
        let flip = names::flip_side_name(name, false);
        (flip != *name).then(|| self.find(&flip)).flatten()
    }

    /// `ED_armature_ebone_is_child_recursive`: `child` is below `parent`.
    pub fn is_child_recursive(&self, parent: usize, child: usize) -> bool {
        let mut c = self.edit_bones[child].parent;
        let mut guard = 0;
        while let Some(i) = c {
            if i == parent {
                return true;
            }
            guard += 1;
            if guard > self.edit_bones.len() {
                break;
            }
            c = self.edit_bones[i].parent;
        }
        false
    }

    /// `ED_armature_ebone_to_mat3`.
    pub fn bone_mat3(&self, i: usize) -> Mat3 {
        let b = &self.edit_bones[i];
        let parent = b.parent.map(|p| {
            let pb = &self.edit_bones[p];
            (pb.head_v(), pb.tail_v(), pb.roll)
        });
        rest::ebone_mat3(b.head_v(), b.tail_v(), b.roll, parent)
    }

    /// `ED_armature_ebone_to_mat4` (`EditBone.matrix`).
    pub fn bone_mat4(&self, i: usize) -> Mat4 {
        math::m4_from_m3(&self.bone_mat3(i), self.edit_bones[i].head_v())
    }

    /// `ED_armature_ebone_from_mat4` (the `EditBone.matrix` setter): head at the matrix's
    /// translation, direction and roll from its rotation, length kept.
    pub fn set_bone_mat4(&mut self, i: usize, mat: &Mat4) {
        let b = &mut self.edit_bones[i];
        let mut tail = b.tail_v() - b.head_v();
        let head = math::loc(mat);
        tail += head;
        b.head = head.to_array();
        b.tail = tail.to_array();
        let len = (b.tail_v() - b.head_v()).length();
        let (vec, roll) = math::mat3_to_vec_roll(&math::m3(mat));
        b.tail = (b.head_v() + vec * len).to_array();
        b.roll = roll;
    }

    /// `rna_Armature_editbone_transform_update`: after a head/tail/roll/length/radius change of
    /// bone `i`, a connected parent's tail and connected children's heads follow it.
    pub fn transform_update(&mut self, i: usize) {
        let (head, tail, rad_head, rad_tail, parent, connected) = {
            let b = &self.edit_bones[i];
            (
                b.head,
                b.tail,
                b.head_radius,
                b.tail_radius,
                b.parent,
                b.use_connect,
            )
        };
        if let Some(p) = parent.filter(|_| connected) {
            self.edit_bones[p].tail = head;
            self.edit_bones[p].tail_radius = rad_head;
        }
        for c in &mut self.edit_bones {
            if c.parent == Some(i) && c.use_connect {
                c.head = tail;
                c.head_radius = rad_tail;
            }
        }
    }
}

/// `any_bone_collection_visible` for a list of collection names: no collection is visible
/// unless a collection is soloed (`ARM_BCOLL_SOLO_ACTIVE`), otherwise one of them must be
/// [`collection_visible_effectively`].
pub fn collections_visible(arm: &Armature, names: &[SmolStr]) -> bool {
    let solo_active = arm.collections.iter().any(|c| c.is_solo);
    if names.is_empty() && !solo_active {
        return true;
    }
    names.iter().any(|n| {
        arm.collections
            .iter()
            .position(|c| c.name == *n)
            .is_some_and(|i| collection_visible_effectively(arm, i))
    })
}

/// `ANIM_armature_bonecoll_is_visible_effectively`: with a soloed collection only the solo flag
/// counts, otherwise the collection and all its ancestors must be visible.
pub fn collection_visible_effectively(arm: &Armature, i: usize) -> bool {
    if arm.collections.iter().any(|c| c.is_solo) {
        return arm.collections.get(i).is_some_and(|c| c.is_solo);
    }
    collection_visible_with_ancestors(arm, i)
}

/// `BoneCollection::is_visible_with_ancestors`.
pub fn collection_visible_with_ancestors(arm: &Armature, mut i: usize) -> bool {
    let mut guard = 0;
    loop {
        let Some(c) = arm.collections.get(i) else {
            return false;
        };
        if !c.is_visible {
            return false;
        }
        match c.parent {
            Some(p) if guard < 1024 => {
                i = p;
                guard += 1;
            }
            _ => return true,
        }
    }
}

/// The edit bone `make_boneList` builds from a rest bone.
fn edit_bone_of(b: &Bone) -> EditBone {
    EditBone {
        name: b.name.clone(),
        parent: b.parent,
        head: b.head_local,
        tail: b.tail_local,
        roll: b.roll,
        use_connect: b.use_connect,
        use_deform: b.use_deform,
        use_inherit_rotation: b.use_inherit_rotation,
        inherit_scale: b.inherit_scale.clone(),
        use_local_location: b.use_local_location,
        envelope_distance: b.envelope_distance,
        envelope_weight: b.envelope_weight,
        head_radius: b.head_radius,
        tail_radius: b.tail_radius,
        select: b.select,
        hide: b.hide,
        hide_select: b.hide_select,
        lock: false,
        collections: b.collections.clone(),
        use_relative_parent: b.use_relative_parent,
        use_envelope_multiply: b.use_envelope_multiply,
        bbone_segments: b.bbone_segments,
        length_stored: (glam::Vec3::from_array(b.tail_local)
            - glam::Vec3::from_array(b.head_local))
        .length(),
    }
}

/// The rest bone `ED_armature_from_edit` builds from an edit bone.
fn bone_of(e: &EditBone, parent: Option<usize>, r: &rest::RestBone) -> Bone {
    Bone {
        name: e.name.clone(),
        parent,
        head_local: e.head,
        tail_local: e.tail,
        matrix_local: rest::rows4(&r.arm_mat),
        roll: e.roll,
        use_connect: e.use_connect,
        use_deform: e.use_deform,
        use_inherit_rotation: e.use_inherit_rotation,
        inherit_scale: e.inherit_scale.clone(),
        use_local_location: e.use_local_location,
        use_relative_parent: e.use_relative_parent,
        envelope_distance: e.envelope_distance,
        envelope_weight: e.envelope_weight,
        use_envelope_multiply: e.use_envelope_multiply,
        head_radius: e.head_radius,
        tail_radius: e.tail_radius,
        bbone_segments: e.bbone_segments,
        hide: e.hide,
        hide_select: e.hide_select,
        select: e.select,
        collections: e.collections.clone(),
    }
}

/// Depth-first order of the tree the parent links make (roots in list order, children in list
/// order). Bones on a parent cycle (cannot happen through Blender's setters) become roots.
fn depth_first(bones: &[EditBone]) -> Vec<usize> {
    let n = bones.len();
    let valid_parent = |i: usize| -> Option<usize> {
        let p = bones[i].parent.filter(|&p| p < n && p != i)?;
        // A cycle never reaches a root: treat such a bone as a root.
        let mut c = Some(p);
        let mut steps = 0;
        while let Some(j) = c {
            if j == i || steps > n {
                return None;
            }
            steps += 1;
            c = bones[j].parent.filter(|&q| q < n);
        }
        Some(p)
    };
    let parents: Vec<Option<usize>> = (0..n).map(valid_parent).collect();
    let mut out = Vec::with_capacity(n);
    fn walk(i: usize, parents: &[Option<usize>], out: &mut Vec<usize>) {
        out.push(i);
        for (j, p) in parents.iter().enumerate() {
            if *p == Some(i) {
                walk(j, parents, out);
            }
        }
    }
    for i in 0..n {
        if parents[i].is_none() {
            walk(i, &parents, &mut out);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_bones_and_matrices() {
        let mut ea = EditArmature::default();
        let a = ea.add_bone("Bone");
        ea.edit_bones[a].tail = [0.0, 0.0, 1.0];
        let b = ea.add_bone("Bone");
        assert_eq!(ea.edit_bones[b].name, "Bone.001");
        ea.edit_bones[b].head = [0.0, 0.0, 1.0];
        ea.edit_bones[b].tail = [0.0, 1.0, 1.0];
        ea.edit_bones[b].parent = Some(a);
        ea.edit_bones[b].use_connect = true;
        ea.active = Some(b);
        let mut arm = Armature::new("A");
        ea.apply_to(&mut arm);
        assert_eq!(arm.bones.len(), 2);
        assert_eq!(arm.bones[1].parent, Some(0));
        assert_eq!(arm.active_bone, Some(1));
        let back = EditArmature::from_armature(&arm);
        assert_eq!(back.edit_bones[1].head, [0.0, 0.0, 1.0]);
        assert_eq!(back.active, Some(1));
    }

    #[test]
    fn zero_length_bones_are_elongated() {
        let mut ea = EditArmature::default();
        ea.add_bone("Bone");
        let mut arm = Armature::new("A");
        ea.apply_to(&mut arm);
        assert_eq!(arm.bones[0].tail_local, [0.0, 0.0, 2e-6]);
    }

    #[test]
    fn depth_first_puts_children_after_parents() {
        let mut ea = EditArmature::default();
        for n in ["A", "B", "C"] {
            ea.add_bone(n);
        }
        // C is A's child, B a root: order A, C, B.
        ea.edit_bones[2].parent = Some(0);
        assert_eq!(depth_first(&ea.edit_bones), vec![0, 2, 1]);
    }

    #[test]
    fn sync_selection_follows_connected_parent() {
        let mut ea = EditArmature::default();
        let a = ea.add_bone("A");
        let b = ea.add_bone("B");
        ea.edit_bones[b].parent = Some(a);
        ea.edit_bones[b].use_connect = true;
        ea.edit_bones[a].select = [false, false, true];
        ea.edit_bones[b].select = [false, false, true];
        ea.sync_selection();
        assert_eq!(ea.edit_bones[b].select, [true, true, true]);
        assert_eq!(ea.edit_bones[a].select, [false, false, true]);
    }
}
