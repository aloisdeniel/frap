// SPDX-License-Identifier: GPL-3.0-or-later
//! Offscreen clear / WGSL draw and padded readback (§19.2: `RENDER_ATTACHMENT | COPY_SRC`
//! targets, rows padded to 256 bytes). Moved from the WP 0.2 contract probe
//! (`crates/frap-probe/render`), where it passed on Metal; WP 1.7 builds the engines on it.

use frap_render_scene::RenderError;

use crate::gpu::Gpu;

/// Bytes per texel for the formats the probe reads back.
pub fn texel_size(f: wgpu::TextureFormat) -> u32 {
    match f {
        wgpu::TextureFormat::Rgba32Float => 16,
        wgpu::TextureFormat::Rgba8Unorm => 4,
        other => panic!("readback: unsupported format {other:?}"),
    }
}

/// `bytes_per_row` must be a multiple of 256 (`COPY_BYTES_PER_ROW_ALIGNMENT`).
pub fn padded_bytes_per_row(width: u32, format: wgpu::TextureFormat) -> u32 {
    let unpadded = width * texel_size(format);
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(align) * align
}

const TRIANGLE_WGSL: &str = r#"
@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0));
    return vec4<f32>(p[i], 0.0, 1.0);
}
@fragment
fn fs() -> @location(0) vec4<f32> { return vec4<f32>(1.0, 0.5, 0.25, 1.0); }
"#;

/// The triangle colour drawn by [`Gpu::clear_and_readback`] with `draw_triangle = true`.
pub const TRIANGLE_RGBA: [f32; 4] = [1.0, 0.5, 0.25, 1.0];

impl Gpu {
    /// Clear an offscreen target (optionally draw a lower-left half triangle through a WGSL
    /// pipeline, which exercises shader translation: MSL / SPIR-V / HLSL+FXC), copy it to a
    /// buffer with padded rows and return tightly packed RGBA rows as f32.
    #[allow(clippy::too_many_arguments)]
    pub fn clear_and_readback(
        &self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        clear: [f64; 4],
        draw_triangle: bool,
    ) -> Result<Vec<[f32; 4]>, RenderError> {
        let dev = &self.device;
        let tex = dev.create_texture(&wgpu::TextureDescriptor {
            label: Some("frap target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bpr = padded_bytes_per_row(width, format);
        let buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frap readback"),
            size: u64::from(bpr) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let pipeline = draw_triangle.then(|| {
            let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("frap triangle"),
                source: wgpu::ShaderSource::Wgsl(TRIANGLE_WGSL.into()),
            });
            dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("frap triangle"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frap"),
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frap clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear[0],
                            g: clear[1],
                            b: clear[2],
                            a: clear[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(p) = &pipeline {
                pass.set_pipeline(p);
                pass.draw(0..3, 0..1);
            }
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        dev.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|e| RenderError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| RenderError::Readback(e.to_string()))?
            .map_err(|e| RenderError::Readback(e.to_string()))?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| RenderError::Readback(format!("{e:?}")))?;
        let ts = texel_size(format) as usize;
        let mut out = Vec::with_capacity((width * height) as usize);
        for row in 0..height as usize {
            let start = row * bpr as usize;
            for x in 0..width as usize {
                let px = &data[start + x * ts..start + (x + 1) * ts];
                out.push(match format {
                    wgpu::TextureFormat::Rgba32Float => {
                        let f =
                            |i: usize| f32::from_le_bytes([px[i], px[i + 1], px[i + 2], px[i + 3]]);
                        [f(0), f(4), f(8), f(12)]
                    }
                    _ => [
                        px[0] as f32 / 255.0,
                        px[1] as f32 / 255.0,
                        px[2] as f32 / 255.0,
                        px[3] as f32 / 255.0,
                    ],
                });
            }
        }
        drop(data);
        buf.unmap();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn padding() {
        assert_eq!(
            super::padded_bytes_per_row(67, wgpu::TextureFormat::Rgba32Float),
            1280
        );
        assert_eq!(
            super::padded_bytes_per_row(13, wgpu::TextureFormat::Rgba8Unorm),
            256
        );
        assert_eq!(
            super::padded_bytes_per_row(64, wgpu::TextureFormat::Rgba8Unorm),
            256
        );
    }

    #[test]
    fn gpu_and_service_are_send() {
        fn send<T: Send>() {}
        send::<crate::Gpu>();
        send::<crate::Renderer>();
    }
}
