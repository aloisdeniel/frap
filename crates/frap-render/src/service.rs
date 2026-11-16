// SPDX-License-Identifier: GPL-3.0-or-later
//! `RenderService` implementation (§19.1): the device is created lazily on the first render.

use frap_render_scene::{
    AdapterSummary, Film, GpuOptions, RenderError, RenderScene, RenderService,
};

use crate::engine::GpuCache;
use crate::gpu::Gpu;

/// The renderer: lazy GPU, engines, cache.
pub struct Renderer {
    opts: GpuOptions,
    gpu: Option<Gpu>,
    cache: GpuCache,
}

/// Creates the render service used by `frap-session` (feature `gpu`).
pub fn new_service(opts: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError> {
    Ok(Box::new(Renderer {
        opts: opts.clone(),
        gpu: None,
        cache: GpuCache::default(),
    }))
}

impl Renderer {
    /// The device, created on first use.
    pub fn gpu(&mut self) -> Result<&Gpu, RenderError> {
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(&self.opts)?);
        }
        Ok(self.gpu.as_ref().expect("invariant: just created"))
    }
}

impl RenderService for Renderer {
    fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError> {
        let _ = (scene, &mut self.cache);
        todo!("WP 1.7: Workbench engine")
    }
    fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError> {
        scenes.iter().map(|s| self.render(s)).collect()
    }
    fn adapter_info(&self) -> AdapterSummary {
        self.gpu.as_ref().map(Gpu::summary).unwrap_or_default()
    }
}
