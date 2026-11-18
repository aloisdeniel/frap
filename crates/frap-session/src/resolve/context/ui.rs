// SPDX-License-Identifier: GPL-3.0-or-later
//! UI members of `bpy.context` in frap's virtual 3D View (ARCHITECTURE §8): `space_data` (a
//! `SpaceView3D` stub exposing `shading` and `region_3d.view_matrix`), read-only `preferences`
//! (writable only under `addons['io_scene_gltf2'].preferences`), and `area`, `region`,
//! `region_data`, `screen`.
//!
//! **Stub** created by M2 W0 (WP 2.0): every member answers `NOT_IMPLEMENTED` as in M1.
//! Implemented by WP 2.3 (OPERATORS.md "Context (preferences, space_data stub)", assigned by
//! the M2 W0 integrator).

use frap_rna::{FrapError, PropDef, PyExc};
use frap_scene::HostRead;

use super::Member;
use crate::errors::NOT_IMPLEMENTED;

/// Reads `bpy.context.<prop>` for the UI members.
pub(super) fn member(host: &dyn HostRead, prop: &'static PropDef) -> Result<Member, FrapError> {
    let _ = host;
    Err(FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!(
            "frap: bpy.context.{} is not implemented yet",
            prop.identifier
        ),
    ))
}
