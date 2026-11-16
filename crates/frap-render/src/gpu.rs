// SPDX-License-Identifier: GPL-3.0-or-later
//! wgpu device creation with the §19.2 backend policy. Moved from the WP 0.2 contract probe
//! (`crates/frap-probe/render`, verified on Metal, M1 Max): macOS Metal; Linux Vulkan
//! (lavapipe in CI); Windows Vulkan first, then DX12 with `Dx12Compiler::Fxc` selected
//! explicitly (WARP in CI; no `static-dxc`, rule 2.2.9). `FRAP_WGPU_BACKEND` overrides the
//! order; `FRAP_FORCE_FALLBACK=1` asks for a software adapter. wgpu 30 API notes: RFC 0001
//! F-REN-3.

use frap_render_scene::{AdapterSummary, GpuOptions, RenderError};

/// A wgpu device and queue.
pub struct Gpu {
    /// Device.
    pub device: wgpu::Device,
    /// Queue.
    pub queue: wgpu::Queue,
    /// Adapter info.
    pub info: wgpu::AdapterInfo,
}

fn backend_order(opts: &GpuOptions) -> Result<Vec<wgpu::Backends>, RenderError> {
    if let Some(b) = &opts.backend {
        return Ok(vec![match b.to_ascii_lowercase().as_str() {
            "metal" => wgpu::Backends::METAL,
            "vulkan" => wgpu::Backends::VULKAN,
            "dx12" => wgpu::Backends::DX12,
            "gl" => wgpu::Backends::GL,
            other => {
                return Err(RenderError::NoAdapter(format!(
                    "FRAP_WGPU_BACKEND={other}: expected metal|vulkan|dx12|gl"
                )));
            }
        }]);
    }
    Ok(if cfg!(target_os = "macos") {
        vec![wgpu::Backends::METAL]
    } else if cfg!(windows) {
        vec![wgpu::Backends::VULKAN, wgpu::Backends::DX12]
    } else {
        vec![wgpu::Backends::VULKAN]
    })
}

/// Device limits: wgpu's downlevel defaults raised to the adapter's texture dimensions
/// (renders up to `max_texture_dimension_2d` per side, as Blender does) and buffer size (large
/// meshes; readback itself is band-split, see [`Gpu::read_texture`]).
fn required_limits(adapter: &wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits {
        max_buffer_size: adapter.max_buffer_size,
        ..wgpu::Limits::downlevel_defaults().using_resolution(adapter.clone())
    }
}

impl Gpu {
    /// Creates the device (first render only; the service keeps it).
    pub fn new(opts: &GpuOptions) -> Result<Gpu, RenderError> {
        let mut tried = vec![];
        for backends in backend_order(opts)? {
            let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
            desc.backends = backends;
            desc.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
            let instance = wgpu::Instance::new(desc);
            let req = wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: opts.force_fallback,
                compatible_surface: None,
                ..Default::default()
            };
            let adapter = match pollster::block_on(instance.request_adapter(&req)) {
                Ok(a) => a,
                Err(e) => {
                    tried.push(format!("{backends:?}: {e}"));
                    continue;
                }
            };
            let info = adapter.get_info();
            if let Some(want) = &opts.adapter
                && !info.name.contains(want.as_str())
            {
                tried.push(format!("{backends:?}: adapter {:?} != {want:?}", info.name));
                continue;
            }
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                    label: Some("frap"),
                    required_features: wgpu::Features::empty(),
                    required_limits: required_limits(&adapter.limits()),
                    ..Default::default()
                }))
                .map_err(|e| RenderError::Device(e.to_string()))?;
            return Ok(Gpu {
                device,
                queue,
                info,
            });
        }
        Err(RenderError::NoAdapter(tried.join("; ")))
    }

    /// Adapter summary.
    pub fn summary(&self) -> AdapterSummary {
        AdapterSummary {
            name: self.info.name.clone(),
            backend: format!("{:?}", self.info.backend),
            device_type: format!("{:?}", self.info.device_type),
            driver: format!("{} {}", self.info.driver, self.info.driver_info),
        }
    }
}
