// SPDX-License-Identifier: GPL-3.0-or-later
//! UI members of `bpy.context` in frap's virtual 3D View (ARCHITECTURE §8; WP 2.3).
//!
//! frap has no screen, area or region: as §8 decides, `area`, `region`, `region_data` and
//! `screen` read as `None` (Blender 5.2.4 returns the Layout screen's 3D View members inside a
//! VIEW_3D override; outside one, `-b` gives `None` for all but `screen`, verified).
//!
//! `space_data` (the `SpaceView3D` stub with `shading` and `region_3d.view_matrix`) and
//! `preferences` (read-only, writable only under `addons['io_scene_gltf2'].preferences`) need
//! struct pointers rooted at `bpy.context`, which the resolver cannot walk yet, and RNA
//! bindings for `Preferences` and a `SpaceView3D` stub, which do not exist yet: they answer
//! `NOT_IMPLEMENTED` (renders read `scene.display.shading`, which is bound).

use frap_rna::{FrapError, PropDef, PyExc};
use frap_scene::HostRead;

use super::super::Target;
use super::Member;
use crate::errors::NOT_IMPLEMENTED;

/// Reads `bpy.context.<prop>` for the UI members.
pub(super) fn member(host: &dyn HostRead, prop: &'static PropDef) -> Result<Member, FrapError> {
    let _ = host;
    match prop.identifier {
        "area" | "region" | "region_data" | "screen" => Ok(Member::Target(Target::None)),
        other => Err(FrapError::new(
            PyExc::NotImplementedError,
            NOT_IMPLEMENTED,
            format!("frap: bpy.context.{other} is not implemented yet"),
        )),
    }
}
