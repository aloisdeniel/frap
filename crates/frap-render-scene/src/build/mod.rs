// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluated scene → `RenderScene` (§19.1): corner-split meshes, `LoweredMaterial`s, images,
//! lights, camera (`BKE_camera_params` port), world, settings. WP 1.15.

use frap_scene::{Evaluator, HostRead};

use crate::types::{RenderRequest, RenderScene};

/// Builds the render scene of a (cloned, animation-applied) state.
pub fn build(
    host: &dyn HostRead,
    eval: &mut dyn Evaluator,
    req: &RenderRequest,
) -> Result<RenderScene, crate::types::RenderError> {
    let _ = (host, eval, req);
    todo!("WP 1.15")
}
