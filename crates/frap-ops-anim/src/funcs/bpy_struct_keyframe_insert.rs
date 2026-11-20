// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy_struct.keyframe_insert(data_path, *, index=-1, frame=None, group="", options=set(),
//! keytype='KEYFRAME') -> bool` (`pyrna_struct_keyframe_insert`,
//! blender/source/blender/python/intern/bpy_rna_anim.cc, Blender 5.2.4): validates the path
//! (`TypeError`/`ValueError` as Blender words them), then `animrig::insert_keyframes` on the
//! owning ID with `INSERTKEY_NO_USERPREF` (BEZIER, AUTO_CLAMPED). `frame=None` keys at the
//! context scene's `frame_current`. When no key was inserted the combined result becomes a
//! WARNING report and the call returns `False` (no exception). An `NlaStrip`'s own F-curves
//! (animated influence / strip time) are not modelled: keying them returns `False`.
//!
//! Values are the originals as they are now: after `frame_current = 10` without an evaluation
//! boundary, the animated channels still hold their old values (§7.3, verified).

use frap_anim::KeyframeType;
use frap_rna::{Report, ReportType, RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use crate::keying::{HostKeying, KeyFlags, KeyPath, KeyingResult, insert_keyframes};

/// The value of a parameter the caller passed (`None` when left at its default).
pub(crate) fn passed<'a>(args: &'a FuncArgs, name: &str) -> Option<&'a Value> {
    let i = args.values.get_index_of(name)?;
    args.is_set
        .contains(i)
        .then(|| args.values.get(name))
        .flatten()
}

/// `frame`, or the context scene's `frame_current` for `None`.
pub(crate) fn frame_arg(h: &dyn Host, args: &FuncArgs) -> f32 {
    match passed(args, "frame").and_then(Value::as_f32) {
        Some(f) => f,
        None => h
            .main()
            .scenes
            .get(h.win().scene)
            .map_or(0.0, |s| s.frame_current as f32),
    }
}

#[rna_func(srna = "bpy_struct", func = "keyframe_insert")]
fn bpy_struct_keyframe_insert(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let prefix = "bpy_struct.keyframe_insert()";
    let data_path = args
        .get("data_path")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let index = match args.get("index") {
        Some(Value::Int(i)) => i32::try_from(*i).unwrap_or(i32::MAX),
        _ => -1,
    };
    let group = passed(args, "group")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let options: Vec<smol_str::SmolStr> = match args.get("options") {
        Some(Value::EnumSet(s)) => s.iter().cloned().collect(),
        _ => Vec::new(),
    };
    let keytype = args
        .get("keytype")
        .and_then(Value::as_str)
        .and_then(KeyframeType::from_identifier)
        .unwrap_or(KeyframeType::Keyframe);
    let parsed = crate::keying::parse_struct_args(&*h, this, prefix, &data_path, index)?;
    let frame = frame_arg(h, args);
    if this.srna.is_a("NlaStrip") {
        return Ok(Value::Bool(false));
    }
    let path = KeyPath {
        path: parsed.path,
        index: Some(parsed.index),
    };
    let result = insert_keyframes(
        &mut HostKeying(h),
        parsed.root,
        group.as_deref(),
        &[path],
        frame,
        keytype,
        KeyFlags::from_api(&options),
    );
    let ok = result.count(KeyingResult::Success) > 0;
    if !ok {
        let (level, msg) = result.report(ReportType::Warning);
        h.report(Report::new(level, msg));
    }
    Ok(Value::Bool(ok))
}
