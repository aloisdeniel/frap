// SPDX-License-Identifier: GPL-3.0-or-later
//! The film (WP 5.1): `Rgba32Float` accumulation of the TAA samples (an `Rgba16Float`
//! accumulator has an 11-bit mantissa and bands after a few dozen samples, §19.2), the
//! `render.filter_size` reconstruction filter, `film_transparent` alpha, and the readback into
//! a scene-linear, straight-alpha [`frap_render_scene::Film`].
//!
//! As EEVEE's film (`eevee_film.cc`, `Film::update_sample_table`): every film pixel gathers
//! the render texels within `filter_radius = filter_size` of its centre, weighted by the
//! Gaussian fit of Blackman-Harris ([`filter_weight`]), and divides by the summed weights at
//! the end. Rendered samples are premultiplied (the background of a transparent film is
//! `(0, 0, 0, 0)`); the readback un-premultiplies.
//!
//! The accumulation is a full-screen pass that reads the previous accumulator and the sample
//! and writes the other accumulator (ping-pong): `Rgba32Float` is renderable but not blendable
//! without an optional feature.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;

use frap_render_scene::{Film, RenderError};

use crate::gpu::Gpu;

/// Format of the film accumulators.
pub const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// Format of one rendered sample (scene-linear, premultiplied).
pub const SAMPLE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Maximum number of film taps per sample (EEVEE: `FILM_PRECOMP_SAMPLE_MAX`, 16; frap keeps
/// the whole neighbourhood of filters up to 2.2 pixels, which needs up to 49 taps).
pub const MAX_TAPS: usize = 49;
/// The film accumulation shader.
pub const FILM_WGSL: &str = include_str!("../shaders/eevee_film.wgsl");

// Ported from blender/source/blender/draw/engines/eevee/eevee_film_shared.hh
// (`film_filter_weight`, v5.2.4).
/// Film filter weight at squared distance `sample_distance_sqr` (pixels²): the Gaussian
/// (σ = 0.284 of the radius) EEVEE fits to Blackman-Harris.
pub fn filter_weight(filter_radius: f32, sample_distance_sqr: f32) -> f32 {
    let r = sample_distance_sqr / (filter_radius * filter_radius);
    let sigma = 0.284f32;
    let fac = -0.5 / (sigma * sigma);
    (fac * r).exp()
}

/// The texels one film pixel gathers for one sample: `(texel offset, weight)`.
#[derive(Clone, Debug, PartialEq)]
pub struct Taps {
    /// Offsets (render texels, `y` down) and weights.
    pub taps: Vec<([i32; 2], f32)>,
    /// Sum of the weights (`samples_weight_total`).
    pub weight_total: f32,
}

/// The film radius of a render: `clamp(filter_size, 0, 100)`, 0 for a single sample (EEVEE
/// disables filtering then).
pub fn filter_radius(filter_size: f32, samples: u32) -> f32 {
    if samples <= 1 {
        0.0
    } else {
        filter_size.clamp(0.0, 100.0)
    }
}

// Ported from blender/source/blender/draw/engines/eevee/eevee_film.cc
// (`Film::update_sample_table`, v5.2.4; scaling factor 1).
/// The taps of a sample rendered with sub-pixel `offset` (the scene seen by render texel `t` is
/// at `t + 0.5 + offset`; the film pixel `p` takes texel `p + k` at distance `k + offset`).
/// `random` is the sample's `SAMPLING_SSS_U/V` pair (large filters only).
pub fn sample_table(filter_radius: f32, offset: [f32; 2], random: [f32; 2]) -> Taps {
    let offset = Vec2::from(offset);
    if filter_radius < 0.01 {
        return Taps {
            taps: vec![([0, 0], 1.0)],
            weight_total: 1.0,
        };
    }
    let mut taps = vec![];
    let mut weight_total = 0.0;
    if filter_radius < 2.20 {
        let ceil = filter_radius.ceil() as i32;
        let r2 = filter_radius * filter_radius;
        for y in -ceil..=ceil {
            for x in -ceil..=ceil {
                let d2 = (Vec2::new(x as f32, y as f32) + offset).length_squared();
                if d2 < r2 {
                    let w = filter_weight(filter_radius, d2);
                    taps.push(([x, y], w));
                    weight_total += w;
                }
            }
        }
    } else {
        // Large filters: 16 taps on a Fibonacci spiral (EEVEE avoids a TDR with them).
        for i in 0..16 {
            let rx = (random[0] + i as f32) / 16.0;
            let omega = 4.0 * std::f32::consts::PI * (1.0 + 5f32.sqrt()) * rx
                + random[1] * 2.0 * std::f32::consts::PI;
            let k = (rx.sqrt() * Vec2::new(omega.cos(), omega.sin()) * filter_radius).floor();
            let d2 = (k + offset).length_squared();
            let w = filter_weight(filter_radius, d2);
            taps.push(([k.x as i32, k.y as i32], w));
            weight_total += w;
        }
    }
    taps.truncate(MAX_TAPS);
    Taps { taps, weight_total }
}

/// Uniforms of one accumulation pass (`eevee_film.wgsl` `FilmU`).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub(crate) struct FilmU {
    /// `xy` texel offset, `z` weight.
    taps: [[f32; 4]; MAX_TAPS],
    /// `x` tap count, `yz` render extent.
    count: [u32; 4],
}

impl FilmU {
    pub(crate) fn new(taps: &Taps, size: [u32; 2]) -> FilmU {
        let mut u = FilmU::zeroed();
        for (i, (k, w)) in taps.taps.iter().enumerate().take(MAX_TAPS) {
            u.taps[i] = [k[0] as f32, k[1] as f32, *w, 0.0];
        }
        u.count = [taps.taps.len().min(MAX_TAPS) as u32, size[0], size[1], 0];
        u
    }
}

/// The accumulation pipeline (created once per device).
pub(crate) struct FilmPipeline {
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
}

impl FilmPipeline {
    pub(crate) fn new(gpu: &Gpu) -> FilmPipeline {
        let dev = &gpu.device;
        let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("frap eevee film"),
            source: wgpu::ShaderSource::Wgsl(FILM_WGSL.into()),
        });
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bgl =
            dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("frap eevee film"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<FilmU>() as u64
                            ),
                        },
                        count: None,
                    },
                    tex(1),
                    tex(2),
                ],
            });
        let layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frap eevee film"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("frap eevee film accumulate"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_fullscreen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_accumulate"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ACCUM_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        FilmPipeline { pipeline, bgl }
    }
}

/// The two accumulators of one render, the uniform buffer of the accumulation pass and the
/// running weight total.
pub(crate) struct FilmTargets {
    size: [u32; 2],
    acc: [wgpu::Texture; 2],
    acc_view: [wgpu::TextureView; 2],
    uniforms: wgpu::Buffer,
    /// `bind[i]` reads accumulator `i` and the sample, and is drawn into accumulator `1 - i`.
    bind: [wgpu::BindGroup; 2],
    /// Number of accumulated samples (the current accumulator is `acc[samples % 2]`).
    samples: usize,
    weight_total: f64,
}

impl FilmTargets {
    /// Accumulators for a `size` render reading the sample texture `sample`.
    pub(crate) fn new(
        gpu: &Gpu,
        pipe: &FilmPipeline,
        size: [u32; 2],
        sample: &wgpu::TextureView,
    ) -> FilmTargets {
        let dev = &gpu.device;
        let acc = [0, 1].map(|_| {
            dev.create_texture(&wgpu::TextureDescriptor {
                label: Some("frap eevee film accumulator"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: ACCUM_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let acc_view = [0, 1].map(|i| acc[i].create_view(&Default::default()));
        let uniforms = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frap eevee film taps"),
            size: std::mem::size_of::<FilmU>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind = [0, 1].map(|i| {
            dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frap eevee film"),
                layout: &pipe.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&acc_view[i]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(sample),
                    },
                ],
            })
        });
        FilmTargets {
            size,
            acc,
            acc_view,
            uniforms,
            bind,
            samples: 0,
            weight_total: 0.0,
        }
    }

    /// Records the accumulation of the sample rendered with `taps` (the uniforms are written
    /// through the queue: submit `enc` before the next call).
    pub(crate) fn accumulate(
        &mut self,
        gpu: &Gpu,
        pipe: &FilmPipeline,
        enc: &mut wgpu::CommandEncoder,
        taps: &Taps,
    ) {
        gpu.queue.write_buffer(
            &self.uniforms,
            0,
            bytemuck::bytes_of(&FilmU::new(taps, self.size)),
        );
        let src = self.samples % 2;
        let dst = 1 - src;
        // The first sample reads an accumulator the pass below has not written yet: clear it.
        if self.samples == 0 {
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frap eevee film clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.acc_view[src],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frap eevee film accumulate"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.acc_view[dst],
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipe.pipeline);
        pass.set_bind_group(0, &self.bind[src], &[]);
        pass.draw(0..3, 0..1);
        drop(pass);
        self.samples += 1;
        self.weight_total += f64::from(taps.weight_total);
    }

    /// Reads the film back: the accumulated premultiplied colour divided by the summed
    /// weights, then un-premultiplied (straight alpha, as `Film` stores it).
    pub(crate) fn resolve(&self, gpu: &Gpu) -> Result<Film, RenderError> {
        let [width, height] = self.size;
        let tex = &self.acc[self.samples % 2];
        let mut rgba = gpu.read_texture(tex, width, height, ACCUM_FORMAT)?;
        let inv = if self.weight_total > 0.0 {
            (1.0 / self.weight_total) as f32
        } else {
            0.0
        };
        for chunk in rgba.chunks_mut(1 << 16) {
            frap_math::budget::check_now();
            for p in chunk {
                *p = unpremultiply(p.map(|c| c * inv));
            }
        }
        Ok(Film {
            width,
            height,
            rgba,
        })
    }
}

/// Premultiplied → straight alpha (alpha clamped to `[0, 1]`; colour kept where alpha is 0,
/// which a transparent film's background never has).
pub fn unpremultiply(p: [f32; 4]) -> [f32; 4] {
    let a = p[3].clamp(0.0, 1.0);
    if a <= 0.0 || a >= 1.0 {
        return [p[0], p[1], p[2], a];
    }
    [p[0] / a, p[1] / a, p[2] / a, a]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_weight_is_eevee_gaussian() {
        assert_eq!(filter_weight(1.5, 0.0), 1.0);
        // At the radius: exp(-0.5 / 0.284²).
        let at_radius = filter_weight(1.5, 2.25);
        assert!((at_radius - (-0.5f32 / (0.284 * 0.284)).exp()).abs() < 1e-7);
        assert!(at_radius < 0.003);
    }

    #[test]
    fn default_filter_taps() {
        // filter_size 1.5, centred sample: the 3 x 3 neighbourhood plus the 4 axis texels at
        // distance 1... all texels with |k| < 1.5 (9 of them: corners at √2 ≈ 1.41).
        let t = sample_table(1.5, [0.0, 0.0], [0.0, 0.0]);
        assert_eq!(t.taps.len(), 9);
        let total: f32 = t.taps.iter().map(|x| x.1).sum();
        assert!((total - t.weight_total).abs() < 1e-6);
        // Shifted half a pixel: the texel at (-1, -1) comes closer, (1, 1) leaves the radius.
        let t = sample_table(1.5, [0.5, 0.5], [0.0, 0.0]);
        assert!(t.taps.iter().any(|(k, _)| *k == [-1, -1]));
        assert!(!t.taps.iter().any(|(k, _)| *k == [1, 1]));
        assert!(t.taps.len() <= MAX_TAPS);
    }

    #[test]
    fn single_sample_and_box() {
        assert_eq!(filter_radius(1.5, 1), 0.0);
        assert_eq!(filter_radius(1.5, 64), 1.5);
        assert_eq!(filter_radius(250.0, 64), 100.0);
        let t = sample_table(0.0, [0.3, 0.1], [0.0, 0.0]);
        assert_eq!(t.taps, vec![([0, 0], 1.0)]);
        let big = sample_table(4.0, [0.0, 0.0], [0.3, 0.7]);
        assert_eq!(big.taps.len(), 16);
        assert!(
            big.taps
                .iter()
                .all(|(k, _)| k[0].abs() <= 4 && k[1].abs() <= 4)
        );
    }

    #[test]
    fn unpremultiply_straight_alpha() {
        assert_eq!(unpremultiply([0.25, 0.5, 0.0, 0.5]), [0.5, 1.0, 0.0, 0.5]);
        assert_eq!(unpremultiply([0.2, 0.3, 0.4, 1.0]), [0.2, 0.3, 0.4, 1.0]);
        assert_eq!(unpremultiply([0.0, 0.0, 0.0, 0.0]), [0.0; 4]);
        assert_eq!(unpremultiply([0.5, 0.5, 0.5, 1.2]), [0.5, 0.5, 0.5, 1.0]);
    }
}
