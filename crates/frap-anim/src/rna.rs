// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA bindings of the animation data (WP 6.1): `Action` (slots, layers, strips, channelbags,
//! groups, F-curves, keyframe points; `Action.fcurves` → `RNA_REMOVED_IN_5X`, answered by the
//! resolver's compatibility table), `AnimData`, `NlaTrack`, `NlaStrip`.
//!
//! Everything below an action is reached through index proxies over the whole action
//! ([`action::Node`] / [`action::NodeMut`], located by [`action::Loc`]): an F-curve's `group`, a
//! channelbag's `slot` and a group's `channels` live elsewhere in the same action, so a proxy
//! holding the action can answer them, mutably too. `AnimData` has the same scheme
//! ([`anim_data::AdtLoc`]); since the ID bindings do not bind `animation_data`, frap-scene's
//! `ID.animation_data` get hook serves it and its set hooks write through
//! [`anim_data::AdtMut`].
//!
//! Setters with side effects follow Blender 5.2.4 (`makesrna/intern/rna_action.cc`,
//! `rna_fcurve.cc`, `rna_animation.cc`, `rna_nla.cc`): `FCurve.extrapolation` and
//! `auto_smoothing` run `rna_FCurve_update_data` (sort, deduplicate, recalculate handles),
//! `Keyframe.co_ui` moves the handles with the key, `Action.frame_start`/`frame_end` keep the
//! range ordered. Setters that need the rest of `Main` (slot identifiers propagate to their
//! users, action assignment) answer `SetOutcome::NeedsHook`.

use frap_rna::traits::{self, ToRnaValue};
use frap_rna::*;
use smol_str::SmolStr;

use crate::action::Action;

pub mod action;
pub mod anim_data;

/// `Action` property reads.
pub(crate) fn action_get<'a>(a: &'a Action, prop: &'static PropDef) -> Result<Field<'a>, RnaError> {
    action::get(a, action::Loc::Action, prop)
}

/// `Action` property writes.
pub(crate) fn action_set(
    a: &mut Action,
    prop: &'static PropDef,
    v: Value,
) -> Result<SetOutcome, RnaError> {
    action::set(a, action::Loc::Action, prop, v)
}

/// `Action` nested structs.
pub(crate) fn action_struct_mut<'a>(
    a: &'a mut Action,
    prop: &'static PropDef,
) -> Result<RnaMutGuard<'a>, RnaError> {
    action::struct_mut(a, action::Loc::Action, prop)
}

/// `Action` collections.
pub(crate) fn action_coll_mut<'a>(
    a: &'a mut Action,
    prop: &'static PropDef,
) -> Result<RnaCollMutGuard<'a>, RnaError> {
    action::coll_mut(a, action::Loc::Action, prop)
}

// ------------------------------------------------------------------------------- helpers

/// A value of `prop` from a scalar float.
pub(crate) fn float(prop: &PropDef, f: f32) -> Field<'static> {
    Field::Value(ToRnaValue::to_rna_value(&f, prop, MathTag::None))
}

/// A float array of `prop`.
pub(crate) fn floats(prop: &PropDef, data: &[f32]) -> Field<'static> {
    Field::Value(Value::FloatArray {
        data: data.iter().copied().collect(),
        shape: prop.array,
        tag: MathTag::for_subtype(prop.subtype),
    })
}

/// `Value::Bool`
pub(crate) fn boolean(b: bool) -> Field<'static> {
    Field::Value(Value::Bool(b))
}

/// `Value::Int`
pub(crate) fn int(i: i64) -> Field<'static> {
    Field::Value(Value::Int(i))
}

/// A string or enum value of `prop`.
pub(crate) fn string(prop: &PropDef, s: &str) -> Field<'static> {
    Field::Value(match prop.kind {
        PropKind::Enum { .. } => Value::Enum(s.into()),
        _ => Value::Str(s.into()),
    })
}

/// The coerced value as a float.
pub(crate) fn as_f32(prop: &PropDef, v: &Value) -> Result<f32, RnaError> {
    v.as_f32().ok_or_else(|| bad_value(prop, v))
}

/// The coerced value as a bool.
pub(crate) fn as_bool(prop: &PropDef, v: &Value) -> Result<bool, RnaError> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Int(i) => Ok(*i != 0),
        _ => Err(bad_value(prop, v)),
    }
}

/// The coerced value as an int.
pub(crate) fn as_i64(prop: &PropDef, v: &Value) -> Result<i64, RnaError> {
    match v {
        Value::Int(i) => Ok(*i),
        Value::Bool(b) => Ok(i64::from(*b)),
        _ => Err(bad_value(prop, v)),
    }
}

/// The coerced value as a string or enum identifier.
pub(crate) fn as_str<'v>(prop: &PropDef, v: &'v Value) -> Result<&'v str, RnaError> {
    v.as_str().ok_or_else(|| bad_value(prop, v))
}

/// The coerced value as `[f32; N]`.
pub(crate) fn as_array<const N: usize>(prop: &PropDef, v: &Value) -> Result<[f32; N], RnaError> {
    match v {
        Value::FloatArray { data, .. } if data.len() == N => {
            let mut out = [0.0; N];
            out.copy_from_slice(data);
            Ok(out)
        }
        _ => Err(bad_value(prop, v)),
    }
}

fn bad_value(prop: &PropDef, v: &Value) -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        format!("frap internal: {} got {v:?}", prop.identifier),
    )
}

/// A property of the struct frap does not bind (`RNA_NOT_IMPLEMENTED` through the resolver).
pub(crate) fn unbound(srna: &'static StructDef, prop: &PropDef) -> RnaError {
    traits::no_attribute(srna, prop.identifier)
}

/// An empty, read-only collection (`FCurve.modifiers`, `sampled_points`, `Action.pose_markers`,
/// `AnimData.drivers`: frap has no F-modifiers, samples, pose markers or drivers yet).
pub(crate) struct EmptyColl(pub &'static StructDef);

impl RnaColl for EmptyColl {
    fn item_type(&self) -> &'static StructDef {
        self.0
    }
    fn len(&self) -> usize {
        0
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        Err(traits::index_out_of_range(i, 0))
    }
}

impl RnaCollMut for EmptyColl {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(traits::index_out_of_range(i, 0))
    }
}

#[cfg(test)]
mod tests;
