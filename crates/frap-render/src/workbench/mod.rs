// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench engine (`BLENDER_WORKBENCH`, §19.3): one uber-shader, SOLID/STUDIO/FLAT,
//! MATERIAL/OBJECT/SINGLE/RANDOM colours, `Rgba32Float` accumulation. WP 1.7.
//!
//! Pipeline (per render): for each anti-aliasing sample, the opaque surfaces are drawn into an
//! `Rgba32Float` sample target (cleared to the background) with a sub-pixel jittered
//! projection, then a full-screen pass adds the premultiplied sample into a ping-ponged
//! `Rgba32Float` accumulator (an `Rgba16Float` accumulator bands after a few dozen samples,
//! §19.2). The accumulator is read back through a buffer with 256-byte padded rows and
//! un-premultiplied on the CPU into a straight-alpha scene-linear [`Film`].
//!
//! What M1 covers and how it maps to Blender 5.2.4:
//! - `shading.type`: SOLID only (wireframe and x-ray are WP 2.3).
//! - `shading.light`: STUDIO (the "Default" rig, view-space lights, specular highlights on, as
//!   in factory settings) and FLAT. MATCAP renders as STUDIO until WP 2.3 adds matcaps.
//! - `shading.color_type`: MATERIAL (factors of the lowered material: base colour, metallic,
//!   roughness), OBJECT (`Object.color`), SINGLE (`single_color`), RANDOM
//!   ([`color::random_color`] of `RObject::random_hash`). VERTEX / TEXTURE / ATTRIBUTE render
//!   as MATERIAL until M2/M3.
//! - Background: `render.film_transparent` gives alpha 0; otherwise the world colour
//!   (`RWorld::Color::rgb`; Workbench ignores the strength; an HDRI world uses the factory
//!   world colour).
//! - At one sample per pixel the result matches Blender 5.2.4's Workbench (`render_aa = 'OFF'`)
//!   per pixel within 1e-3 for these modes (`tests/workbench_ref.rs`).
//! - Anti-aliasing: [`DEFAULT_SAMPLES`] = 8 (factory `scene.display.render_aa = '8'`), box
//!   filtered over a fixed Halton (2, 3) jitter sequence. Blender's Blackman-Harris weighted TAA
//!   and SMAA are WP 2.3's (the M2 SSIM-vs-Blender gate).

pub mod color;
pub mod studio_light;

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use frap_render_scene::{Film, MeshKey, RObject, RSettings, RWorld, RenderError, RenderScene};
use glam::{Mat3, Mat4, Vec3};

use crate::engine::{Engine, GpuCache, VERTEX_STRIDE};
use crate::gpu::Gpu;
use studio_light::StudioLight;

/// The uber-shader (all entry points, lighting modes selected by uniform flags).
pub const SHADER_WGSL: &str = include_str!("workbench.wgsl");

/// Samples per render (`scene.display.render_aa` factory value `'8'`).
pub const DEFAULT_SAMPLES: u32 = 8;

/// Format of the sample target and of the accumulator.
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// Factory `world.color` (verified), used when the world is not a plain colour.
const FACTORY_WORLD_COLOR: [f32; 3] = [0.050876, 0.050876, 0.050876];

/// `shading.light`, as the shader sees it (values of Blender's `WORKBENCH_LIGHTING_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lighting {
    /// STUDIO (`WORKBENCH_LIGHTING_STUDIO`)
    Studio,
    /// FLAT (`WORKBENCH_LIGHTING_FLAT`)
    Flat,
}

impl Lighting {
    fn shader_value(self) -> u32 {
        match self {
            Lighting::Studio => 0,
            Lighting::Flat => 2,
        }
    }
}

/// `shading.color_type` sources implemented in M1.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorType {
    /// MATERIAL
    Material,
    /// OBJECT
    Object,
    /// SINGLE
    Single,
    /// RANDOM
    Random,
}

/// The resolved `scene.display.shading` of one render.
#[derive(Clone, Debug, PartialEq)]
pub struct Shading {
    /// Lighting mode.
    pub lighting: Lighting,
    /// Colour source.
    pub color_type: ColorType,
    /// `single_color`
    pub single_color: [f32; 3],
    /// Studio rig (`studio_light`, factory "Default").
    pub studio: StudioLight,
    /// `show_specular_highlight` (factory True) and the rig's specular pass.
    pub use_specular: bool,
}

impl Shading {
    /// Reads the Workbench part of the render settings.
    pub fn from_settings(s: &RSettings) -> Shading {
        let lighting = match s.shading_light.as_str() {
            "FLAT" => Lighting::Flat,
            // "STUDIO", and "MATCAP" until WP 2.3.
            _ => Lighting::Studio,
        };
        let color_type = match s.shading_color_type.as_str() {
            "OBJECT" => ColorType::Object,
            "SINGLE" => ColorType::Single,
            "RANDOM" => ColorType::Random,
            // "MATERIAL", and VERTEX / TEXTURE / ATTRIBUTE until M2/M3.
            _ => ColorType::Material,
        };
        let studio = studio_light::default_rig();
        Shading {
            lighting,
            color_type,
            single_color: s.shading_single_color,
            use_specular: studio.specular_highlight_pass,
            studio,
        }
    }
}

/// What the surface shader receives for one object / material slot.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Surface {
    /// Base colour (linear) and alpha.
    pub base_color: [f32; 4],
    /// Metallic as the lighting sees it (quantised like Blender's G-buffer).
    pub metallic: f32,
    /// Roughness as the lighting sees it (quantised like Blender's G-buffer).
    pub roughness: f32,
}

/// Blender's surface values for `obj`'s material `slot` under `shading`
/// (`workbench_materials.cc`).
pub fn surface(scene: &RenderScene, obj: &RObject, slot: u32, shading: &Shading) -> Surface {
    let (rgb, alpha, metallic, roughness) = match shading.color_type {
        ColorType::Material => {
            let mat = obj
                .materials
                .get(slot as usize)
                .copied()
                .flatten()
                .and_then(|k| scene.materials.get(&k));
            // KNOWN DEVIATION (contract gap, integrator + WP 1.15): Blender's Workbench reads
            // `Material.diffuse_color` / `.metallic` / `.roughness` (viewport display values,
            // fresh-material roughness 0.4), not the Principled node. The frozen render-scene
            // contract carries only the lowered Principled factors, so those are used until an
            // additive field carries the viewport values.
            match mat {
                Some(m) => {
                    let c = m.base_color.factor;
                    (
                        [c[0], c[1], c[2]],
                        c[3],
                        m.metallic.factor,
                        m.roughness.factor,
                    )
                }
                // `BKE_material_default_empty()`: 0.8 grey, metallic 0, roughness 0.4.
                None => ([0.8; 3], 1.0, 0.0, 0.4),
            }
        }
        ColorType::Object => (
            [obj.color[0], obj.color[1], obj.color[2]],
            obj.color[3],
            0.0,
            0.4,
        ),
        ColorType::Single => (shading.single_color, 1.0, 0.0, 0.4),
        ColorType::Random => (color::random_color(obj.random_hash), obj.color[3], 0.0, 0.4),
    };
    let (roughness, metallic) = color::gbuffer_roughness_metallic(roughness, metallic);
    Surface {
        base_color: [rgb[0], rgb[1], rgb[2], alpha],
        metallic,
        roughness,
    }
}

/// The colour the sample target is cleared to (straight alpha).
pub fn background(scene: &RenderScene) -> [f32; 4] {
    if scene.settings.film_transparent {
        return [0.0; 4];
    }
    // KNOWN DEVIATION (contract gap, as for `surface`): Blender's Workbench clears to
    // `World.color` (viewport display), not the Background node colour that `RWorld::Color`
    // carries; an HDRI world falls back to the factory grey.
    let rgb = match &scene.world {
        RWorld::Color { rgb, .. } => *rgb,
        _ => FACTORY_WORLD_COLOR,
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
}

/// Sub-pixel offsets (in pixels, within `[-0.5, 0.5)`) of `n` samples: the pixel centre for a
/// single sample, else the Halton (2, 3) sequence from index 1.
pub fn jitter(n: u32) -> Vec<[f32; 2]> {
    if n <= 1 {
        return vec![[0.0, 0.0]];
    }
    let halton = |mut i: u32, base: u32| {
        let (mut f, mut r) = (1.0f64, 0.0f64);
        while i > 0 {
            f /= f64::from(base);
            r += f * f64::from(i % base);
            i /= base;
        }
        r
    };
    (1..=n)
        .map(|i| [(halton(i, 2) - 0.5) as f32, (halton(i, 3) - 0.5) as f32])
        .collect()
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct LightU {
    direction: [f32; 4],
    specular_color: [f32; 4],
    diffuse_color_wrap: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameU {
    view: [f32; 16],
    proj: [f32; 16],
    lights: [LightU; 4],
    ambient_color: [f32; 4],
    mode: [u32; 4],
    accum: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawU {
    model: [f32; 16],
    normal_matrix: [f32; 16],
    base_color: [f32; 4],
    params: [f32; 4],
}

struct DrawCmd {
    mesh: MeshKey,
    range: Range<u32>,
}

struct Pipelines {
    device: wgpu::Device,
    frame_bgl: wgpu::BindGroupLayout,
    draw_bgl: wgpu::BindGroupLayout,
    accum_bgl: wgpu::BindGroupLayout,
    geom: wgpu::RenderPipeline,
    accum: wgpu::RenderPipeline,
}

/// The Workbench engine. Pipelines are created on the first render and reused.
pub struct Workbench {
    samples: u32,
    pipes: Option<Pipelines>,
    pipeline_builds: u32,
}

impl Default for Workbench {
    fn default() -> Self {
        Workbench::new()
    }
}

impl Workbench {
    /// An engine rendering [`DEFAULT_SAMPLES`] samples per pixel.
    pub fn new() -> Workbench {
        Workbench::with_samples(DEFAULT_SAMPLES)
    }

    /// An engine rendering `samples` (≥ 1) samples per pixel; 1 samples pixel centres only
    /// (Blender `render_aa = 'OFF'`).
    pub fn with_samples(samples: u32) -> Workbench {
        Workbench {
            samples: samples.max(1),
            pipes: None,
            pipeline_builds: 0,
        }
    }

    /// How many times the pipeline set (one geometry + one accumulation pipeline from the one
    /// uber-shader) was built: once per device, never per material or lighting mode.
    pub fn pipeline_builds(&self) -> u32 {
        self.pipeline_builds
    }

    /// Samples per pixel.
    pub fn samples(&self) -> u32 {
        self.samples
    }

    fn pipelines(&mut self, gpu: &Gpu) -> &Pipelines {
        if self.pipes.as_ref().is_none_or(|p| p.device != gpu.device) {
            self.pipes = Some(create_pipelines(gpu));
            self.pipeline_builds += 1;
        }
        self.pipes.as_ref().expect("invariant: just created")
    }
}

fn uniform_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    size: usize,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(size as u64),
        },
        count: None,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn create_pipelines(gpu: &Gpu) -> Pipelines {
    let dev = &gpu.device;
    let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frap workbench"),
        source: wgpu::ShaderSource::Wgsl(SHADER_WGSL.into()),
    });
    let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
    let frame_size = std::mem::size_of::<FrameU>();
    let frame_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench frame"),
        entries: &[uniform_entry(0, vf, frame_size)],
    });
    let draw_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench draw"),
        entries: &[uniform_entry(0, vf, std::mem::size_of::<DrawU>())],
    });
    let accum_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench accumulate"),
        entries: &[
            uniform_entry(0, vf, frame_size),
            texture_entry(1),
            texture_entry(2),
        ],
    });
    let geom_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap workbench geometry"),
        bind_group_layouts: &[Some(&frame_bgl), Some(&draw_bgl)],
        immediate_size: 0,
    });
    let accum_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap workbench accumulate"),
        bind_group_layouts: &[Some(&accum_bgl)],
        immediate_size: 0,
    });
    let attributes = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x3,
            offset: 12,
            shader_location: 1,
        },
    ];
    let color_target = [Some(wgpu::ColorTargetState {
        format: COLOR_FORMAT,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    })];
    let geom = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("frap workbench geometry"),
        layout: Some(&geom_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_geom"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: VERTEX_STRIDE,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &attributes,
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_geom"),
            compilation_options: Default::default(),
            targets: &color_target,
        }),
        multiview_mask: None,
        cache: None,
    });
    let accum = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("frap workbench accumulate"),
        layout: Some(&accum_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_full"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_accum"),
            compilation_options: Default::default(),
            targets: &color_target,
        }),
        multiview_mask: None,
        cache: None,
    });
    Pipelines {
        device: dev.clone(),
        frame_bgl,
        draw_bgl,
        accum_bgl,
        geom,
        accum,
    }
}

fn align_up(v: u64, align: u64) -> u64 {
    v.div_ceil(align) * align
}

fn texture(
    dev: &wgpu::Device,
    label: &str,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    dev.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// Pads `items` (each `size_of::<T>()` bytes) to `stride` bytes apiece.
fn strided<T: Pod>(items: &[T], stride: u64) -> Vec<u8> {
    let mut out = vec![0u8; items.len().max(1) * stride as usize];
    for (i, it) in items.iter().enumerate() {
        let b = bytemuck::bytes_of(it);
        let at = i * stride as usize;
        out[at..at + b.len()].copy_from_slice(b);
    }
    out
}

fn frame_uniforms(scene: &RenderScene, shading: &Shading, samples: u32) -> Vec<FrameU> {
    let [w, h] = scene.resolution;
    let lights = std::array::from_fn(|i| {
        let l = &shading.studio.lights[i];
        if l.use_light {
            LightU {
                direction: [l.direction[0], l.direction[1], l.direction[2], 0.0],
                specular_color: [l.specular[0], l.specular[1], l.specular[2], 0.0],
                diffuse_color_wrap: [l.diffuse[0], l.diffuse[1], l.diffuse[2], l.smooth],
            }
        } else {
            // `get_light_data_from_studio_solidlight` for a disabled light.
            LightU {
                direction: [1.0, 0.0, 0.0, 0.0],
                specular_color: [0.0; 4],
                diffuse_color_wrap: [0.0; 4],
            }
        }
    });
    let a = shading.studio.ambient;
    let offsets = jitter(samples);
    let weight = 1.0 / offsets.len() as f32;
    offsets
        .iter()
        .map(|&[dx, dy]| {
            // Clip-space translation proportional to w: shifts the image by (dx, dy) pixels
            // (framebuffer y points down, clip y up).
            let t =
                Mat4::from_translation(Vec3::new(2.0 * dx / w as f32, -2.0 * dy / h as f32, 0.0));
            FrameU {
                view: scene.camera.view.to_cols_array(),
                proj: (t * scene.camera.projection).to_cols_array(),
                lights,
                ambient_color: [a[0], a[1], a[2], 0.0],
                mode: [
                    shading.lighting.shader_value(),
                    u32::from(shading.use_specular),
                    u32::from(scene.camera.is_perspective),
                    0,
                ],
                accum: [weight, 0.0, 0.0, 0.0],
            }
        })
        .collect()
}

impl Engine for Workbench {
    fn render(
        &mut self,
        gpu: &Gpu,
        cache: &mut GpuCache,
        scene: &RenderScene,
    ) -> Result<Film, RenderError> {
        let [width, height] = scene.resolution;
        let max = gpu.device.limits().max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(RenderError::Unsupported(format!(
                "resolution {width}x{height} (this adapter renders 1..={max} pixels per side)"
            )));
        }
        cache.begin_render();
        let shading = Shading::from_settings(&scene.settings);

        // Draw list: one draw per object and material slot range.
        let view = scene.camera.view;
        let mut draws: Vec<DrawCmd> = vec![];
        let mut draw_data: Vec<DrawU> = vec![];
        for obj in &scene.objects {
            let Some(key) = obj.mesh else { continue };
            let rmesh = scene.meshes.get(&key).ok_or_else(|| {
                RenderError::Unsupported(format!(
                    "object {:?} names mesh {key:?}, which the render scene lacks",
                    obj.name
                ))
            })?;
            let Some(gm) = cache.mesh(gpu, key, rmesh) else {
                continue;
            };
            let count = gm.index_count;
            let mv3 = Mat3::from_mat4(view * obj.matrix_world);
            let det = mv3.determinant();
            if !det.is_finite() || det == 0.0 {
                continue; // Zero scale: nothing visible.
            }
            let normal_matrix = Mat4::from_mat3(mv3.inverse().transpose());
            let flip = if det < 0.0 { 1.0 } else { 0.0 };
            let full = [(0u32, 0..count)];
            let subs: &[(u32, Range<u32>)] = if rmesh.submeshes.is_empty() {
                &full
            } else {
                &rmesh.submeshes
            };
            for (slot, range) in subs {
                let range = range.start.min(count)..range.end.min(count);
                if range.is_empty() {
                    continue;
                }
                let s = surface(scene, obj, *slot, &shading);
                draw_data.push(DrawU {
                    model: obj.matrix_world.to_cols_array(),
                    normal_matrix: normal_matrix.to_cols_array(),
                    base_color: s.base_color,
                    params: [s.metallic, s.roughness, flip, 0.0],
                });
                draws.push(DrawCmd { mesh: key, range });
            }
        }

        let samples = self.samples;
        let pipes = self.pipelines(gpu);
        let dev = &gpu.device;
        let align = u64::from(dev.limits().min_uniform_buffer_offset_alignment);
        let frame_stride = align_up(std::mem::size_of::<FrameU>() as u64, align);
        let draw_stride = align_up(std::mem::size_of::<DrawU>() as u64, align);
        let frames = frame_uniforms(scene, &shading, samples);

        use wgpu::util::DeviceExt;
        let frame_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frap workbench frames"),
            contents: &strided(&frames, frame_stride),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let draw_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("frap workbench draws"),
            contents: &strided(&draw_data, draw_stride),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let frame_binding = wgpu::BufferBinding {
            buffer: &frame_buf,
            offset: 0,
            size: wgpu::BufferSize::new(std::mem::size_of::<FrameU>() as u64),
        };
        let draw_binding = wgpu::BufferBinding {
            buffer: &draw_buf,
            offset: 0,
            size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64),
        };

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let tu = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let sample_tex = texture(dev, "frap workbench sample", size, COLOR_FORMAT, tu);
        let depth_tex = texture(
            dev,
            "frap workbench depth",
            size,
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let acc_tex = [0, 1].map(|_| {
            texture(
                dev,
                "frap workbench accumulator",
                size,
                COLOR_FORMAT,
                tu | wgpu::TextureUsages::COPY_SRC,
            )
        });
        let dv = wgpu::TextureViewDescriptor::default();
        let sample_view = sample_tex.create_view(&dv);
        let depth_view = depth_tex.create_view(&dv);
        let acc_view = [acc_tex[0].create_view(&dv), acc_tex[1].create_view(&dv)];

        let frame_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frap workbench frame"),
            layout: &pipes.frame_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(frame_binding.clone()),
            }],
        });
        let draw_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frap workbench draw"),
            layout: &pipes.draw_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(draw_binding),
            }],
        });
        // accum_bg[i] reads accumulator i and writes accumulator 1 - i.
        let accum_bg = [0usize, 1].map(|i| {
            dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frap workbench accumulate"),
                layout: &pipes.accum_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(frame_binding.clone()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&acc_view[i]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&sample_view),
                    },
                ],
            })
        });

        let bg = background(scene);
        let clear_bg = wgpu::Color {
            r: f64::from(bg[0]),
            g: f64::from(bg[1]),
            b: f64::from(bg[2]),
            a: f64::from(bg[3]),
        };
        let color_attachment = |view, load| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load,
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frap workbench"),
        });
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frap workbench clear accumulator"),
            color_attachments: &[color_attachment(
                &acc_view[0],
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            )],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for i in 0..frames.len() {
            let frame_offset = (i as u64 * frame_stride) as u32;
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("frap workbench opaque"),
                    color_attachments: &[color_attachment(
                        &sample_view,
                        wgpu::LoadOp::Clear(clear_bg),
                    )],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&pipes.geom);
                pass.set_bind_group(0, &frame_bg, &[frame_offset]);
                for (d, cmd) in draws.iter().enumerate() {
                    let gm = cache
                        .get(cmd.mesh)
                        .expect("invariant: uploaded while building the draw list");
                    pass.set_bind_group(1, &draw_bg, &[(d as u64 * draw_stride) as u32]);
                    pass.set_vertex_buffer(0, gm.vertices.slice(..));
                    pass.set_index_buffer(gm.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(cmd.range.clone(), 0, 0..1);
                }
            }
            {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("frap workbench accumulate"),
                    color_attachments: &[color_attachment(
                        &acc_view[(i + 1) % 2],
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    )],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&pipes.accum);
                pass.set_bind_group(0, &accum_bg[i % 2], &[frame_offset]);
                pass.draw(0..3, 0..1);
            }
        }
        gpu.queue.submit([enc.finish()]);

        let result = &acc_tex[frames.len() % 2];
        let mut rgba = gpu.read_texture(result, width, height, COLOR_FORMAT)?;
        for p in &mut rgba {
            *p = unpremultiply(*p);
        }
        Ok(Film {
            width,
            height,
            rgba,
        })
    }
}

/// Premultiplied accumulator texel → straight alpha (alpha clamped to `[0, 1]`).
fn unpremultiply(p: [f32; 4]) -> [f32; 4] {
    let a = p[3].clamp(0.0, 1.0);
    if a <= 0.0 {
        return [0.0; 4];
    }
    if a >= 1.0 {
        return [p[0], p[1], p[2], 1.0];
    }
    [p[0] / p[3], p[1] / p[3], p[2] / p[3], a]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_layouts_match_wgsl() {
        // WGSL sizes: Frame = 2 mat4 (128) + 4 × Light (48) + 3 vec4 (48) = 368; Draw = 160.
        assert_eq!(std::mem::size_of::<FrameU>(), 368);
        assert_eq!(std::mem::size_of::<DrawU>(), 160);
    }

    #[test]
    fn jitter_is_fixed_and_centred() {
        assert_eq!(jitter(1), vec![[0.0, 0.0]]);
        let j = jitter(8);
        assert_eq!(j.len(), 8);
        assert_eq!(j[0], [0.0, -1.0 / 6.0]);
        assert!(j.iter().all(|p| p.iter().all(|c| (-0.5..0.5).contains(c))));
        let mean: f32 = j.iter().map(|p| p[0]).sum::<f32>() / 8.0;
        assert!(mean.abs() < 0.1, "{mean}");
        assert_eq!(jitter(8), j);
    }

    #[test]
    fn unpremultiply_edges() {
        assert_eq!(unpremultiply([0.0; 4]), [0.0; 4]);
        assert_eq!(unpremultiply([0.2, 0.4, 0.6, 1.0]), [0.2, 0.4, 0.6, 1.0]);
        assert_eq!(unpremultiply([0.1, 0.2, 0.3, 0.5]), [0.2, 0.4, 0.6, 0.5]);
    }

    #[test]
    fn shading_from_settings() {
        let mut s = RSettings::default();
        let d = Shading::from_settings(&s);
        assert_eq!(
            (d.lighting, d.color_type, d.use_specular),
            (Lighting::Studio, ColorType::Material, true)
        );
        assert_eq!(d.studio.name, "Default");
        s.shading_light = "FLAT".into();
        s.shading_color_type = "RANDOM".into();
        let f = Shading::from_settings(&s);
        assert_eq!(
            (f.lighting, f.color_type),
            (Lighting::Flat, ColorType::Random)
        );
        s.shading_light = "MATCAP".into();
        s.shading_color_type = "TEXTURE".into();
        let m = Shading::from_settings(&s);
        assert_eq!(
            (m.lighting, m.color_type),
            (Lighting::Studio, ColorType::Material)
        );
    }

    #[test]
    fn background_rules() {
        let mut sc = RenderScene::default();
        assert_eq!(background(&sc), [0.050876, 0.050876, 0.050876, 1.0]);
        sc.world = RWorld::Color {
            rgb: [0.1, 0.2, 0.3],
            strength: 5.0,
        };
        assert_eq!(background(&sc), [0.1, 0.2, 0.3, 1.0]);
        sc.settings.film_transparent = true;
        assert_eq!(background(&sc), [0.0; 4]);
    }
}
