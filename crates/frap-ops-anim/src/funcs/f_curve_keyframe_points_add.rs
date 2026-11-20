// SPDX-License-Identifier: GPL-3.0-or-later
//! `fcurve.keyframe_points.add(count)` (`rna_FKeyframe_points_add` → `ED_keyframes_add`,
//! Blender 5.2.4): appends `count` keys at (0, 0) with their handles at (0, 0), BEZIER,
//! AUTO_CLAMPED, selected, `back`/`amplitude`/`period` 0, without sorting or recalculating the
//! handles (scripts set `co` and call `fcurve.update()`). `count <= 0` adds nothing.
//!
//! The RNA range goes up to `INT_MAX`; frap checks the keys' memory against the statement's
//! `--max-elements` budget first, so an absurd count fails at once with `LIMIT_EXCEEDED`
//! (deviation D-52) instead of exhausting the process.

use frap_anim::Keyframe;
use frap_anim::fcurve::keys;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::{arg_int, with_fcurve};

#[rna_func(srna = "FCurveKeyframePoints", func = "add")]
fn f_curve_keyframe_points_add(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let count = arg_int(args, "count", 1);
    if count <= 0 {
        return Ok(Value::None);
    }
    frap_math::budget::check_buffer("keyframe_points", key_bytes(count));
    with_fcurve(h, this, |fc, _| {
        keys::add_zeroed(fc, usize::try_from(count).unwrap_or(0));
        Ok(Value::None)
    })
}

/// The memory `count` new keys take (`None` on overflow).
fn key_bytes(count: i64) -> Option<u64> {
    u64::try_from(count)
        .ok()?
        .checked_mul(std::mem::size_of::<Keyframe>() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_math::budget::{Budget, reserve_buffer, scope};

    #[test]
    fn absurd_counts_exceed_the_default_budget() {
        // The CLI's default `--max-elements`.
        let _s = scope(Budget {
            max_elements: 50_000_000,
            ..Budget::default()
        });
        assert!(reserve_buffer("keyframe_points", key_bytes(1_000_000)).is_ok());
        let e = reserve_buffer("keyframe_points", key_bytes(2_000_000_000)).expect_err("2e9");
        assert_eq!(e.buffer, Some("keyframe_points"));
    }
}
