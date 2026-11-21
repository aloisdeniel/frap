// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.frame_set(frame, subframe=0.0)` (`rna_Scene_frame_set`, Blender 5.2.4): sets the
//! frame (`BKE_scene_frame_set`, clamped to ±1048574, no "negative frames" clamp, unlike the
//! `frame_current` setter) and evaluates the scene for it
//! (`BKE_scene_graph_update_for_newframe`): animation at `frame + subframe` is written into the
//! originals even when the frame did not change (§7.3, WP 6.4). Verified on 5.2.4: a
//! Bézier-interpolated `location` reads its animated value right after the call.
//!
//! Only the datablocks of the window's depsgraph are animated
//! (`frap_eval::animate::depsgraph_roots`): an object unlinked from the scene, or linked only
//! into another scene, keeps its values (verified on 5.2.4).
//!
//! frap stores no sub-frame (`Scene.frame_subframe` is not bound): the sub-frame only moves
//! the evaluation time. For another scene than the window's, the animation is written only
//! for its view layers the window showed before (Blender's depsgraph stays active once shown;
//! verified on 5.2.4: `bpy.data.scenes['Other'].frame_set(5)` of a scene never shown animates
//! nothing, of a scene shown before animates its objects; switching the window to a scene
//! applies its frame, see `Window.scene`).

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
fn scene_frame_set(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
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
    // `BKE_scene_frame_set`: `modf` (the whole part truncates toward zero).
    let whole = cfra.trunc();
    {
        let (main, log) = h.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.frame_current = whole as i32;
        }
    }
    if h.win().scene == scene {
        // This evaluation consumes a pending time change (a no-op when none is pending), then
        // the animation is written at the exact time, sub-frame included.
        h.apply_pending_animation()?;
    }
    frap_eval::animate::apply_scene_animation_at(h, scene, cfra as f32)?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    Ok(Value::None)
}
