// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone collection management (WP 8.3.10, M8): adding, inserting as a child, reordering and
//! removing bone collections exactly as Blender lays out `Armature.collections_all`
//! (`collection_array`: the roots first, the children of each collection in one contiguous
//! block, `child_index` / `child_count` per collection), plus the membership helpers of the
//! `armature.collection_*`, `assign_to_collection` and `move_to_collection` operators.
//!
//! frap stores the array as `Armature::collections` (in `collections_all` order) with a
//! `parent` index per collection; [`CollArray`] rebuilds Blender's hierarchy indices from it,
//! runs the ported algorithms (which move items around exactly as Blender does, so
//! `BoneCollection.index` and `collections_all` order match), and writes the result back.
//!
//! Ported from blender/source/blender/animrig/intern/bone_collections.cc
//! (`ANIM_bonecoll_new`, `bonecoll_ensure_name_unique`, `bonecoll_insert_at_index`,
//! `bonecoll_insert_as_root`, `bonecoll_insert_as_child`, `ANIM_armature_bonecoll_new`,
//! `ANIM_armature_bonecoll_active_set`, `ANIM_armature_bonecoll_active_index_set`,
//! `ANIM_armature_bonecoll_remove_from_index`, `bonecoll_child_number`,
//! `armature_bonecoll_find_parent_index`, `armature_bonecoll_child_number_find`,
//! `armature_bonecoll_child_number_set`, `armature_bonecoll_is_root`,
//! `armature_bonecoll_is_child_of`, `armature_bonecoll_move_to_parent`,
//! `bonecolls_rotate_block`, `bonecolls_move_to_index`, `bonecolls_find_index_near`), Blender
//! 5.2.4, GPL-2.0-or-later. The `BONE_COLLECTION_ANCESTORS_VISIBLE` and solo flags are not
//! stored by frap (visibility is derived on read), so their refreshes have no counterpart.

#![allow(clippy::needless_range_loop)] // index loops mirror Blender's array walks

use smol_str::SmolStr;

use crate::id::{Armature, BoneCollection};

/// `bonecoll_default_name`
pub const DEFAULT_NAME: &str = "Bones";

/// One entry of `collection_array`.
#[derive(Clone, Debug)]
struct Item {
    /// Identity (the `BoneCollection *` Blender compares).
    id: usize,
    coll: BoneCollection,
    child_index: usize,
    child_count: usize,
}

/// Blender's bone collection array of one armature, with the runtime active collection.
#[derive(Clone, Debug)]
pub struct CollArray {
    items: Vec<Item>,
    root_count: usize,
    /// `runtime->active_collection_index` (-1 = none).
    active_index: i64,
    /// `runtime->active_collection` (identity).
    active_ptr: Option<usize>,
    next_id: usize,
}

impl CollArray {
    /// The array of `arm` (its collections must be in Blender's layout: roots first, the
    /// children of a collection contiguous; frap's own operators keep it so; another order is
    /// first sorted into that layout, stably).
    pub fn from_armature(arm: &Armature) -> CollArray {
        let colls = layout(&arm.collections);
        let n = colls.len();
        let mut items: Vec<Item> = colls
            .iter()
            .enumerate()
            .map(|(i, c)| Item {
                id: i,
                coll: c.clone(),
                child_index: 0,
                child_count: 0,
            })
            .collect();
        let mut root_count = 0;
        for i in 0..n {
            match colls[i].parent {
                Some(p) if p < n => {
                    if items[p].child_count == 0 {
                        items[p].child_index = i;
                    }
                    items[p].child_count += 1;
                }
                _ => root_count += 1,
            }
        }
        // `layout` keeps an array already in layout as is; after a reorder the active
        // collection is found again by name.
        let active = arm
            .active_collection
            .and_then(|a| arm.collections.get(a))
            .and_then(|c| colls.iter().position(|x| x.name == c.name));
        CollArray {
            items,
            root_count,
            active_index: active.map_or(-1, |a| a as i64),
            active_ptr: active,
            next_id: n,
        }
    }

    /// Writes the collections (parents from the hierarchy indices) and the active collection
    /// back to `arm`.
    pub fn store(&self, arm: &mut Armature) {
        let n = self.items.len();
        let mut parents: Vec<Option<usize>> = vec![None; n];
        for (p, it) in self.items.iter().enumerate() {
            for c in it.child_index..(it.child_index + it.child_count).min(n) {
                if it.child_count > 0 {
                    parents[c] = Some(p);
                }
            }
        }
        arm.collections = self
            .items
            .iter()
            .zip(parents)
            .map(|(it, parent)| BoneCollection {
                parent,
                ..it.coll.clone()
            })
            .collect();
        arm.active_collection = self.active();
    }

    /// The active collection index (`collections.active_index`).
    pub fn active(&self) -> Option<usize> {
        usize::try_from(self.active_index)
            .ok()
            .filter(|&i| i < self.items.len())
    }

    /// Number of collections.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no collections.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The name of collection `i`.
    pub fn name(&self, i: usize) -> &SmolStr {
        &self.items[i].coll.name
    }

    /// `ANIM_armature_bonecoll_get_index_by_name`.
    pub fn index_by_name(&self, name: &str) -> Option<usize> {
        self.items.iter().position(|it| it.coll.name == name)
    }

    fn index_of_id(&self, id: usize) -> Option<usize> {
        self.items.iter().position(|it| it.id == id)
    }

    /// `armature_bonecoll_active_clear`
    fn active_clear(&mut self) {
        self.active_index = -1;
        self.active_ptr = None;
    }

    /// `ANIM_armature_bonecoll_active_index_set`: out of range clears.
    pub fn active_index_set(&mut self, index: i64) {
        match usize::try_from(index)
            .ok()
            .filter(|&i| i < self.items.len())
        {
            Some(i) => {
                self.active_index = index;
                self.active_ptr = Some(self.items[i].id);
            }
            None => self.active_clear(),
        }
    }

    /// `ANIM_armature_bonecoll_active_set` with the collection at `index` (`None` clears).
    pub fn active_set(&mut self, index: Option<usize>) {
        let id = index.and_then(|i| self.items.get(i)).map(|it| it.id);
        self.active_set_id(id);
    }

    fn active_set_id(&mut self, id: Option<usize>) {
        match id.and_then(|id| self.index_of_id(id)) {
            Some(i) => {
                self.active_index = i as i64;
                self.active_ptr = id;
            }
            None => self.active_clear(),
        }
    }

    /// `bonecolls_rotate_block`
    fn rotate_block(&mut self, start: usize, count: usize, direction: i64) {
        if count == 0 {
            return;
        }
        let (move_from, move_to) = if direction > 0 {
            (start + count, start)
        } else {
            (start - 1, start + count - 1)
        };
        if direction > 0 {
            self.items[start..=start + count].rotate_right(1);
        } else {
            self.items[start - 1..start + count].rotate_left(1);
        }
        for it in &mut self.items {
            if it.child_index == 0 && it.child_count == 0 {
                continue;
            }
            if start <= it.child_index && it.child_index < start + count {
                it.child_index = (it.child_index as i64 + direction) as usize;
            }
        }
        let a = self.active_index;
        if a == move_from as i64 {
            self.active_index = move_to as i64;
        } else if start as i64 <= a && a < (start + count) as i64 {
            self.active_index += direction;
        }
    }

    /// `bonecolls_move_to_index`
    fn move_to_index(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        if from < to {
            self.rotate_block(from + 1, to - from, -1);
        } else {
            self.rotate_block(to, from - to, 1);
        }
    }

    /// `bonecolls_find_index_near`
    fn find_index_near(&self, id: usize, index: usize) -> Option<usize> {
        if self.items.get(index).is_some_and(|it| it.id == id) {
            return Some(index);
        }
        if index > 0 && self.items[index - 1].id == id {
            return Some(index - 1);
        }
        if index + 1 < self.items.len() && self.items[index + 1].id == id {
            return Some(index + 1);
        }
        None
    }

    /// `bonecoll_insert_at_index`
    fn insert_at_index(&mut self, item: Item, index: usize) {
        self.items.push(item);
        let rotate_count = self.items.len() - index - 1;
        self.rotate_block(index, rotate_count, 1);
        if self.active_index >= index as i64 {
            self.active_index_set(self.active_index + 1);
        }
    }

    /// `bonecoll_ensure_name_unique`: `Bones`, `Bones.001`, … among the other collections.
    fn unique_name(&self, name: &str, skip: Option<usize>) -> SmolStr {
        frap_rna::naming::unique_subname(
            &|n| {
                self.items
                    .iter()
                    .any(|it| Some(it.id) != skip && it.coll.name == n)
            },
            name,
            super::names::MAX_BONE_NAME,
            DEFAULT_NAME,
        )
    }

    /// `ANIM_armature_bonecoll_new(armature, name, parent_index)`: a new visible collection
    /// (`Bones` when `name` is empty, made unique), appended to the roots (`parent_index` < 0)
    /// or to the children of `parent_index`; the active collection is kept. Returns its index.
    pub fn new_collection(&mut self, name: &str, parent_index: i64) -> usize {
        let name = if name.is_empty() { DEFAULT_NAME } else { name };
        let name = frap_rna::naming::truncate_utf8(name, super::names::MAX_BONE_NAME);
        let id = self.next_id;
        self.next_id += 1;
        let coll = BoneCollection {
            name: self.unique_name(name, Some(id)),
            parent: None,
            is_visible: true,
            is_solo: false,
        };
        let item = Item {
            id,
            coll,
            child_index: 0,
            child_count: 0,
        };
        match usize::try_from(parent_index)
            .ok()
            .filter(|&p| p < self.items.len())
        {
            None => {
                let at = self.root_count;
                self.insert_at_index(item, at);
                self.root_count += 1;
            }
            Some(p) => {
                let parent_id = self.items[p].id;
                if self.items[p].child_index == 0 {
                    self.items[p].child_index = self.items.len();
                }
                let at = self.items[p].child_index + self.items[p].child_count;
                self.insert_at_index(item, at);
                if let Some(pi) = self.index_of_id(parent_id) {
                    self.items[pi].child_count += 1;
                }
            }
        }
        let ptr = self.active_ptr;
        self.active_set_id(ptr);
        self.index_of_id(id).unwrap_or(0)
    }

    /// `armature_bonecoll_find_parent_index` (-1 for a root).
    pub fn find_parent_index(&self, index: i64) -> i64 {
        if index < self.root_count as i64 {
            return -1;
        }
        for (i, it) in self.items.iter().enumerate() {
            if (it.child_index as i64) <= index && index < (it.child_index + it.child_count) as i64
            {
                return i as i64;
            }
        }
        -1
    }

    /// `bonecoll_child_number`
    fn child_number(&self, parent_index: i64, index: usize) -> i64 {
        if parent_index < 0 {
            return index as i64;
        }
        index as i64 - self.items[parent_index as usize].child_index as i64
    }

    /// `armature_bonecoll_child_number_find`
    pub fn child_number_find(&self, index: usize) -> i64 {
        let parent = self.find_parent_index(index as i64);
        self.child_number(parent, index)
    }

    /// `armature_bonecoll_child_number_set`: moves collection `index` among its siblings
    /// (`-1` = last); returns its new index, or -1 when out of range.
    pub fn child_number_set(&mut self, index: usize, mut new_child_number: i64) -> i64 {
        let parent = self.find_parent_index(index as i64);
        let (child_index, child_count) = if parent < 0 {
            (0, self.root_count)
        } else {
            let p = &self.items[parent as usize];
            (p.child_index, p.child_count)
        };
        if new_child_number >= child_count as i64 {
            return -1;
        }
        if new_child_number < 0 {
            new_child_number = child_count as i64 - 1;
        }
        let old_parent_child_index = child_index;
        let to_index = child_index + new_child_number as usize;
        let parent_id = (parent >= 0).then(|| self.items[parent as usize].id);
        self.move_to_index(index, to_index);
        if let Some(pid) = parent_id
            && let Some(pi) = self.index_of_id(pid)
        {
            self.items[pi].child_index = old_parent_child_index;
        }
        if self.active_index == index as i64 {
            self.active_index_set(to_index as i64);
        }
        to_index as i64
    }

    /// `armature_bonecoll_is_root`
    fn is_root(&self, index: usize) -> bool {
        index < self.root_count
    }

    /// `armature_bonecoll_is_child_of`
    fn is_child_of(&self, parent: i64, child: i64) -> bool {
        if child < 0 {
            return false;
        }
        if self.is_root(child as usize) {
            return parent == -1;
        }
        if parent < 0 {
            return false;
        }
        let p = &self.items[parent as usize];
        (p.child_index as i64) <= child && child < (p.child_index + p.child_count) as i64
    }

    /// `(child_index, child_count)` of a parent (`-1` = the armature's roots).
    fn hier(&self, parent: i64) -> (usize, usize) {
        if parent < 0 {
            (0, self.root_count)
        } else {
            let p = &self.items[parent as usize];
            (p.child_index, p.child_count)
        }
    }

    fn set_hier(&mut self, parent: i64, child_index: usize, child_count: usize) {
        if parent < 0 {
            self.root_count = child_count;
        } else {
            let p = &mut self.items[parent as usize];
            p.child_index = child_index;
            p.child_count = child_count;
        }
    }

    /// `armature_bonecoll_move_to_parent`
    fn move_to_parent(
        &mut self,
        from_index: usize,
        mut to_child_num: i64,
        from_parent: i64,
        to_parent: i64,
    ) -> usize {
        if from_parent == to_parent {
            return from_index;
        }
        let from_id = (from_parent >= 0).then(|| self.items[from_parent as usize].id);
        let to_id = (to_parent >= 0).then(|| self.items[to_parent as usize].id);
        let (to_ci, to_cc) = self.hier(to_parent);
        if to_child_num < 0 {
            to_child_num = to_cc as i64;
        }
        let to_index = if to_cc == 0 {
            self.items.len() - 1
        } else {
            let mut t = to_ci + to_child_num as usize;
            if t > from_index {
                t -= 1;
            }
            t
        };
        let (from_ci, _) = self.hier(from_parent);
        let needs_bump = from_ci == from_index && to_index <= from_index;
        let becomes_first = to_child_num == 0 || to_cc == 0;
        self.move_to_index(from_index, to_index);
        // The parents may have moved: find them again by identity (the armature's roots stay).
        let find = |s: &CollArray, id: Option<usize>| -> i64 {
            id.and_then(|id| s.index_of_id(id)).map_or(-1, |i| i as i64)
        };
        let fp = find(self, from_id);
        let (mut fci, mut fcc) = self.hier(fp);
        fcc -= 1;
        if fcc == 0 {
            fci = 0;
        } else if needs_bump {
            fci += 1;
        }
        self.set_hier(fp, fci, fcc);
        let tp = find(self, to_id);
        let (mut tci, tcc) = self.hier(tp);
        if becomes_first {
            tci = to_index;
        }
        self.set_hier(tp, tci, tcc + 1);
        to_index
    }

    /// `ANIM_armature_bonecoll_remove_from_index`: the collection's children take its place
    /// under its parent; the active collection becomes the next sibling, else the previous one,
    /// else the parent. Returns the removed collection's name (the caller unassigns its bones).
    pub fn remove(&mut self, mut index: usize) -> SmolStr {
        let id = self.items[index].id;
        let name = self.items[index].coll.name.clone();
        let active = self.active_index;
        let mut parent = self.find_parent_index(index as i64);
        let parent_id = (parent >= 0).then(|| self.items[parent as usize].id);
        let mut move_to_child_num = self.child_number(parent, index);
        let mut guard = 0;
        while self.items[index].child_count > 0 && guard <= self.items.len() {
            let child = self.items[index].child_index;
            self.move_to_parent(child, move_to_child_num, index as i64, parent);
            match self.find_index_near(id, index) {
                Some(i) => index = i,
                None => break,
            }
            if let Some(pid) = parent_id {
                match self.find_index_near(pid, parent as usize) {
                    Some(p) => parent = p as i64,
                    None => break,
                }
            }
            move_to_child_num += 1;
            guard += 1;
        }
        if parent < 0 {
            self.root_count = self.root_count.saturating_sub(1);
        } else {
            let p = &mut self.items[parent as usize];
            p.child_count -= 1;
            if p.child_count == 0 {
                p.child_index = 0;
            }
        }
        let last = self.items.len() - 1;
        self.move_to_index(index, last);
        self.items.pop();
        if active >= 0 {
            if self.is_child_of(parent, active) {
                self.active_index_set(active);
            } else if active > 0 && self.is_child_of(parent, active - 1) {
                self.active_index_set(active - 1);
            } else {
                self.active_index_set(parent);
            }
        }
        name
    }
}

/// The collections in Blender's layout (roots first, each collection's children contiguous):
/// an array already in that layout is returned as is; otherwise roots keep their order, then
/// the children blocks follow breadth first (parents re-indexed).
fn layout(colls: &[BoneCollection]) -> Vec<BoneCollection> {
    let n = colls.len();
    let valid = |p: Option<usize>| p.filter(|&p| p < n);
    let in_layout = {
        let roots = colls
            .iter()
            .take_while(|c| valid(c.parent).is_none())
            .count();
        let rest_ok = colls[roots..].iter().all(|c| valid(c.parent).is_some());
        let blocks_ok = (0..n).all(|p| {
            let kids: Vec<usize> = (0..n)
                .filter(|&c| valid(colls[c].parent) == Some(p))
                .collect();
            kids.windows(2).all(|w| w[1] == w[0] + 1)
        });
        rest_ok && blocks_ok
    };
    if in_layout {
        return colls.to_vec();
    }
    let mut order: Vec<usize> = (0..n)
        .filter(|&i| valid(colls[i].parent).is_none())
        .collect();
    let mut k = 0;
    while k < order.len() {
        let p = order[k];
        for c in 0..n {
            if valid(colls[c].parent) == Some(p) && !order.contains(&c) {
                order.push(c);
            }
        }
        k += 1;
    }
    for c in 0..n {
        if !order.contains(&c) {
            order.push(c);
        }
    }
    let mut new_index = vec![0usize; n];
    for (ni, &oi) in order.iter().enumerate() {
        new_index[oi] = ni;
    }
    order
        .iter()
        .map(|&oi| BoneCollection {
            parent: valid(colls[oi].parent).map(|p| new_index[p]),
            ..colls[oi].clone()
        })
        .collect()
}

/// `ANIM_armature_bonecoll_assign_editbone` / `ANIM_armature_bonecoll_assign` on a list of
/// collection names: whether `name` was added.
pub fn assign(list: &mut Vec<SmolStr>, name: &SmolStr) -> bool {
    if list.contains(name) {
        return false;
    }
    list.push(name.clone());
    true
}

/// `ANIM_armature_bonecoll_unassign(_editbone)`: whether `name` was removed.
pub fn unassign(list: &mut Vec<SmolStr>, name: &SmolStr) -> bool {
    match list.iter().position(|n| n == name) {
        Some(i) => {
            list.remove(i);
            true
        }
        None => false,
    }
}

/// `ANIM_armature_bonecoll_assign_and_move(_editbone)`: leaves every collection, joins
/// `name`; always `true`.
pub fn assign_and_move(list: &mut Vec<SmolStr>, name: &SmolStr) -> bool {
    list.clear();
    list.push(name.clone());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm_with(names: &[(&str, Option<usize>)]) -> Armature {
        let mut a = Armature::new("A");
        a.collections = names
            .iter()
            .map(|&(n, p)| BoneCollection {
                name: n.into(),
                parent: p,
                is_visible: true,
                is_solo: false,
            })
            .collect();
        a
    }

    fn names(a: &Armature) -> Vec<(String, Option<usize>)> {
        a.collections
            .iter()
            .map(|c| (c.name.to_string(), c.parent))
            .collect()
    }

    #[test]
    fn new_roots_and_children_keep_the_layout() {
        let mut a = arm_with(&[]);
        let mut arr = CollArray::from_armature(&a);
        assert_eq!(arr.new_collection("", -1), 0);
        assert_eq!(arr.new_collection("", -1), 1);
        // A child of `Bones` goes to the end, a second root before it.
        assert_eq!(arr.new_collection("kid", 0), 2);
        assert_eq!(arr.new_collection("Bones", -1), 2);
        arr.store(&mut a);
        assert_eq!(
            names(&a),
            [
                ("Bones".into(), None),
                ("Bones.001".into(), None),
                ("Bones.002".into(), None),
                ("kid".into(), Some(0)),
            ]
        );
    }

    #[test]
    fn child_number_set_moves_among_siblings() {
        let mut a = arm_with(&[("A", None), ("B", None), ("C", None)]);
        a.active_collection = Some(0);
        let mut arr = CollArray::from_armature(&a);
        let n = arr.new_collection("", -1);
        let to = arr.child_number_set(n, 1);
        assert_eq!(to, 1);
        arr.active_set(Some(1));
        arr.store(&mut a);
        let got: Vec<String> = a.collections.iter().map(|c| c.name.to_string()).collect();
        assert_eq!(got, ["A", "Bones", "B", "C"]);
        assert_eq!(a.active_collection, Some(1));
    }

    #[test]
    fn remove_lifts_children_and_picks_the_next_active() {
        let mut a = arm_with(&[("A", None), ("B", None), ("a1", Some(0)), ("a2", Some(0))]);
        a.active_collection = Some(0);
        let mut arr = CollArray::from_armature(&a);
        assert_eq!(arr.remove(0), "A");
        arr.store(&mut a);
        let got: Vec<String> = a.collections.iter().map(|c| c.name.to_string()).collect();
        assert_eq!(got, ["a1", "a2", "B"]);
        assert!(a.collections.iter().all(|c| c.parent.is_none()));
        assert_eq!(a.active_collection, Some(0));
    }

    #[test]
    fn remove_last_child_selects_parent() {
        let mut a = arm_with(&[("A", None), ("a1", Some(0))]);
        a.active_collection = Some(1);
        let mut arr = CollArray::from_armature(&a);
        arr.remove(1);
        arr.store(&mut a);
        assert_eq!(names(&a), [("A".into(), None)]);
        assert_eq!(a.active_collection, Some(0));
    }

    #[test]
    fn out_of_layout_input_is_sorted() {
        let a = arm_with(&[("kid", Some(1)), ("A", None)]);
        let mut arr = CollArray::from_armature(&a);
        let mut b = a.clone();
        arr.new_collection("x", -1);
        arr.store(&mut b);
        assert_eq!(
            names(&b),
            [
                ("A".into(), None),
                ("x".into(), None),
                ("kid".into(), Some(0)),
            ]
        );
    }

    #[test]
    fn membership_helpers() {
        let mut l: Vec<SmolStr> = vec!["A".into()];
        assert!(!assign(&mut l, &"A".into()));
        assert!(assign(&mut l, &"B".into()));
        assert!(unassign(&mut l, &"A".into()));
        assert!(!unassign(&mut l, &"A".into()));
        assert!(assign_and_move(&mut l, &"B".into()));
        assert_eq!(l, ["B"]);
    }
}
