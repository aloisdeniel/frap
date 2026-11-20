// SPDX-License-Identifier: GPL-3.0-or-later
//! Edits of an F-curve's keyframe array, behind `FCurveKeyframePoints.insert/add/remove/
//! clear/sort/deduplicate/handles_recalc` and `FCurve.update()`.
//!
//! Ported from blender/source/blender/animrig/intern/fcurve.cc (`insert_vert_fcurve`,
//! `insert_bezt_fcurve`, `initialize_bezt`, `subdivide_nonauto_handles`, `new_key_needed`),
//! blender/source/blender/blenkernel/intern/fcurve.cc (`sort_time_fcurve`,
//! `BKE_fcurve_deduplicate_keys`, `BKE_fcurve_bezt_subdivide_handles`,
//! `BKE_fcurve_delete_key`, `BKE_fcurve_calc_range`) and
//! blender/source/blender/editors/animation/keyframing.cc (`ED_keyframes_add`), Blender 5.2.4,
//! GPL-2.0-or-later.

use super::{FCurve, HandleType, Interpolation, Keyframe, KeyframeType};
use crate::eval::curve::{BEZT_BINARYSEARCH_THRESH, binarysearch_index, correct_bezpart, findzero};
use crate::eval::{evaluate_fcurve, recalc_handles};

/// `eInsertKeyFlags` the API exposes (`keyframe_points.insert(options=…)`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct InsertFlags {
    /// `REPLACE` (`INSERTKEY_REPLACE`): only replace existing keys.
    pub replace: bool,
    /// `NEEDED` (`INSERTKEY_NEEDED`): only insert where the curve changes.
    pub needed: bool,
    /// `FAST` (`INSERTKEY_FAST`): skip the handle recalculation.
    pub fast: bool,
}

/// `KeyframeSettings`
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct KeyframeSettings {
    /// `keyframe_type`
    pub keyframe_type: KeyframeType,
    /// `handle`
    pub handle: HandleType,
    /// `interpolation`
    pub interpolation: Interpolation,
}

impl Default for KeyframeSettings {
    /// `get_keyframe_settings(false)` (and the factory preferences): KEYFRAME, AUTO_CLAMPED,
    /// BEZIER.
    fn default() -> Self {
        KeyframeSettings {
            keyframe_type: KeyframeType::Keyframe,
            handle: HandleType::AutoClamped,
            interpolation: Interpolation::Bezier,
        }
    }
}

/// `SingleKeyingResult` (the outcomes the data API distinguishes).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InsertResult {
    /// The key was inserted or replaced, at this index.
    Success(usize),
    /// `NEEDED` and the curve already has the value there.
    NoKeyNeeded,
    /// `REPLACE` and no key to replace.
    Failure,
}

/// `initialize_bezt`
pub fn initialize_bezt(position: [f32; 2], settings: &KeyframeSettings, fc: &FCurve) -> Keyframe {
    let (x, y) = (position[0], position[1]);
    let mut interpolation = settings.interpolation;
    if fc.discrete_values {
        interpolation = Interpolation::Constant;
    } else if interpolation == Interpolation::Bezier && fc.int_values {
        interpolation = Interpolation::Linear;
    }
    Keyframe {
        co: [x, y],
        handle_left: [x - 1.0, y],
        handle_right: [x + 1.0, y],
        handle_left_type: settings.handle,
        handle_right_type: settings.handle,
        interpolation,
        r#type: settings.keyframe_type,
        ..Keyframe::new(x, y)
    }
}

/// `replace_bezt_keyframe_ypos`: moves the existing key (and its handles) to the new value.
fn replace_ypos(dst: &mut Keyframe, src: &Keyframe) {
    let dy = src.co[1] - dst.co[1];
    dst.handle_left[1] += dy;
    dst.co[1] += dy;
    dst.handle_right[1] += dy;
    dst.select_left_handle = src.select_left_handle;
    dst.select_control_point = src.select_control_point;
    dst.select_right_handle = src.select_right_handle;
}

/// `insert_bezt_fcurve` (no full overwrite, not cycle-aware): the index of the new or replaced
/// key, `None` when `replace` found nothing to replace.
fn insert_bezt(fc: &mut FCurve, bezt: Keyframe, flags: InsertFlags) -> Option<usize> {
    if fc.keyframe_points.is_empty() {
        if flags.replace {
            return None;
        }
        fc.keyframe_points.push(bezt);
        return Some(0);
    }
    let (i, replace) = binarysearch_index(&fc.keyframe_points, bezt.co[0]);
    if replace {
        if i < fc.keyframe_points.len() {
            replace_ypos(&mut fc.keyframe_points[i], &bezt);
        }
        return Some(i);
    }
    if flags.replace {
        return None;
    }
    fc.keyframe_points.insert(i, bezt);
    Some(i)
}

/// `BKE_fcurve_bezt_subdivide_handles`: splits the segment `prev..next` at the new key's time
/// (De Casteljau) so the curve keeps its shape.
fn subdivide_handles(bezt: &mut Keyframe, prev: &mut Keyframe, next: &mut Keyframe) -> bool {
    let prev_coords = prev.co;
    let next_coords = next.co;
    let new_coords = bezt.co;
    if new_coords[0] <= prev_coords[0] || new_coords[0] >= next_coords[0] {
        return false;
    }
    correct_bezpart(
        prev_coords,
        &mut prev.handle_right,
        &mut next.handle_left,
        next_coords,
    );
    let mut roots = [0.0_f32; 3];
    if findzero(
        new_coords[0],
        prev_coords[0],
        prev.handle_right[0],
        next.handle_left[0],
        next_coords[0],
        &mut roots,
    ) == 0
    {
        return false;
    }
    let t = roots[0];
    if t <= 0.0 || t >= 1.0 {
        return false;
    }
    let lerp = |a: [f32; 2], b: [f32; 2]| {
        let s = 1.0 - t;
        [s * a[0] + t * b[0], s * a[1] + t * b[1]]
    };
    let s10 = lerp(prev_coords, prev.handle_right);
    let s11 = lerp(prev.handle_right, next.handle_left);
    let s12 = lerp(next.handle_left, next_coords);
    let s20 = lerp(s10, s11);
    let s21 = lerp(s11, s12);
    let s3 = lerp(s20, s21);
    prev.handle_right = s10;
    next.handle_left = s12;
    let diff = [new_coords[0] - s3[0], new_coords[1] - s3[1]];
    bezt.handle_left = [s20[0] + diff[0], s20[1] + diff[1]];
    bezt.handle_right = [s21[0] + diff[0], s21[1] + diff[1]];
    true
}

/// `subdivide_nonauto_handles`: keeps the curve shape when a key lands between keys with
/// manual handles (and turns the new key's auto handles to ALIGNED when needed).
fn subdivide_nonauto(fc: &mut FCurve, a: usize) {
    let smooth = fc.auto_smoothing == "CONT_ACCEL";
    let (left, rest) = fc.keyframe_points.split_at_mut(a);
    let (mid, right) = rest.split_at_mut(1);
    let (prev, bezt, next) = (&mut left[a - 1], &mut mid[0], &mut right[0]);
    if prev.interpolation != Interpolation::Bezier || bezt.interpolation != Interpolation::Bezier {
        return;
    }
    // Don't change vector handles, or completely auto regions.
    let vect = HandleType::Vector;
    let bezt_auto =
        bezt.is_auto_h() || (bezt.handle_left_type == vect && bezt.handle_right_type == vect);
    let prev_auto = prev.is_auto_h() || prev.handle_right_type == vect;
    let next_auto = next.is_auto_h() || next.handle_left_type == vect;
    if bezt_auto && prev_auto && next_auto {
        return;
    }
    if !subdivide_handles(bezt, prev, next) {
        return;
    }
    if !bezt.is_auto_h() {
        return;
    }
    if (prev_auto || next_auto) && smooth {
        let hx = bezt.co[0] - bezt.handle_left[0];
        let dx = bezt.co[0] - prev.co[0];
        // This mode always uses a third of the key distance for the handle's x size.
        if (hx - dx / 3.0).abs() < 0.001 {
            return;
        }
    }
    bezt.handle_left_type = HandleType::Aligned;
    bezt.handle_right_type = HandleType::Aligned;
}

/// `ulp_diff_ff`
fn ulp_diff_ff(a: f32, b: f32) -> u32 {
    const SIGN: u32 = 0x8000_0000;
    const INF: u32 = 0x7f80_0000;
    let (ua, ub) = (a.to_bits(), b.to_bits());
    let (a_abs, b_abs) = (ua & !SIGN, ub & !SIGN);
    if a_abs > INF || b_abs > INF {
        return u32::MAX;
    }
    if (ua & SIGN) == (ub & SIGN) {
        a_abs.max(b_abs) - a_abs.min(b_abs)
    } else {
        a_abs.wrapping_add(b_abs)
    }
}

/// `compare_ff_relative`
fn compare_ff_relative(a: f32, b: f32, max_diff: f32, max_ulps: u32) -> bool {
    (a - b).abs() <= max_diff || ulp_diff_ff(a, b) <= max_ulps
}

/// `new_key_needed`
fn new_key_needed(fc: &FCurve, frame: f32, value: f32) -> bool {
    if fc.keyframe_points.is_empty() {
        return true;
    }
    let (i, replace) = binarysearch_index(&fc.keyframe_points, frame);
    if replace {
        return fc.keyframe_points[i].co[1] != value;
    }
    !compare_ff_relative(evaluate_fcurve(fc, frame), value, f32::EPSILON, 32)
}

/// `insert_vert_fcurve` (not cycle-aware).
pub fn insert_vert(
    fc: &mut FCurve,
    position: [f32; 2],
    settings: &KeyframeSettings,
    flags: InsertFlags,
) -> InsertResult {
    if flags.needed && !new_key_needed(fc, position[0], position[1]) {
        return InsertResult::NoKeyNeeded;
    }
    let beztr = initialize_bezt(position, settings, fc);
    let old_tot = fc.keyframe_points.len();
    let Some(a) = insert_bezt(fc, beztr, flags) else {
        return InsertResult::Failure;
    };
    let n = fc.keyframe_points.len();
    if n > 2 && !flags.replace && n > old_tot {
        // Interpolation from the previous key (or the next, for a new first key).
        let ipo = if a > 0 {
            Some(fc.keyframe_points[a - 1].interpolation)
        } else if a < n - 1 {
            Some(fc.keyframe_points[a + 1].interpolation)
        } else {
            None
        };
        if let Some(ipo) = ipo {
            fc.keyframe_points[a].interpolation = ipo;
        }
        if 0 < a && a < n - 1 {
            subdivide_nonauto(fc, a);
        }
    }
    if !flags.fast {
        recalc_handles(fc);
    }
    InsertResult::Success(a)
}

/// `ED_keyframes_add`: appends `count` zero keys (unsorted, handles not recalculated).
pub fn add_zeroed(fc: &mut FCurve, count: usize) {
    fc.keyframe_points
        .extend(std::iter::repeat_with(Keyframe::zeroed).take(count));
}

/// `sort_time_fcurve`: bubble-sorts the keys by time, then swaps handles that crossed sides
/// and clamps the others to their key.
pub fn sort_time(fc: &mut FCurve) {
    let keys = &mut fc.keyframe_points;
    let mut ok = true;
    while ok {
        ok = false;
        for a in 0..keys.len().saturating_sub(1) {
            if keys[a].co[0] > keys[a + 1].co[0] {
                keys.swap(a, a + 1);
                ok = true;
            }
        }
    }
    for b in keys.iter_mut() {
        if b.handle_left[0] > b.co[0] && b.handle_right[0] < b.co[0] {
            std::mem::swap(&mut b.handle_left, &mut b.handle_right);
        } else {
            b.handle_left[0] = b.handle_left[0].min(b.co[0]);
            b.handle_right[0] = b.handle_right[0].max(b.co[0]);
        }
    }
}

/// `BKE_fcurve_keyframe_move_time_with_handles`
fn move_time_with_handles(k: &mut Keyframe, new_time: f32) {
    let dx = new_time - k.co[0];
    k.co[0] += dx;
    k.handle_left[0] += dx;
    k.handle_right[0] += dx;
}

/// `BKE_fcurve_deduplicate_keys`: of keys closer than [`BEZT_BINARYSEARCH_THRESH`] the last
/// wins (keeping the earlier time unless the last one sits on a whole frame).
pub fn deduplicate(fc: &mut FCurve) {
    let keys = &mut fc.keyframe_points;
    if keys.len() < 2 {
        return;
    }
    let mut prev = 0;
    for i in 1..keys.len() {
        let bezt_x = keys[i].co[0];
        let prev_x = keys[prev].co[0];
        if bezt_x - prev_x <= BEZT_BINARYSEARCH_THRESH {
            keys[prev] = keys[i].clone();
            if bezt_x.floor() != bezt_x {
                move_time_with_handles(&mut keys[prev], prev_x);
            }
            continue;
        }
        prev += 1;
        if prev != i {
            keys[prev] = keys[i].clone();
        }
    }
    keys.truncate(prev + 1);
}

/// `FCurve.update()` (`rna_FCurve_update_data_ex`): sort, deduplicate, recalculate handles.
pub fn update(fc: &mut FCurve) {
    sort_time(fc);
    deduplicate(fc);
    recalc_handles(fc);
}

/// `BKE_fcurve_calc_range(fcu, …, selected_keys_only = false)`: the first and last key times.
pub fn range(fc: &FCurve) -> Option<[f32; 2]> {
    Some([fc.keyframe_points.first()?.co[0], fc.keyframe_points.last()?.co[0]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fc() -> FCurve {
        FCurve::new("location", 0)
    }

    /// Verified on 5.2.4: `insert(1, 0)`, `insert(10, 5)` → handles (-2, 0) (4, 0) and (7, 5)
    /// (13, 5); `add(2)` appends two zero keys.
    #[test]
    fn insert_and_add() {
        let mut f = fc();
        let s = KeyframeSettings::default();
        assert_eq!(
            insert_vert(&mut f, [1.0, 0.0], &s, InsertFlags::default()),
            InsertResult::Success(0)
        );
        assert_eq!(
            insert_vert(&mut f, [10.0, 5.0], &s, InsertFlags::default()),
            InsertResult::Success(1)
        );
        let k = &f.keyframe_points;
        assert_eq!((k[0].handle_left, k[0].handle_right), ([-2.0, 0.0], [4.0, 0.0]));
        assert_eq!((k[1].handle_left, k[1].handle_right), ([7.0, 5.0], [13.0, 5.0]));
        // Replacing moves the key and its handles.
        assert_eq!(
            insert_vert(&mut f, [10.0, 6.0], &s, InsertFlags::default()),
            InsertResult::Success(1)
        );
        assert_eq!(f.keyframe_points[1].co, [10.0, 6.0]);
        let flags = InsertFlags {
            replace: true,
            ..Default::default()
        };
        assert_eq!(
            insert_vert(&mut f, [5.0, 6.0], &s, flags),
            InsertResult::Failure
        );
        let needed = InsertFlags {
            needed: true,
            ..Default::default()
        };
        assert_eq!(
            insert_vert(&mut f, [10.0, 6.0], &s, needed),
            InsertResult::NoKeyNeeded
        );
        add_zeroed(&mut f, 2);
        assert_eq!(f.keyframe_points.len(), 4);
        assert_eq!(f.keyframe_points[3].co, [0.0, 0.0]);
        assert_eq!(f.keyframe_points[3].back, 0.0);
        update(&mut f);
        // Two zero keys at frame 0 collapse into one, sorted first.
        assert_eq!(f.keyframe_points.len(), 3);
        assert_eq!(f.keyframe_points[0].co, [0.0, 0.0]);
        assert_eq!(range(&f), Some([0.0, 10.0]));
    }

    #[test]
    fn int_and_discrete_curves() {
        let mut f = fc();
        f.int_values = true;
        let k = initialize_bezt([1.0, 2.0], &KeyframeSettings::default(), &f);
        assert_eq!(k.interpolation, Interpolation::Linear);
        f.discrete_values = true;
        let k = initialize_bezt([1.0, 2.0], &KeyframeSettings::default(), &f);
        assert_eq!(k.interpolation, Interpolation::Constant);
    }
}
