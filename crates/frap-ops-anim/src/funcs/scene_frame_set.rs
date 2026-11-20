// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.frame_set(frame, subframe=0.0)` (`rna_Scene_frame_set`, Blender 5.2.4): sets the
//! frame (`BKE_scene_frame_set`, clamped to ±1048574, no "negative frames" clamp, unlike the
//! `frame_current` setter) and evaluates the scene for it
//! (`BKE_scene_graph_update_for_newframe`): animation at `frame + subframe` is written into the
//! originals even when the frame did not change (§7.3, WP 6.4). Verified on 5.2.4: a
//! Bézier-interpolated `location` reads its animated value right after the call.
//!
//! frap stores no sub-frame (`Scene.frame_subframe` is not bound): the sub-frame only moves
//! the evaluation time. For another scene than the window's, only the frame is set and time
//! is marked dirty (frap evaluates the window's scene).

use frap_rna::{IdRef, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, SceneId, Value, rna_func};
use frap_scene::{FuncArgs, Host};

/// The scene `this` is rooted at.
fn scene_of(this: &RnaPtr) -> Result<IdRef<SceneId>, RnaError> {
    match this.root {
        Root::Id(a) => a.typed(),
        _ => None,
    }
    .ok_or_else(|| {
        RnaError::new(
            RnaErrorKind::Other,
            PyExc::RuntimeError,
            "frap internal: the pointer is not rooted at a Scene",
        )
    })
}

/// `MINAFRAME` / `MAXFRAME`
const MAXFRAME: f64 = 1_048_574.0;

#[rna_func(srna = "Scene", func = "frame_set")]
fn scene_frame_set(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let scene = scene_of(this)?;
    let frame = match args.get("frame") {
        Some(Value::Int(i)) => *i as f64,
        _ => 0.0,
    };
    let subframe = args
        .get("subframe")
        .and_then(Value::as_f32)
        .map_or(0.0, f64::from);
    let cfra = (frame + subframe).clamp(-MAXFRAME, MAXFRAME);
    let whole = cfra.floor();
    {
        let (main, log) = h.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.frame_current = whole as i32;
        }
    }
    if h.win().scene != scene {
        h.mark_time_dirty();
        return Ok(Value::None);
    }
    // This evaluation consumes a pending time change (a no-op when none is pending), then the
    // animation is written at the exact time, sub-frame included.
    h.apply_pending_animation()?;
    frap_eval::animate::apply_animation_at(h, cfra as f32)?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    Ok(Value::None)
}
