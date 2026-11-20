// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF animation import (WP 6.7, `crates/frap-io-gltf/src/import/animation/**`): one action
//! per glTF animation and object (slots, F-curves from LINEAR/STEP/CUBICSPLINE samplers, the
//! add-on's frame conversion), the active one assigned and the others stashed on NLA tracks
//! (`import_select_created_objects`, `BlenderAnimation.anim`/`restore_animation`), ported from
//! io_scene_gltf2 5.2.41 `blender/imp/animation*.py`. Bone channels target pose bones through
//! `VNode::blender_bone_name` (WP 6.8).
//!
//! M5+M6 W0: [`IMPLEMENTED`] is `false`, so the importer keeps its WARNING ("animations are
//! imported from M6") and never calls [`create`].

use frap_scene::{ChangeLog, Main};

use crate::import::{Gltf, ImportError};

/// Whether animations are imported (WP 6.7 sets it).
pub(crate) const IMPLEMENTED: bool = false;

/// Creates the actions of every glTF animation (after the scenes and their objects).
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<(), ImportError> {
    let _ = (cx, main, log);
    Ok(())
}
