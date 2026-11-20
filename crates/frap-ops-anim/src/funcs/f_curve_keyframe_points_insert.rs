// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.insert(frame, value, options=set(), keyframe_type='KEYFRAME')`
//! (`rna_FKeyframe_points_insert` → `insert_vert_fcurve` with `get_keyframe_settings(false)`,
//! Blender 5.2.4): a BEZIER key with AUTO_CLAMPED handles (CONSTANT on discrete curves, LINEAR
//! on integer ones), a new key between others taking its left neighbour's interpolation, then
//! the handles recalculated. A key within 0.01 frame of an existing one replaces its value
//! (handles move with it). `options`: `REPLACE` only replaces, `NEEDED` skips a key the curve
//! already has, `FAST` skips the handle recalculation. Returns the key, `None` when nothing
//! was inserted. Verified on 5.2.4: `insert(1, 0)`, `insert(10, 5)` → handles (-2, 0) (4, 0)
//! and (7, 5) (13, 5).

use frap_anim::KeyframeType;
use frap_anim::fcurve::keys::{InsertFlags, InsertResult, KeyframeSettings, insert_vert};
use frap_anim::rna::action::Loc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{
    action_of, arg_f32, arg_flags, arg_str, loc_ptr, removed, with_fcurve,
};

#[rna_func(srna = "FCurveKeyframePoints", func = "insert")]
fn f_curve_keyframe_points_insert(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let frame = arg_f32(args, "frame", 0.0);
    let value = arg_f32(args, "value", 0.0);
    let options = arg_flags(args, "options");
    let flags = InsertFlags {
        replace: options.iter().any(|o| o == "REPLACE"),
        needed: options.iter().any(|o| o == "NEEDED"),
        fast: options.iter().any(|o| o == "FAST"),
    };
    let settings = KeyframeSettings {
        keyframe_type: KeyframeType::from_identifier(arg_str(args, "keyframe_type"))
            .unwrap_or_default(),
        ..KeyframeSettings::default()
    };
    let a = action_of(this)?;
    let key = with_fcurve(h, this, |fc, loc| {
        let r = insert_vert(fc, [frame, value], &settings, flags);
        let Loc::FCurve(l, s, b, f) = loc else {
            return Ok(None);
        };
        Ok(match r {
            InsertResult::Success(_) => {
                let (i, _) = frap_anim::eval::curve::binarysearch_index(&fc.keyframe_points, frame);
                Some(Loc::Key(l, s, b, f, i))
            }
            _ => None,
        })
    })?;
    let act = h.main().actions.get(a).ok_or_else(|| removed("Action"))?;
    Ok(key.map_or(Value::None, |k| loc_ptr(a, act, k)))
}
