// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.frame_start` (`rna_Scene_start_frame_set`): clamped to `[0, 1048574]` (by the RNA
//! range) and pushes `frame_end` up when it passes it (verified on 5.2.4: `frame_end = 10;
//! frame_start = 20` gives `(20, 20)`).

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::scene_of;

#[rna_hook(srna = "Scene", prop = "frame_start", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let scene = scene_of(this)?;
    let Value::Int(i) = v else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.frame_start = i.clamp(0, 1_048_574) as i32;
        if s.frame_start > s.frame_end {
            s.frame_end = s.frame_start;
        }
    }
    Ok(())
}
