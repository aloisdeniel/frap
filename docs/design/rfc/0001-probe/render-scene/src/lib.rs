// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-render-scene` (§19.2): plain-data `RenderScene`, `Film` and the only
//! GPU-facing trait, `RenderService`. `build` turns an evaluated host into a `RenderScene`.
#![forbid(unsafe_code)]

use frap_probe_scene::{EvalMode, Evaluator, HostRead};

pub mod types {
    /// Subset: resolution and a constant world colour (enough for clear-and-readback).
    #[derive(Clone, Debug, PartialEq)]
    pub struct RenderScene {
        pub resolution: [u32; 2],
        pub world_rgb: [f32; 3],
        pub objects: Vec<RObject>,
    }
    #[derive(Clone, Debug, PartialEq)]
    pub struct RObject {
        pub name: String,
        pub matrix_world: [[f32; 4]; 4],
    }

    /// Scene-linear, straight alpha.
    #[derive(Clone, Debug, PartialEq)]
    pub struct Film {
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<[f32; 4]>,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub enum RenderError {
        NoAdapter(String),
        Device(String),
        Readback(String),
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct AdapterSummary {
        pub name: String,
        pub backend: String,
        pub device_type: String,
        pub driver: String,
    }

    /// The only GPU-facing trait the rest of frap sees (CONTRACT).
    pub trait RenderService: Send {
        fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError>;
        fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError>;
        fn adapter_info(&self) -> AdapterSummary;
    }
}

pub use types::*;

/// Backend selection knobs (§19.2). Defined here (L8) so that a build without the `gpu`
/// feature can still name the factory type (RFC 0001 F-REN-2).
#[derive(Clone, Debug, Default)]
pub struct GpuOptions {
    pub backend: Option<String>,
    pub force_fallback: bool,
}

impl GpuOptions {
    /// `FRAP_WGPU_BACKEND` (`metal|vulkan|dx12|gl`), `FRAP_FORCE_FALLBACK=1`.
    pub fn from_env() -> Self {
        GpuOptions {
            backend: std::env::var("FRAP_WGPU_BACKEND").ok().filter(|s| !s.is_empty()),
            force_fallback: std::env::var("FRAP_FORCE_FALLBACK").is_ok_and(|v| v == "1"),
        }
    }
}

/// Request (§19.1 `build(&dyn HostRead, &mut dyn Evaluator, &RenderRequest)`; undefined in the
/// contract, RFC 0001 F-REN-1).
#[derive(Clone, Debug, Default)]
pub struct RenderRequest {
    pub resolution: Option<[u32; 2]>,
}

/// §19.1 builder (probe subset).
pub fn build(host: &dyn HostRead, eval: &mut dyn Evaluator, req: &RenderRequest) -> RenderScene {
    let ctx = host.ctx();
    let scene = ctx.scene();
    let mut objects = vec![];
    for ob in ctx.visible_objects() {
        if let Ok(e) = eval.object(host, ob, EvalMode::Render) {
            let name = host.main().objects.get(ob).map(|o| o.header.name.to_string()).unwrap_or_default();
            objects.push(RObject {
                name,
                matrix_world: e.matrix_world,
            });
        }
    }
    objects.sort_by(|a, b| a.name.cmp(&b.name));
    RenderScene {
        resolution: req.resolution.unwrap_or([64, 32]),
        world_rgb: scene.world_color,
        objects,
    }
}
