// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.frame_current` (`rna_Scene_frame_current_set`): stores the frame and marks time
//! dirty only (§7.3: animation is applied at the next evaluation boundary). Blender's setter
//! does not apply the RNA range: with the factory preference "Allow Negative Frames" off,
//! negative frames become 0 (`FRAMENUMBER_MIN_CLAMP`) and large frames are kept (verified on
//! 5.2.4: `= 1000000000` reads back 1000000000, `= -1000000000` reads back 0).
//!
//! The setter receives the value after `coerce`, which clamps to the property's hard range
//! (±1048574) unless the caller skips that clamp for this property
//! ([`crate::id::Scene::UNCLAMPED`], WP 1.14's resolver); the hook applies
//! [`crate::id::Scene::frame_from_input`].

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::scene_of;

#[rna_hook(srna = "Scene", prop = "frame_current", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let scene = scene_of(this)?;
    let Value::Int(i) = v else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.frame_current = crate::id::Scene::frame_from_input(i);
    }
    h.mark_time_dirty();
    Ok(())
}
