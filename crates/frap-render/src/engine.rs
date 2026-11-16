// SPDX-License-Identifier: GPL-3.0-or-later
//! `trait Engine` and the GPU cache (§19.2). One uber-shader per engine (WP 1.7).

use frap_render_scene::{Film, RenderError, RenderScene};

use crate::gpu::Gpu;

/// GPU resources kept between renders: meshes by `MeshKey`, textures by `ImageKey`.
#[derive(Default)]
pub struct GpuCache {
    _private: (),
}

/// A render engine (Workbench, EEVEE-like).
pub trait Engine {
    /// Renders one scene to scene-linear pixels.
    fn render(
        &mut self,
        gpu: &Gpu,
        cache: &mut GpuCache,
        scene: &RenderScene,
    ) -> Result<Film, RenderError>;
}
