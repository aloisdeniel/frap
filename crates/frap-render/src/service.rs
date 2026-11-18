// SPDX-License-Identifier: GPL-3.0-or-later
//! `RenderService` implementation (§19.1): the device is created lazily on the first render.

use frap_render_scene::{
    AdapterSummary, Film, GpuOptions, RenderError, RenderScene, RenderService,
};

use crate::engine::{Engine, GpuCache};
use crate::gpu::Gpu;
use crate::workbench::Workbench;

/// The renderer: lazy GPU, engines, cache.
pub struct Renderer {
    opts: GpuOptions,
    gpu: Option<Gpu>,
    cache: GpuCache,
    workbench: Workbench,
}

/// Creates the render service used by `frap-session` (feature `gpu`).
pub fn new_service(opts: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError> {
    Ok(Box::new(Renderer::new(opts)))
}

impl Renderer {
    /// A renderer; no device is created until the first render.
    pub fn new(opts: &GpuOptions) -> Renderer {
        Renderer {
            opts: opts.clone(),
            gpu: None,
            cache: GpuCache::default(),
            workbench: Workbench::new(),
        }
    }

    /// The device, created on first use.
    pub fn gpu(&mut self) -> Result<&Gpu, RenderError> {
        if self.gpu.is_none() {
            self.gpu = Some(Gpu::new(&self.opts)?);
        }
        Ok(self.gpu.as_ref().expect("invariant: just created"))
    }

    /// Whether the device exists yet (it is created by the first render).
    pub fn has_device(&self) -> bool {
        self.gpu.is_some()
    }

    /// The Workbench engine.
    pub fn workbench(&self) -> &Workbench {
        &self.workbench
    }

    /// The GPU cache (meshes kept between renders).
    pub fn cache(&self) -> &GpuCache {
        &self.cache
    }
}

impl RenderService for Renderer {
    fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError> {
        // `--stmt-timeout` cancellation point (§12). Never inside the error scopes below: an
        // unwind there would leave them pushed on the device.
        frap_math::budget::check_now();
        self.gpu()?;
        let gpu = self.gpu.as_ref().expect("invariant: created above");
        // M1: every engine renders with Workbench. `render.render` reports the EEVEE/CYCLES
        // fallback WARNING (WP 1.24); the EEVEE-like engine arrives in M5.
        let engine: &mut dyn Engine = &mut self.workbench;
        // wgpu reports validation and allocation failures to an error scope instead of the
        // default panicking handler.
        let oom = gpu.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let film = engine.render(gpu, &mut self.cache, scene);
        let errors = [validation.pop(), oom.pop()].map(pollster::block_on);
        if let Some(e) = errors.into_iter().flatten().next() {
            return Err(RenderError::Device(e.to_string()));
        }
        let film = film?;
        frap_math::budget::check_now();
        Ok(film)
    }
    fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError> {
        scenes.iter().map(|s| self.render(s)).collect()
    }
    fn adapter_info(&self) -> AdapterSummary {
        self.gpu.as_ref().map(Gpu::summary).unwrap_or_default()
    }
}
