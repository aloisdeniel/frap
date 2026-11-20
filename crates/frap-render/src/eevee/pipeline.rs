// SPDX-License-Identifier: GPL-3.0-or-later
//! EEVEE passes and pipelines (WP 5.1): depth pre-pass, shadow maps, the forward uber-shader
//! (opaque, then BLENDED sorted per object, DITHERED hashed), screen-space refraction for
//! transmission, the world background, TAA accumulation into the film.
//!
//! Stub (M5+M6 W0): [`IMPLEMENTED`] is `false`, so the render service keeps rendering
//! `BLENDER_EEVEE` with Workbench (and its WARNING) until WP 5.1 lands and sets it.

use frap_render_scene::{Film, RenderError, RenderScene};

use crate::engine::GpuCache;
use crate::gpu::Gpu;

/// Whether the EEVEE-like engine renders (`RenderService::supports_engine("BLENDER_EEVEE")`).
/// WP 5.1 sets it to `true` when its acceptance passes.
pub const IMPLEMENTED: bool = false;

/// Per-material parameters of the forward uber-shader (a uniform/storage record): the core
/// PBR inputs WP 5.1 packs, the feature flag bits and the values and texture slots the shading
/// features (`features/*.rs`, WP 5.5) pack. Layout owned by WP 5.1.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct MaterialParams {}

/// Engine state kept between renders (pipelines per target format, shadow atlases, samplers).
#[derive(Default)]
#[non_exhaustive]
pub struct State {}

/// Renders `scene` (WP 5.1).
pub fn render(
    state: &mut State,
    gpu: &Gpu,
    cache: &mut GpuCache,
    scene: &RenderScene,
) -> Result<Film, RenderError> {
    let _ = (state, gpu, cache, scene);
    Err(RenderError::Unsupported(
        "the EEVEE-like engine is not implemented yet (M5, WP 5.1)".into(),
    ))
}
