// SPDX-License-Identifier: GPL-3.0-or-later
//! `Action` and everything below it, as index proxies over the action.

use frap_rna::traits::{index_out_of_range, read_only, wrong_access};
use frap_rna::*;
use smol_str::SmolStr;

use super::{
    EmptyColl, as_array, as_bool, as_f32, as_i64, as_str, boolean, float, floats, int, string,
    unbound,
};
use crate::action::{Action, ActionGroup, ActionStrip, Channelbag, KeyframeStrip};
use crate::fcurve::{
    Easing, Extrapolation, FCurve, HandleType, Interpolation, Keyframe, KeyframeType, keys,
};

/// Where a struct lives inside an action (indices into `slots`, `layers[l].strips[s]`,
/// `channelbags[b]`, `groups[g]`, `fcurves[f]`, `keyframe_points[k]`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Loc {
    /// The action itself.
    Action,
    /// `slots[i]`
    Slot(usize),
    /// `layers[l]`
    Layer(usize),
    /// `layers[l].strips[s]`
    Strip(usize, usize),
    /// `….channelbags[b]`
    Bag(usize, usize, usize),
    /// `….channelbags[b].groups[g]`
    Group(usize, usize, usize, usize),
    /// `….channelbags[b].fcurves[f]`
    FCurve(usize, usize, usize, usize),
    /// `….fcurves[f].keyframe_points[k]`
    Key(usize, usize, usize, usize, usize),
}

/// A collection inside an action.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Coll {
    /// `Action.slots`
    Slots,
    /// `Action.layers`
    Layers,
    /// `ActionLayer.strips`
    Strips(usize),
    /// `ActionKeyframeStrip.channelbags`
    Bags(usize, usize),
    /// `ActionChannelbag.fcurves`
    FCurves(usize, usize, usize),
    /// `ActionChannelbag.groups`
    Groups(usize, usize, usize),
    /// `FCurve.keyframe_points`
    Keys(usize, usize, usize, usize),
    /// `ActionGroup.channels`
    GroupChannels(usize, usize, usize, usize),
}

// ------------------------------------------------------------------------------- access

fn kstrip(a: &Action, l: usize, s: usize) -> Option<&KeyframeStrip> {
    match a.layers.get(l)?.strips.get(s)? {
        ActionStrip::Keyframe(k) => Some(k),
    }
}

fn kstrip_mut(a: &mut Action, l: usize, s: usize) -> Option<&mut KeyframeStrip> {
    match a.layers.get_mut(l)?.strips.get_mut(s)? {
        ActionStrip::Keyframe(k) => Some(k),
    }
}

/// The channelbag at `(l, s, b)`.
pub fn bag(a: &Action, l: usize, s: usize, b: usize) -> Option<&Channelbag> {
    kstrip(a, l, s)?.channelbags.get(b)
}

/// The channelbag at `(l, s, b)`, mutably.
pub fn bag_mut(a: &mut Action, l: usize, s: usize, b: usize) -> Option<&mut Channelbag> {
    kstrip_mut(a, l, s)?.channelbags.get_mut(b)
}

fn stale() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "StructRNA of type Action has been removed",
    )
}

/// The indices of the F-curves of group `g` of a channelbag (`ActionGroup.channels`).
fn group_channels(bag: &Channelbag, g: usize) -> Vec<usize> {
    let Some(name) = bag.groups.get(g).map(|g| &g.name) else {
        return Vec::new();
    };
    (0..bag.fcurves.len())
        .filter(|&i| bag.fcurves[i].group.as_ref() == Some(name))
        .collect()
}

/// The struct definition of `loc`.
pub fn srna_of(loc: Loc) -> &'static StructDef {
    match loc {
        Loc::Action => &frap_spec_rna::ACTION,
        Loc::Slot(_) => &frap_spec_rna::ACTION_SLOT,
        Loc::Layer(_) => &frap_spec_rna::ACTION_LAYER,
        Loc::Strip(..) => &frap_spec_rna::ACTION_KEYFRAME_STRIP,
        Loc::Bag(..) => &frap_spec_rna::ACTION_CHANNELBAG,
        Loc::Group(..) => &frap_spec_rna::ACTION_GROUP,
        Loc::FCurve(..) => &frap_spec_rna::F_CURVE,
        Loc::Key(..) => &frap_spec_rna::KEYFRAME,
    }
}

fn item_type(c: Coll) -> &'static StructDef {
    match c {
        Coll::Slots => &frap_spec_rna::ACTION_SLOT,
        Coll::Layers => &frap_spec_rna::ACTION_LAYER,
        Coll::Strips(_) => &frap_spec_rna::ACTION_STRIP,
        Coll::Bags(..) => &frap_spec_rna::ACTION_CHANNELBAG,
        Coll::FCurves(..) | Coll::GroupChannels(..) => &frap_spec_rna::F_CURVE,
        Coll::Groups(..) => &frap_spec_rna::ACTION_GROUP,
        Coll::Keys(..) => &frap_spec_rna::KEYFRAME,
    }
}

fn coll_len(a: &Action, c: Coll) -> usize {
    match c {
        Coll::Slots => a.slots.len(),
        Coll::Layers => a.layers.len(),
        Coll::Strips(l) => a.layers.get(l).map_or(0, |x| x.strips.len()),
        Coll::Bags(l, s) => kstrip(a, l, s).map_or(0, |k| k.channelbags.len()),
        Coll::FCurves(l, s, b) => bag(a, l, s, b).map_or(0, |x| x.fcurves.len()),
        Coll::Groups(l, s, b) => bag(a, l, s, b).map_or(0, |x| x.groups.len()),
        Coll::Keys(l, s, b, f) => bag(a, l, s, b)
            .and_then(|x| x.fcurves.get(f))
            .map_or(0, |fc| fc.keyframe_points.len()),
        Coll::GroupChannels(l, s, b, g) => {
            bag(a, l, s, b).map_or(0, |x| group_channels(x, g).len())
        }
    }
}

fn coll_key(a: &Action, c: Coll, i: usize) -> Option<SmolStr> {
    match c {
        Coll::Slots => a.slots.get(i).map(|s| s.identifier.clone()),
        Coll::Layers => a.layers.get(i).map(|l| l.name.clone()),
        Coll::Groups(l, s, b) => bag(a, l, s, b)?.groups.get(i).map(|g| g.name.clone()),
        _ => None,
    }
}

/// The location of item `i` of collection `c`.
pub fn item_loc(a: &Action, c: Coll, i: usize) -> Option<Loc> {
    if i >= coll_len(a, c) {
        return None;
    }
    Some(match c {
        Coll::Slots => Loc::Slot(i),
        Coll::Layers => Loc::Layer(i),
        Coll::Strips(l) => Loc::Strip(l, i),
        Coll::Bags(l, s) => Loc::Bag(l, s, i),
        Coll::FCurves(l, s, b) => Loc::FCurve(l, s, b, i),
        Coll::Groups(l, s, b) => Loc::Group(l, s, b, i),
        Coll::Keys(l, s, b, f) => Loc::Key(l, s, b, f, i),
        Coll::GroupChannels(l, s, b, g) => {
            Loc::FCurve(l, s, b, group_channels(bag(a, l, s, b)?, g)[i])
        }
    })
}

/// The collection `prop` of the struct at `loc`.
fn coll_of(loc: Loc, prop: &str) -> Option<Coll> {
    Some(match (loc, prop) {
        (Loc::Action, "slots") => Coll::Slots,
        (Loc::Action, "layers") => Coll::Layers,
        (Loc::Layer(l), "strips") => Coll::Strips(l),
        (Loc::Strip(l, s), "channelbags") => Coll::Bags(l, s),
        (Loc::Bag(l, s, b), "fcurves") => Coll::FCurves(l, s, b),
        (Loc::Bag(l, s, b), "groups") => Coll::Groups(l, s, b),
        (Loc::FCurve(l, s, b, f), "keyframe_points") => Coll::Keys(l, s, b, f),
        (Loc::Group(l, s, b, g), "channels") => Coll::GroupChannels(l, s, b, g),
        _ => return None,
    })
}

/// The struct the pointer `prop` of `loc` points to inside the action (`Channelbag.slot`,
/// `FCurve.group`); `Ok(None)` for a null pointer.
fn pointer_of(a: &Action, loc: Loc, prop: &str) -> Option<Result<Option<Loc>, RnaError>> {
    Some(match (loc, prop) {
        (Loc::Bag(l, s, b), "slot") => bag(a, l, s, b)
            .ok_or_else(stale)
            .map(|x| a.slot_index(x.slot_handle).map(Loc::Slot)),
        (Loc::FCurve(l, s, b, f), "group") => bag(a, l, s, b)
            .and_then(|x| Some((x, x.fcurves.get(f)?)))
            .ok_or_else(stale)
            .map(|(x, fc)| {
                fc.group
                    .as_ref()
                    .and_then(|g| x.group_find(g))
                    .map(|g| Loc::Group(l, s, b, g))
            }),
        _ => return None,
    })
}

/// Locates an `RnaPtr` path below an action (`slots["OBCube"]`,
/// `layers["Layer"].strips[0].channelbags[0].fcurves[2].keyframe_points[1]`, `….slot`,
/// `….group`, `….channels[0]`). A trailing collection name (a function called on
/// `….fcurves`) is returned with the struct that owns it.
pub fn locate<'s>(a: &Action, path: &'s [Seg]) -> Option<(Loc, Option<&'s str>)> {
    let mut loc = Loc::Action;
    let mut i = 0;
    while i < path.len() {
        let Seg::Prop(name) = &path[i] else {
            return None;
        };
        if let Some(c) = coll_of(loc, name) {
            let Some(sub) = path.get(i + 1) else {
                return Some((loc, Some(name)));
            };
            let index = match sub {
                Seg::Index(n) => {
                    let len = coll_len(a, c) as i64;
                    let n = if *n < 0 { *n + len } else { *n };
                    usize::try_from(n).ok()?
                }
                Seg::Key(k) => {
                    (0..coll_len(a, c)).find(|&j| coll_key(a, c, j).as_deref() == Some(k))?
                }
                Seg::Prop(_) => return Some((loc, Some(name))),
            };
            loc = item_loc(a, c, index)?;
            i += 2;
            continue;
        }
        match pointer_of(a, loc, name)? {
            Ok(Some(l)) => loc = l,
            _ => return None,
        }
        i += 1;
    }
    Some((loc, None))
}

// ------------------------------------------------------------------------------- proxies

/// A struct inside an action, read-only.
pub struct Node<'a> {
    /// The action.
    pub a: &'a Action,
    /// Where.
    pub loc: Loc,
}

/// A struct inside an action, writable.
pub struct NodeMut<'a> {
    /// The action.
    pub a: &'a mut Action,
    /// Where.
    pub loc: Loc,
}

/// A collection inside an action, read-only.
pub struct CollView<'a> {
    a: &'a Action,
    c: Coll,
}

/// A collection inside an action, writable.
pub struct CollViewMut<'a> {
    a: &'a mut Action,
    c: Coll,
}

impl Rna for Node<'_> {
    fn srna(&self) -> &'static StructDef {
        srna_of(self.loc)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        get(self.a, self.loc, prop)
    }
}

impl Rna for NodeMut<'_> {
    fn srna(&self) -> &'static StructDef {
        srna_of(self.loc)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        get(self.a, self.loc, prop)
    }
}

impl RnaMut for NodeMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        set(self.a, self.loc, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        struct_mut(self.a, self.loc, prop)
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        coll_mut(self.a, self.loc, prop)
    }
}

impl RnaColl for CollView<'_> {
    fn item_type(&self) -> &'static StructDef {
        item_type(self.c)
    }
    fn len(&self) -> usize {
        coll_len(self.a, self.c)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        coll_key(self.a, self.c, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        let loc = item_loc(self.a, self.c, i).ok_or_else(|| index_out_of_range(i, self.len()))?;
        Ok(Item::Struct(StructRef::Owned(Box::new(Node {
            a: self.a,
            loc,
        }))))
    }
}

impl RnaColl for CollViewMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        item_type(self.c)
    }
    fn len(&self) -> usize {
        coll_len(self.a, self.c)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        coll_key(self.a, self.c, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        let loc = item_loc(self.a, self.c, i).ok_or_else(|| index_out_of_range(i, self.len()))?;
        Ok(Item::Struct(StructRef::Owned(Box::new(Node {
            a: self.a,
            loc,
        }))))
    }
}

impl RnaCollMut for CollViewMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.len();
        let loc = item_loc(self.a, self.c, i).ok_or_else(|| index_out_of_range(i, len))?;
        Ok(Box::new(NodeMut {
            a: &mut *self.a,
            loc,
        }))
    }
}

// ------------------------------------------------------------------------------- reads

fn node(a: &Action, loc: Loc) -> Field<'_> {
    Field::Struct(StructRef::Owned(Box::new(Node { a, loc })))
}

/// Reads `prop` of the struct at `loc`.
pub fn get<'a>(a: &'a Action, loc: Loc, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    let id = prop.identifier;
    if let Some(c) = coll_of(loc, id) {
        return Ok(Field::Coll(CollRef::Owned(Box::new(CollView { a, c }))));
    }
    if let Some(p) = pointer_of(a, loc, id) {
        return Ok(match p? {
            Some(l) => node(a, l),
            None => Field::Value(Value::None),
        });
    }
    let srna = srna_of(loc);
    match loc {
        Loc::Action => {
            let h = &a.header;
            Ok(match id {
                "name" | "name_full" | "users" => Field::NeedsHook,
                "id_type" => Field::Value(Value::Enum("ACTION".into())),
                "session_uid" => int(i64::from(h.session_uid as i32)),
                "is_evaluated" | "is_missing" | "is_library_indirect" => boolean(false),
                "use_fake_user" => boolean(h.use_fake_user),
                "is_embedded_data" => boolean(h.is_embedded_data),
                "is_linked_packed" => boolean(h.is_linked_packed),
                "is_editable" => boolean(h.library.is_none()),
                "library" => Field::Id(h.library.map(|l| l.any())),
                "is_empty" | "is_action_legacy" => boolean(a.is_empty()),
                "is_action_layered" => boolean(true),
                "use_frame_range" => boolean(a.use_frame_range),
                "use_cyclic" => boolean(a.use_cyclic),
                "frame_start" => float(prop, a.frame_range[0]),
                "frame_end" => float(prop, a.frame_range[1]),
                "frame_range" => floats(prop, &a.frame_range_get()),
                "curve_frame_range" => floats(prop, &a.curve_frame_range()),
                "pose_markers" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(
                    frap_spec_rna::find_struct("TimelineMarker").unwrap_or(srna),
                )))),
                _ => return Err(unbound(srna, prop)),
            })
        }
        Loc::Slot(i) => {
            let s = a.slots.get(i).ok_or_else(stale)?;
            Ok(match id {
                "identifier" => string(prop, &s.identifier),
                "target_id_type" => string(prop, &s.target_id_type),
                "name_display" => string(prop, s.name_display()),
                "handle" => int(i64::from(s.handle)),
                "active" => boolean(s.active),
                "select" => boolean(s.select),
                "show_expanded" => boolean(s.show_expanded),
                _ => return Err(unbound(srna, prop)),
            })
        }
        Loc::Layer(l) => {
            let x = a.layers.get(l).ok_or_else(stale)?;
            match id {
                "name" => Ok(string(prop, &x.name)),
                _ => Err(unbound(srna, prop)),
            }
        }
        Loc::Strip(..) => match id {
            "type" => Ok(string(prop, "KEYFRAME")),
            _ => Err(unbound(srna, prop)),
        },
        Loc::Bag(l, s, b) => {
            let x = bag(a, l, s, b).ok_or_else(stale)?;
            match id {
                "slot_handle" => Ok(int(i64::from(x.slot_handle))),
                _ => Err(unbound(srna, prop)),
            }
        }
        Loc::Group(l, s, b, g) => {
            let x = bag(a, l, s, b)
                .and_then(|x| x.groups.get(g))
                .ok_or_else(stale)?;
            group_get(x, prop).unwrap_or_else(|| Err(unbound(srna, prop)))
        }
        Loc::FCurve(l, s, b, f) => {
            let fc = bag(a, l, s, b)
                .and_then(|x| x.fcurves.get(f))
                .ok_or_else(stale)?;
            fcurve_get(fc, prop).unwrap_or_else(|| Err(unbound(srna, prop)))
        }
        Loc::Key(l, s, b, f, k) => {
            let key = bag(a, l, s, b)
                .and_then(|x| x.fcurves.get(f))
                .and_then(|fc| fc.keyframe_points.get(k))
                .ok_or_else(stale)?;
            key_get(key, prop).unwrap_or_else(|| Err(unbound(srna, prop)))
        }
    }
}

fn group_get(g: &ActionGroup, prop: &'static PropDef) -> Option<Result<Field<'static>, RnaError>> {
    Some(Ok(match prop.identifier {
        "name" => string(prop, &g.name),
        "select" => boolean(g.select),
        "lock" => boolean(g.lock),
        "mute" => boolean(g.mute),
        "show_expanded" => boolean(g.show_expanded),
        "show_expanded_graph" => boolean(g.show_expanded_graph),
        "use_pin" => boolean(g.use_pin),
        "color_set" => string(prop, &g.color_set),
        "is_custom_color_set" => boolean(g.color_set == "CUSTOM"),
        _ => return None,
    }))
}

fn fcurve_get(fc: &FCurve, prop: &'static PropDef) -> Option<Result<Field<'static>, RnaError>> {
    Some(Ok(match prop.identifier {
        "extrapolation" => string(prop, fc.extrapolation.identifier()),
        "driver" => Field::Value(Value::None),
        "data_path" => string(prop, &fc.data_path),
        "array_index" => int(i64::from(fc.array_index)),
        "color_mode" => string(prop, &fc.color_mode),
        "color" => floats(prop, &fc.color),
        "select" => boolean(fc.select),
        "lock" => boolean(fc.lock),
        "mute" => boolean(fc.mute),
        "hide" => boolean(fc.hide),
        "auto_smoothing" => string(prop, &fc.auto_smoothing),
        "is_valid" => boolean(fc.is_valid),
        "is_empty" => boolean(fc.is_empty()),
        "sampled_points" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(
            &frap_spec_rna::F_CURVE_SAMPLE,
        )))),
        "modifiers" => Field::Coll(CollRef::Owned(Box::new(EmptyColl(
            &frap_spec_rna::F_MODIFIER,
        )))),
        _ => return None,
    }))
}

fn key_get(k: &Keyframe, prop: &'static PropDef) -> Option<Result<Field<'static>, RnaError>> {
    Some(Ok(match prop.identifier {
        "select_left_handle" => boolean(k.select_left_handle),
        "select_right_handle" => boolean(k.select_right_handle),
        "select_control_point" => boolean(k.select_control_point),
        "handle_left_type" => string(prop, k.handle_left_type.identifier()),
        "handle_right_type" => string(prop, k.handle_right_type.identifier()),
        "interpolation" => string(prop, k.interpolation.identifier()),
        "type" => string(prop, k.r#type.identifier()),
        "easing" => string(prop, k.easing.identifier()),
        "back" => float(prop, k.back),
        "amplitude" => float(prop, k.amplitude),
        "period" => float(prop, k.period),
        "handle_left" => floats(prop, &k.handle_left),
        "co" | "co_ui" => floats(prop, &k.co),
        "handle_right" => floats(prop, &k.handle_right),
        _ => return None,
    }))
}

// ------------------------------------------------------------------------------- writes

fn enum_of<T>(prop: &PropDef, v: &Value, f: fn(&str) -> Option<T>) -> Result<T, RnaError> {
    let s = as_str(prop, v)?;
    f(s).ok_or_else(|| {
        RnaError::new(
            RnaErrorKind::Type,
            PyExc::TypeError,
            format!("frap internal: {} has no item {s:?}", prop.identifier),
        )
    })
}

/// Writes `prop` of the struct at `loc`.
pub fn set(
    a: &mut Action,
    loc: Loc,
    prop: &'static PropDef,
    v: Value,
) -> Result<SetOutcome, RnaError> {
    let id = prop.identifier;
    let srna = srna_of(loc);
    let done = Ok(SetOutcome::Done);
    match loc {
        Loc::Action => match id {
            "name" => Ok(SetOutcome::NeedsHook),
            "use_fake_user" => {
                a.header.use_fake_user = as_bool(prop, &v)?;
                done
            }
            "use_frame_range" => {
                // `rna_Action_use_frame_range_set`: a blank range starts from the keys'.
                if as_bool(prop, &v)? {
                    if a.frame_range[0] == a.frame_range[1] && a.frame_range[0] == 0.0 {
                        a.frame_range = a.curve_frame_range();
                    }
                    a.use_frame_range = true;
                } else {
                    a.use_frame_range = false;
                }
                done
            }
            "use_cyclic" => {
                a.use_cyclic = as_bool(prop, &v)?;
                done
            }
            "frame_start" => {
                let f = as_f32(prop, &v)?;
                a.frame_range[0] = f;
                a.frame_range[1] = a.frame_range[1].max(f);
                done
            }
            "frame_end" => {
                let f = as_f32(prop, &v)?;
                a.frame_range[1] = f;
                a.frame_range[0] = a.frame_range[0].min(f);
                done
            }
            "frame_range" => {
                let r: [f32; 2] = as_array(prop, &v)?;
                a.use_frame_range = true;
                a.frame_range = [r[0], r[1].max(r[0])];
                done
            }
            _ if prop.is_readonly() => Err(read_only(srna, id)),
            _ => Err(unbound(srna, prop)),
        },
        Loc::Slot(i) => {
            let s = a.slots.get_mut(i).ok_or_else(stale)?;
            match id {
                // Renames propagate to the users' `last_slot_identifier` (hooks).
                "identifier" | "name_display" => Ok(SetOutcome::NeedsHook),
                "target_id_type" => {
                    // `rna_ActionSlot_target_id_type_set`: only an unspecified slot takes a
                    // type; otherwise Blender ignores the assignment with a printed warning.
                    if !s.has_idtype() {
                        let t = as_str(prop, &v)?;
                        s.target_id_type = t.into();
                        let h = s.handle;
                        a.slot_identifier_ensure_prefix(h);
                    }
                    done
                }
                "select" => {
                    s.select = as_bool(prop, &v)?;
                    done
                }
                "show_expanded" => {
                    s.show_expanded = as_bool(prop, &v)?;
                    done
                }
                _ if prop.is_readonly() => Err(read_only(srna, id)),
                _ => Err(unbound(srna, prop)),
            }
        }
        Loc::Layer(l) => {
            let x = a.layers.get_mut(l).ok_or_else(stale)?;
            match id {
                "name" => {
                    x.name = as_str(prop, &v)?.into();
                    done
                }
                _ if prop.is_readonly() => Err(read_only(srna, id)),
                _ => Err(unbound(srna, prop)),
            }
        }
        Loc::Strip(..) | Loc::Bag(..) => {
            if prop.is_readonly() {
                Err(read_only(srna, id))
            } else {
                Err(unbound(srna, prop))
            }
        }
        Loc::Group(l, s, b, g) => {
            let x = bag_mut(a, l, s, b).ok_or_else(stale)?;
            if g >= x.groups.len() {
                return Err(stale());
            }
            if id == "name" {
                let n = as_str(prop, &v)?.to_owned();
                x.group_rename(g, &n);
                return done;
            }
            let grp = &mut x.groups[g];
            match id {
                "select" => grp.select = as_bool(prop, &v)?,
                "lock" => grp.lock = as_bool(prop, &v)?,
                "mute" => grp.mute = as_bool(prop, &v)?,
                "show_expanded" => grp.show_expanded = as_bool(prop, &v)?,
                "show_expanded_graph" => grp.show_expanded_graph = as_bool(prop, &v)?,
                "use_pin" => grp.use_pin = as_bool(prop, &v)?,
                "color_set" => grp.color_set = as_str(prop, &v)?.into(),
                _ if prop.is_readonly() => return Err(read_only(srna, id)),
                _ => return Err(unbound(srna, prop)),
            }
            done
        }
        Loc::FCurve(l, s, b, f) => {
            let x = bag_mut(a, l, s, b).ok_or_else(stale)?;
            if f >= x.fcurves.len() {
                return Err(stale());
            }
            if id == "group" {
                // `rna_FCurve_group_set`: a group of the same channelbag; anything else
                // (None, another channelbag's group) is ignored with a printed error.
                if let Value::Ptr(p) = &v
                    && let Some(name) = match p.path.last() {
                        Some(Seg::Key(k)) => Some(k.clone()),
                        Some(Seg::Index(i)) => usize::try_from(*i)
                            .ok()
                            .and_then(|i| x.groups.get(i))
                            .map(|g| g.name.clone()),
                        _ => None,
                    }
                    && let Some(g) = x.group_find(&name)
                {
                    x.fcurve_assign_to_group(f, g);
                }
                return done;
            }
            let fc = &mut x.fcurves[f];
            match id {
                "extrapolation" => {
                    fc.extrapolation = enum_of(prop, &v, Extrapolation::from_identifier)?;
                    keys::update(fc);
                }
                "auto_smoothing" => {
                    fc.auto_smoothing = as_str(prop, &v)?.into();
                    keys::update(fc);
                }
                "data_path" => {
                    // `rna_FCurve_RnaPath_set`: a new path clears `FCURVE_DISABLED`.
                    let p = as_str(prop, &v)?;
                    if !p.is_empty() {
                        fc.is_valid = true;
                    }
                    fc.data_path = p.into();
                }
                "array_index" => {
                    fc.array_index = i32::try_from(as_i64(prop, &v)?).unwrap_or(i32::MAX);
                }
                "color_mode" => fc.color_mode = as_str(prop, &v)?.into(),
                "color" => fc.color = as_array(prop, &v)?,
                "select" => fc.select = as_bool(prop, &v)?,
                "lock" => fc.lock = as_bool(prop, &v)?,
                "mute" => fc.mute = as_bool(prop, &v)?,
                "hide" => fc.hide = as_bool(prop, &v)?,
                "is_valid" => fc.is_valid = as_bool(prop, &v)?,
                _ if prop.is_readonly() => return Err(read_only(srna, id)),
                _ => return Err(unbound(srna, prop)),
            }
            done
        }
        Loc::Key(l, s, b, f, k) => {
            let key = bag_mut(a, l, s, b)
                .and_then(|x| x.fcurves.get_mut(f))
                .and_then(|fc| fc.keyframe_points.get_mut(k))
                .ok_or_else(stale)?;
            match id {
                "select_left_handle" => key.select_left_handle = as_bool(prop, &v)?,
                "select_right_handle" => key.select_right_handle = as_bool(prop, &v)?,
                "select_control_point" => key.select_control_point = as_bool(prop, &v)?,
                "handle_left_type" => {
                    key.handle_left_type = enum_of(prop, &v, HandleType::from_identifier)?;
                }
                "handle_right_type" => {
                    key.handle_right_type = enum_of(prop, &v, HandleType::from_identifier)?;
                }
                "interpolation" => {
                    key.interpolation = enum_of(prop, &v, Interpolation::from_identifier)?;
                }
                "type" => key.r#type = enum_of(prop, &v, KeyframeType::from_identifier)?,
                "easing" => key.easing = enum_of(prop, &v, Easing::from_identifier)?,
                "back" => key.back = as_f32(prop, &v)?,
                "amplitude" => key.amplitude = as_f32(prop, &v)?,
                "period" => key.period = as_f32(prop, &v)?,
                "handle_left" => key.handle_left = as_array(prop, &v)?,
                "handle_right" => key.handle_right = as_array(prop, &v)?,
                "co" => key.co = as_array(prop, &v)?,
                "co_ui" => {
                    // `rna_FKeyframe_ctrlpoint_ui_set`: the handles move with the key.
                    let c: [f32; 2] = as_array(prop, &v)?;
                    let d = [c[0] - key.co[0], c[1] - key.co[1]];
                    key.handle_left = [key.handle_left[0] + d[0], key.handle_left[1] + d[1]];
                    key.co = c;
                    key.handle_right = [key.handle_right[0] + d[0], key.handle_right[1] + d[1]];
                }
                _ if prop.is_readonly() => return Err(read_only(srna, id)),
                _ => return Err(unbound(srna, prop)),
            }
            done
        }
    }
}

/// Mutable access to the nested struct `prop` of `loc`.
pub fn struct_mut<'a>(
    a: &'a mut Action,
    loc: Loc,
    prop: &'static PropDef,
) -> Result<RnaMutGuard<'a>, RnaError> {
    match pointer_of(a, loc, prop.identifier) {
        Some(Ok(Some(l))) => Ok(Box::new(NodeMut { a, loc: l })),
        Some(Ok(None)) => Err(RnaError::new(
            RnaErrorKind::Other,
            PyExc::AttributeError,
            format!("'NoneType' object has no attribute of {}", prop.identifier),
        )),
        Some(Err(e)) => Err(e),
        None => Err(wrong_access(srna_of(loc), prop.identifier, "a struct")),
    }
}

/// Mutable access to the collection `prop` of `loc`.
pub fn coll_mut<'a>(
    a: &'a mut Action,
    loc: Loc,
    prop: &'static PropDef,
) -> Result<RnaCollMutGuard<'a>, RnaError> {
    if let Some(c) = coll_of(loc, prop.identifier) {
        return Ok(Box::new(CollViewMut { a, c }));
    }
    match prop.identifier {
        "pose_markers" | "sampled_points" | "modifiers" => Ok(Box::new(EmptyColl(
            frap_spec_rna::find_struct("FModifier").unwrap_or(&frap_spec_rna::F_MODIFIER),
        ))),
        id => Err(wrong_access(srna_of(loc), id, "a collection")),
    }
}

/// Every property identifier these bindings serve, per struct (checked against the generated
/// definitions in the tests).
pub const BOUND: &[(&str, &[&str])] = &[
    (
        "Action",
        &[
            "name",
            "name_full",
            "users",
            "id_type",
            "session_uid",
            "is_evaluated",
            "is_missing",
            "is_library_indirect",
            "use_fake_user",
            "is_embedded_data",
            "is_linked_packed",
            "is_editable",
            "library",
            "is_empty",
            "is_action_legacy",
            "is_action_layered",
            "use_frame_range",
            "use_cyclic",
            "frame_start",
            "frame_end",
            "frame_range",
            "curve_frame_range",
            "pose_markers",
            "slots",
            "layers",
        ],
    ),
    (
        "ActionSlot",
        &[
            "identifier",
            "target_id_type",
            "name_display",
            "handle",
            "active",
            "select",
            "show_expanded",
        ],
    ),
    ("ActionLayer", &["name", "strips"]),
    ("ActionKeyframeStrip", &["type", "channelbags"]),
    (
        "ActionChannelbag",
        &["slot_handle", "slot", "fcurves", "groups"],
    ),
    (
        "ActionGroup",
        &[
            "name",
            "channels",
            "select",
            "lock",
            "mute",
            "show_expanded",
            "show_expanded_graph",
            "use_pin",
            "color_set",
            "is_custom_color_set",
        ],
    ),
    (
        "FCurve",
        &[
            "extrapolation",
            "driver",
            "group",
            "data_path",
            "array_index",
            "color_mode",
            "color",
            "select",
            "lock",
            "mute",
            "hide",
            "auto_smoothing",
            "is_valid",
            "is_empty",
            "sampled_points",
            "keyframe_points",
            "modifiers",
        ],
    ),
    (
        "Keyframe",
        &[
            "select_left_handle",
            "select_right_handle",
            "select_control_point",
            "handle_left_type",
            "handle_right_type",
            "interpolation",
            "type",
            "easing",
            "back",
            "amplitude",
            "period",
            "handle_left",
            "co",
            "co_ui",
            "handle_right",
        ],
    ),
];
