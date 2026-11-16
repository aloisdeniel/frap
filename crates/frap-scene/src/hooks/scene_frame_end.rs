// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.frame_end` (`rna_Scene_end_frame_set`): clamped to `[0, 1048574]` and pulls
//! `frame_start` down when it passes it (verified on 5.2.4: `frame_end = 0` with
//! `frame_start = 1` gives `(0, 0)`).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::scene_of;

#[rna_hook(srna = "Scene", prop = "frame_end", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let scene = scene_of(this)?;
    let Value::Int(i) = v else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.frame_end = i.clamp(0, 1_048_574) as i32;
        if s.frame_end < s.frame_start {
            s.frame_start = s.frame_end;
        }
    }
    Ok(())
}
