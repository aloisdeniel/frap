// SPDX-License-Identifier: GPL-3.0-or-later
//! Edits of an action's slots, layers, strips, channelbags, groups and F-curves, behind the
//! Action data API (`slots.new`, `layers.new`, `strips.new`, `channelbag(…, ensure=True)`,
//! `fcurves.new`, `groups.new`, …).
//!
//! Ported from blender/source/blender/animrig/intern/action.cc (`Action::slot_add*`,
//! `slot_identifier_ensure_unique`, `slot_display_name_define`, `slot_remove`, `layer_add`,
//! `Layer::strip_add`, `StripKeyframeData::channelbag_for_slot_*`, `Channelbag::fcurve_create`,
//! `channel_group_create`, `channel_group_remove`, `fcurve_assign_to_channel_group`,
//! `fcurve_ungroup`, `fcurve_detach_by_index`, `fcurves_clear`) and animrig/intern/fcurve.cc
//! (`fcurve_find`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Blender keeps a group's F-curves as a contiguous range of the channelbag's array, groups in
//! order, ungrouped F-curves last; frap's `FCurve::group` names the group and these functions
//! keep the same array order.

use frap_rna::naming::unique_subname;
use smol_str::SmolStr;

use super::{Action, ActionGroup, ActionLayer, ActionSlot, ActionStrip, Channelbag, KeyframeStrip};
use crate::fcurve::FCurve;

/// `DNA_DEFAULT_ACTION_LAST_SLOT_HANDLE`: slot handles start above it, so they are never
/// mistaken for array indices (the first slot of an action is `0x37627bf6` = 929201142,
/// verified on 5.2.4).
pub const DEFAULT_LAST_SLOT_HANDLE: i32 = 0x3762_7bf5;

/// `Slot::identifier_length_max - 1` (`MAX_ID_NAME`).
const SLOT_IDENTIFIER_MAX: usize = 257;
/// `MAX_NAME - 1` (group and layer names).
const NAME_MAX: usize = 63;
/// `slot_untyped_prefix`
const UNTYPED_PREFIX: &str = "XX";
/// `slot_default_name`
const SLOT_DEFAULT_NAME: &str = "Slot";

/// Every `ID_Type` with its RNA identifier (`rna_enum_id_type_items`) and two-letter code.
const ID_TYPES: &[(&str, &str)] = &[
    ("ACTION", "AC"),
    ("ARMATURE", "AR"),
    ("BRUSH", "BR"),
    ("CACHEFILE", "CF"),
    ("CAMERA", "CA"),
    ("COLLECTION", "GR"),
    ("CURVE", "CU"),
    ("CURVES", "CV"),
    ("FONT", "VF"),
    ("GREASEPENCIL", "GD"),
    ("GREASEPENCIL_V3", "GP"),
    ("IMAGE", "IM"),
    ("KEY", "KE"),
    ("LATTICE", "LT"),
    ("LIBRARY", "LI"),
    ("LIGHT", "LA"),
    ("LIGHT_PROBE", "LP"),
    ("LINESTYLE", "LS"),
    ("MASK", "MS"),
    ("MATERIAL", "MA"),
    ("MESH", "ME"),
    ("META", "MB"),
    ("MOVIECLIP", "MC"),
    ("NODETREE", "NT"),
    ("OBJECT", "OB"),
    ("PAINTCURVE", "PC"),
    ("PALETTE", "PL"),
    ("PARTICLE", "PA"),
    ("POINTCLOUD", "PT"),
    ("SCENE", "SC"),
    ("SCREEN", "SR"),
    ("SOUND", "SO"),
    ("SPEAKER", "SK"),
    ("TEXT", "TX"),
    ("TEXTURE", "TE"),
    ("VOLUME", "VO"),
    ("WINDOWMANAGER", "WM"),
    ("WORKSPACE", "WS"),
    ("WORLD", "WO"),
];

/// The two-letter identifier prefix of an `ActionSlot.target_id_type` (`XX` for
/// `UNSPECIFIED` or an unknown type; `Slot::idtype_string`).
pub fn id_type_prefix(id_type: &str) -> &'static str {
    ID_TYPES
        .iter()
        .find(|(t, _)| *t == id_type)
        .map_or(UNTYPED_PREFIX, |(_, c)| c)
}

/// The `target_id_type` of a two-letter ID code (`UNSPECIFIED` when unknown).
pub fn id_type_of_prefix(prefix: &str) -> &'static str {
    ID_TYPES
        .iter()
        .find(|(_, c)| *c == prefix)
        .map_or("UNSPECIFIED", |(t, _)| t)
}

impl ActionSlot {
    /// A slot as `Action::slot_allocate` makes it (expanded, not selected, not active).
    pub fn new(handle: i32, identifier: impl Into<SmolStr>, target_id_type: &str) -> ActionSlot {
        ActionSlot {
            handle,
            identifier: identifier.into(),
            target_id_type: target_id_type.into(),
            select: false,
            show_expanded: true,
            active: false,
        }
    }

    /// `Slot::has_idtype`
    pub fn has_idtype(&self) -> bool {
        self.target_id_type != "UNSPECIFIED"
    }

    /// `Slot::is_suitable_for` an ID of type `id_type`.
    pub fn is_suitable_for(&self, id_type: &str) -> bool {
        !self.has_idtype() || self.target_id_type == id_type
    }

    /// `ActionSlot.name_display`: the identifier without its two-letter prefix.
    pub fn name_display(&self) -> &str {
        self.identifier.get(2..).unwrap_or("")
    }
}

impl ActionLayer {
    /// A new layer (`Action::layer_add`: full influence, REPLACE, no strips).
    pub fn new(name: impl Into<SmolStr>) -> ActionLayer {
        ActionLayer {
            name: name.into(),
            influence: 1.0,
            mix_mode: "REPLACE".into(),
            strips: Vec::new(),
        }
    }
}

impl ActionGroup {
    /// A new group (`Channelbag::channel_group_create`: selected, collapsed, `DEFAULT`
    /// colours; verified on 5.2.4).
    pub fn new(name: impl Into<SmolStr>) -> ActionGroup {
        ActionGroup {
            name: name.into(),
            color_set: "DEFAULT".into(),
            show_expanded: false,
            mute: false,
            lock: false,
            select: true,
            show_expanded_graph: false,
            use_pin: false,
        }
    }
}

impl ActionStrip {
    /// The keyframe data (every 5.2 strip is a keyframe strip).
    pub fn keyframe(&self) -> Option<&KeyframeStrip> {
        match self {
            ActionStrip::Keyframe(k) => Some(k),
        }
    }

    /// The keyframe data, mutably.
    pub fn keyframe_mut(&mut self) -> Option<&mut KeyframeStrip> {
        match self {
            ActionStrip::Keyframe(k) => Some(k),
        }
    }
}

impl KeyframeStrip {
    /// `channelbag_for_slot`
    pub fn channelbag_index(&self, slot_handle: i32) -> Option<usize> {
        self.channelbags
            .iter()
            .position(|c| c.slot_handle == slot_handle)
    }

    /// `channelbag_for_slot_ensure`: the index of the channelbag of `slot_handle`, appended
    /// when missing.
    pub fn channelbag_ensure(&mut self, slot_handle: i32) -> usize {
        match self.channelbag_index(slot_handle) {
            Some(i) => i,
            None => {
                self.channelbags.push(Channelbag {
                    slot_handle,
                    ..Channelbag::default()
                });
                self.channelbags.len() - 1
            }
        }
    }
}

impl Action {
    /// The index of the slot with `handle`.
    pub fn slot_index(&self, handle: i32) -> Option<usize> {
        self.slots.iter().position(|s| s.handle == handle)
    }

    /// The slot with `handle`, mutably.
    pub fn slot_mut(&mut self, handle: i32) -> Option<&mut ActionSlot> {
        self.slots.iter_mut().find(|s| s.handle == handle)
    }

    /// `slot_find_by_identifier`
    pub fn slot_by_identifier(&self, identifier: &str) -> Option<&ActionSlot> {
        self.slots.iter().find(|s| s.identifier == identifier)
    }

    /// `slot_identifier_ensure_unique`: `requested`, made unique among the other slots.
    fn unique_slot_identifier(&self, requested: &str, except: Option<i32>) -> SmolStr {
        unique_subname(
            &|n| {
                self.slots
                    .iter()
                    .any(|s| Some(s.handle) != except && s.identifier == n)
            },
            requested,
            SLOT_IDENTIFIER_MAX,
            "",
        )
    }

    /// `Action::slot_allocate` + append: a new slot with the next handle.
    fn slot_push(&mut self, identifier: &str, id_type: &str) -> i32 {
        self.last_slot_handle += 1;
        let handle = self.last_slot_handle;
        let identifier = self.unique_slot_identifier(identifier, None);
        self.slots
            .push(ActionSlot::new(handle, identifier, id_type));
        handle
    }

    /// `slots.new(id_type, name)` (`slot_add_for_id_type` + `slot_display_name_set`): a slot
    /// identified as `<prefix><name>` (made unique with `.001`, …); its handle.
    pub fn slot_add_for_id_type(&mut self, id_type: &str, display_name: &str) -> i32 {
        let prefix = id_type_prefix(id_type);
        let id_type = if prefix == UNTYPED_PREFIX {
            "UNSPECIFIED"
        } else {
            id_type
        };
        let handle = self.slot_push(&format!("{prefix}{SLOT_DEFAULT_NAME}"), id_type);
        self.slot_display_name_define(handle, display_name);
        handle
    }

    /// `Action::slot_add_for_id`: a slot for an ID of `id_type` named `id_name` (the ID's name
    /// with its two-letter code, `OBCube`), preferring the animated data's
    /// `last_slot_identifier` when it has one (keying, WP 6.4).
    pub fn slot_add_for_id(
        &mut self,
        id_type: &str,
        id_name: &str,
        last_slot_identifier: &str,
    ) -> i32 {
        let requested = if last_slot_identifier.is_empty() {
            id_name
        } else {
            last_slot_identifier
        };
        let handle = self.slot_push(requested, id_type);
        self.slot_identifier_ensure_prefix(handle);
        handle
    }

    /// `slot_display_name_define`: replaces what follows the prefix (`name_display`).
    pub fn slot_display_name_define(&mut self, handle: i32, name: &str) {
        let Some(i) = self.slot_index(handle) else {
            return;
        };
        let prefix: String = self.slots[i].identifier.chars().take(2).collect();
        let id = self.unique_slot_identifier(&format!("{prefix}{name}"), Some(handle));
        self.slots[i].identifier = id;
    }

    /// `slot_identifier_define`: a new identifier (made unique).
    pub fn slot_identifier_define(&mut self, handle: i32, identifier: &str) {
        let id = self.unique_slot_identifier(identifier, Some(handle));
        if let Some(s) = self.slot_mut(handle) {
            s.identifier = id;
        }
    }

    /// `slot_identifier_ensure_prefix`: the prefix follows `target_id_type`.
    pub fn slot_identifier_ensure_prefix(&mut self, handle: i32) {
        let Some(i) = self.slot_index(handle) else {
            return;
        };
        let s = &self.slots[i];
        let prefix = id_type_prefix(&s.target_id_type);
        let rest: String = s.identifier.chars().skip(2).collect();
        let id = self.unique_slot_identifier(&format!("{prefix}{rest}"), Some(handle));
        self.slots[i].identifier = id;
    }

    /// `slot_setup_for_id`: binds an untyped slot to `id_type` on assignment.
    pub fn slot_setup_for_id(&mut self, handle: i32, id_type: &str) {
        let Some(s) = self.slot_mut(handle) else {
            return;
        };
        if s.has_idtype() {
            return;
        }
        s.target_id_type = id_type.into();
        self.slot_identifier_ensure_prefix(handle);
    }

    /// `Action::slot_remove`: the slot and its channelbags in every keyframe strip.
    pub fn slot_remove(&mut self, handle: i32) -> bool {
        let Some(i) = self.slot_index(handle) else {
            return false;
        };
        for layer in &mut self.layers {
            for strip in &mut layer.strips {
                let ActionStrip::Keyframe(k) = strip;
                k.channelbags.retain(|c| c.slot_handle != handle);
            }
        }
        self.slots.remove(i);
        true
    }

    /// `animrig::duplicate_slot`: a copy of the slot (identifier made unique, type and flags
    /// kept, the channels of every keyframe strip copied); the new handle.
    pub fn slot_duplicate(&mut self, handle: i32) -> Option<i32> {
        let src = self.slot(handle)?.clone();
        self.last_slot_handle += 1;
        let new = self.last_slot_handle;
        let identifier = self.unique_slot_identifier(&src.identifier, None);
        // `clone_slot` copies everything but the handle (flags included).
        self.slots.push(ActionSlot {
            handle: new,
            identifier,
            ..src
        });
        for layer in &mut self.layers {
            for strip in &mut layer.strips {
                let ActionStrip::Keyframe(k) = strip;
                if let Some(i) = k.channelbag_index(handle) {
                    let mut bag = k.channelbags[i].clone();
                    bag.slot_handle = new;
                    k.channelbags.push(bag);
                }
            }
        }
        Some(new)
    }

    /// `generic_slot_for_autoassign`: the slot an ID of `id_type` named `id_name` (with its
    /// two-letter code, `OBCube`) gets when this action is assigned to it, given the
    /// `last_slot_identifier` it remembers: that identifier (preferring a typed variant of an
    /// untyped one, falling back to the untyped variant of a typed one), then the ID's name,
    /// then the only slot when it is untyped.
    pub fn slot_for_autoassign(&self, id_type: &str, id_name: &str, last: &str) -> Option<i32> {
        let suitable = |identifier: &str| {
            self.slot_by_identifier(identifier)
                .filter(|s| s.is_suitable_for(id_type))
                .map(|s| s.handle)
        };
        if !last.is_empty() {
            let rest = last.get(2..).unwrap_or("");
            let typed = !last.starts_with(UNTYPED_PREFIX);
            if !typed
                && let Some(h) = suitable(&format!("{}{rest}", id_name.get(..2).unwrap_or("")))
            {
                return Some(h);
            }
            if let Some(h) = suitable(last) {
                return Some(h);
            }
            if typed && let Some(h) = suitable(&format!("{UNTYPED_PREFIX}{rest}")) {
                return Some(h);
            }
        }
        if let Some(h) = suitable(id_name) {
            return Some(h);
        }
        match self.slots.as_slice() {
            [only] if !only.has_idtype() => Some(only.handle),
            _ => None,
        }
    }

    /// `Action::slot_active_set`
    pub fn slot_active_set(&mut self, handle: i32) {
        for s in &mut self.slots {
            s.active = s.handle == handle;
        }
    }

    /// `Action::is_empty`: no layers and no slots.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty() && self.slots.is_empty()
    }

    /// `Action.curve_frame_range`: the range of every key (`get_frame_range_of_keys`).
    pub fn curve_frame_range(&self) -> [f32; 2] {
        crate::eval::nla::frame_range_of_keys(self)
    }

    /// `Action.frame_range` (`Action::get_frame_range`): the manual range when set, the keys'
    /// otherwise.
    pub fn frame_range_get(&self) -> [f32; 2] {
        if self.use_frame_range {
            self.frame_range
        } else {
            self.curve_frame_range()
        }
    }

    /// `Action::layer_keystrip_ensure` then `channelbag_for_slot_ensure`: the channelbag of
    /// `slot_handle` in the first strip of the first layer, created with the layer and strip
    /// when missing (keying).
    pub fn channelbag_ensure(&mut self, slot_handle: i32) -> &mut Channelbag {
        if self.layers.is_empty() {
            self.layers.push(ActionLayer::new("Layer"));
        }
        let layer = &mut self.layers[0];
        if layer.strips.is_empty() {
            layer
                .strips
                .push(ActionStrip::Keyframe(KeyframeStrip::default()));
        }
        let ActionStrip::Keyframe(k) = &mut layer.strips[0];
        let i = k.channelbag_ensure(slot_handle);
        &mut k.channelbags[i]
    }

    /// The channelbag of slot `handle` in the first strip of the first layer, mutably.
    pub fn channelbag_mut(&mut self, handle: i32) -> Option<&mut Channelbag> {
        let ActionStrip::Keyframe(s) = self.layers.first_mut()?.strips.first_mut()?;
        s.channelbags.iter_mut().find(|c| c.slot_handle == handle)
    }

    /// Every F-curve of every channelbag (the old `action.fcurves` view, `fcurves_all`).
    pub fn all_fcurves(&self) -> impl Iterator<Item = &FCurve> {
        self.layers
            .iter()
            .flat_map(|l| &l.strips)
            .flat_map(|s| {
                let ActionStrip::Keyframe(k) = s;
                &k.channelbags
            })
            .flat_map(|b| &b.fcurves)
    }

    /// `Action.deselect_keys`
    pub fn deselect_keys(&mut self) {
        for layer in &mut self.layers {
            for strip in &mut layer.strips {
                let ActionStrip::Keyframe(k) = strip;
                for bag in &mut k.channelbags {
                    for fc in &mut bag.fcurves {
                        for key in &mut fc.keyframe_points {
                            key.select_control_point = false;
                            key.select_left_handle = false;
                            key.select_right_handle = false;
                        }
                    }
                }
            }
        }
    }
}

impl Channelbag {
    /// `fcurve_find`
    pub fn fcurve_find(&self, data_path: &str, array_index: i32) -> Option<usize> {
        self.fcurves
            .iter()
            .position(|f| f.array_index == array_index && f.data_path == data_path)
    }

    /// `channel_group_find`
    pub fn group_find(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.name == name)
    }

    /// The number of F-curves of group `g` (`fcurve_range_length`).
    fn group_len(&self, g: usize) -> usize {
        let name = &self.groups[g].name;
        self.fcurves
            .iter()
            .filter(|f| f.group.as_ref() == Some(name))
            .count()
    }

    /// `fcurve_range_start + fcurve_range_length` of group `g`.
    fn group_end(&self, g: usize) -> usize {
        (0..=g).map(|i| self.group_len(i)).sum()
    }

    /// `channel_group_create`: a new group with a unique name (`Group` when empty); its
    /// index.
    pub fn group_create(&mut self, name: &str) -> usize {
        let name = unique_subname(
            &|n| self.groups.iter().any(|g| g.name == n),
            name,
            NAME_MAX,
            "Group",
        );
        self.groups.push(ActionGroup::new(name));
        self.groups.len() - 1
    }

    /// `channel_group_ensure`
    pub fn group_ensure(&mut self, name: &str) -> usize {
        match self.group_find(name) {
            Some(g) => g,
            None => self.group_create(name),
        }
    }

    /// `Channelbag::fcurve_create`: a new F-curve (in `group` when given, which is created if
    /// needed: at the end of the group's range); its index.
    pub fn fcurve_create(
        &mut self,
        data_path: &str,
        array_index: i32,
        group: Option<&str>,
    ) -> usize {
        let mut fc = FCurve::new(data_path, array_index);
        match group {
            Some(g) => {
                let gi = self.group_ensure(g);
                let at = self.group_end(gi);
                fc.group = Some(self.groups[gi].name.clone());
                self.fcurves.insert(at, fc);
                at
            }
            None => {
                self.fcurves.push(fc);
                self.fcurves.len() - 1
            }
        }
    }

    /// `fcurve_create_unique`: `None` when the F-curve exists.
    pub fn fcurve_create_unique(
        &mut self,
        data_path: &str,
        array_index: i32,
        group: Option<&str>,
    ) -> Option<usize> {
        if self.fcurve_find(data_path, array_index).is_some() {
            return None;
        }
        Some(self.fcurve_create(data_path, array_index, group))
    }

    /// `fcurve_ensure`
    pub fn fcurve_ensure(
        &mut self,
        data_path: &str,
        array_index: i32,
        group: Option<&str>,
    ) -> usize {
        match self.fcurve_find(data_path, array_index) {
            Some(i) => i,
            None => self.fcurve_create(data_path, array_index, group),
        }
    }

    /// Removes group `g` when no F-curve is in it any more (`channel_group_remove_raw` after a
    /// range shrank to zero).
    fn drop_group_if_empty(&mut self, name: &SmolStr) {
        if let Some(g) = self.group_find(name)
            && self.group_len(g) == 0
        {
            self.groups.remove(g);
        }
    }

    /// `fcurve_detach_by_index` / `fcurve_remove`: the F-curve, removing its group when it was
    /// the last one in it.
    pub fn fcurve_remove(&mut self, index: usize) -> Option<FCurve> {
        if index >= self.fcurves.len() {
            return None;
        }
        let fc = self.fcurves.remove(index);
        if let Some(g) = &fc.group {
            self.drop_group_if_empty(g);
        }
        Some(fc)
    }

    /// `fcurve_append`: an F-curve at the end, ungrouped.
    pub fn fcurve_append(&mut self, mut fc: FCurve) -> usize {
        fc.group = None;
        self.fcurves.push(fc);
        self.fcurves.len() - 1
    }

    /// `fcurves_clear`: every F-curve (the groups stay, empty).
    pub fn fcurves_clear(&mut self) {
        self.fcurves.clear();
    }

    /// `channel_group_remove`: the group's F-curves move to the start of the ungrouped part.
    pub fn group_remove(&mut self, g: usize) -> bool {
        if g >= self.groups.len() {
            return false;
        }
        let name = self.groups[g].name.clone();
        let (mut members, rest): (Vec<FCurve>, Vec<FCurve>) = self
            .fcurves
            .drain(..)
            .partition(|f| f.group.as_ref() == Some(&name));
        for f in &mut members {
            f.group = None;
        }
        self.groups.remove(g);
        let (grouped, ungrouped): (Vec<FCurve>, Vec<FCurve>) =
            rest.into_iter().partition(|f| f.group.is_some());
        self.fcurves = grouped;
        self.fcurves.extend(members);
        self.fcurves.extend(ungrouped);
        true
    }

    /// `fcurve_assign_to_channel_group`: moves F-curve `index` to the end of group `g`.
    pub fn fcurve_assign_to_group(&mut self, index: usize, g: usize) -> bool {
        if index >= self.fcurves.len() || g >= self.groups.len() {
            return false;
        }
        let target = self.groups[g].name.clone();
        if self.fcurves[index].group.as_ref() == Some(&target) {
            return true;
        }
        let mut fc = self.fcurves.remove(index);
        if let Some(old) = fc.group.take() {
            self.drop_group_if_empty(&old);
        }
        let Some(g) = self.group_find(&target) else {
            return false;
        };
        let at = self.group_end(g);
        fc.group = Some(target);
        self.fcurves.insert(at, fc);
        true
    }

    /// `fcurve_ungroup`: moves F-curve `index` to the end of the array.
    pub fn fcurve_ungroup(&mut self, index: usize) -> bool {
        if index >= self.fcurves.len() {
            return false;
        }
        let mut fc = self.fcurves.remove(index);
        if let Some(old) = fc.group.take() {
            self.drop_group_if_empty(&old);
        }
        self.fcurves.push(fc);
        true
    }

    /// Renames group `g`, following its F-curves (`ActionGroup.name` is a plain string in
    /// Blender; the F-curves point to the group).
    pub fn group_rename(&mut self, g: usize, name: &str) {
        let Some(group) = self.groups.get_mut(g) else {
            return;
        };
        let old = std::mem::replace(&mut group.name, name.into());
        for f in &mut self.fcurves {
            if f.group.as_ref() == Some(&old) {
                f.group = Some(name.into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verified on 5.2.4: handles start at 929201142; identifiers `OBCube`, `OBCube.001`;
    /// `XX` for `UNSPECIFIED`.
    #[test]
    fn slots() {
        let mut a = Action::new("A");
        let h = a.slot_add_for_id_type("OBJECT", "Cube");
        assert_eq!(h, 929_201_142);
        assert_eq!(a.slot(h).map(|s| s.identifier.as_str()), Some("OBCube"));
        let h2 = a.slot_add_for_id_type("OBJECT", "Cube");
        assert_eq!(
            a.slot(h2).map(|s| s.identifier.as_str()),
            Some("OBCube.001")
        );
        let h3 = a.slot_add_for_id_type("UNSPECIFIED", "Any");
        assert_eq!(a.slot(h3).map(|s| s.identifier.as_str()), Some("XXAny"));
        a.slot_setup_for_id(h3, "MATERIAL");
        assert_eq!(a.slot(h3).map(|s| s.identifier.as_str()), Some("MAAny"));
        assert!(a.slot_remove(h));
        assert!(!a.slot_remove(h));
        assert_eq!(a.slot_add_for_id_type("OBJECT", "X"), 929_201_145);
    }

    /// Verified on 5.2.4: `groups.new("G")` twice gives `G`, `G.001`; a grouped F-curve goes
    /// before the ungrouped ones.
    #[test]
    fn groups_keep_array_order() {
        let mut b = Channelbag::default();
        b.fcurve_create("location", 0, None);
        assert_eq!(b.group_create("G"), 0);
        let g = b.group_create("G");
        assert_eq!(b.groups[g].name, "G.001");
        let i = b.fcurve_create("rotation_euler", 2, Some("G"));
        assert_eq!(i, 0);
        assert_eq!(b.fcurves[1].data_path, "location");
        assert_eq!(b.fcurve_create_unique("location", 0, None), None);
        b.fcurve_assign_to_group(1, 1);
        assert_eq!(
            b.fcurves
                .iter()
                .map(|f| f.group.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("G"), Some("G.001")]
        );
        // Removing the last F-curve of a group removes the group.
        b.fcurve_remove(0);
        assert_eq!(b.groups.len(), 1);
        b.group_remove(0);
        assert!(b.groups.is_empty() && b.fcurves[0].group.is_none());
    }
}
