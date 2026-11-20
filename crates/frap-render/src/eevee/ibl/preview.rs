// SPDX-License-Identifier: GPL-3.0-or-later
//! IBL preview renderer (tests only, `shaders/preview.wgsl`): analytic spheres and ground
//! quads lit by a [`WorldLighting`], used to compare the world lighting with Blender's EEVEE
//! renders of the `render_eevee/world` golden scenes.

use bytemuck::{Pod, Zeroable};
use frap_render_scene::{RLightKind, RenderError};
use glam::{Mat3, Vec3};
use wgpu::util::DeviceExt;

use super::WorldLighting;
use crate::gpu::Gpu;

/// A Principled BSDF with Base Color, Metallic, Roughness and IOR (Blender's defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub ior: f32,
}

impl Default for Material {
    /// `Principled BSDF` defaults (also EEVEE's default surface).
    fn default() -> Self {
        Material {
            base_color: [0.8, 0.8, 0.8],
            metallic: 0.0,
            roughness: 0.5,
            ior: 1.5,
        }
    }
}

/// A shape of the preview.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A sphere (centre, radius).
    Sphere([f32; 3], f32),
    /// A horizontal square (centre, half size), normal +Z.
    Quad([f32; 3], f32),
}

/// A Blender camera and the shapes it sees.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub width: u32,
    pub height: u32,
    pub cam_location: [f32; 3],
    /// XYZ Euler (radians).
    pub cam_rotation: [f32; 3],
    /// `lens` (mm), `sensor_width` (mm), `sensor_fit = 'AUTO'`.
    pub lens: f32,
    pub sensor: f32,
    pub shapes: Vec<(Shape, Material)>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    cam_back: [f32; 4],
    cam_pos: [f32; 4],
    frustum: [f32; 4],
    sh: [[f32; 4]; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    misc: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuShape {
    geo: [f32; 4],
    mat0: [f32; 4],
    mat1: [f32; 4],
}

fn gpu_shape(c: [f32; 3], s: f32, m: &Material) -> GpuShape {
    GpuShape {
        geo: [c[0], c[1], c[2], s],
        mat0: [
            m.base_color[0],
            m.base_color[1],
            m.base_color[2],
            m.metallic,
        ],
        mat1: [m.roughness, m.ior, 0.0, 0.0],
    }
}

/// Renders `scene` to scene-linear RGBA (top row first).
pub fn render(
    gpu: &Gpu,
    world: &WorldLighting,
    scene: &Scene,
) -> Result<Vec<[f32; 4]>, RenderError> {
    let dev = &gpu.device;
    let src = format!(
        "{}\n{}\n{}",
        super::WGSL,
        super::SHADING_WGSL,
        include_str!("shaders/preview.wgsl")
    );
    let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frap ibl preview"),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    });
    let pipeline = dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("frap ibl preview"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let r = scene.cam_rotation;
    let rot =
        Mat3::from_rotation_z(r[2]) * Mat3::from_rotation_y(r[1]) * Mat3::from_rotation_x(r[0]);
    let tan_x = scene.sensor * 0.5 / scene.lens;
    let (tx, ty) = if scene.width >= scene.height {
        (tan_x, tan_x * scene.height as f32 / scene.width as f32)
    } else {
        (tan_x * scene.width as f32 / scene.height as f32, tan_x)
    };
    let v4 = |v: Vec3| [v.x, v.y, v.z, 0.0];
    let (sun_dir, sun_color) = match &world.sun {
        Some(l) if matches!(l.kind, RLightKind::Sun { .. }) => {
            let z = l.matrix_world.z_axis.truncate().normalize();
            (
                [z.x, z.y, z.z, 1.0],
                [
                    l.color[0] * l.energy,
                    l.color[1] * l.energy,
                    l.color[2] * l.energy,
                    0.0,
                ],
            )
        }
        _ => ([0.0; 4], [0.0; 4]),
    };
    let mut spheres = vec![];
    let mut quads = vec![];
    for (s, m) in &scene.shapes {
        match *s {
            Shape::Sphere(c, r) => spheres.push(gpu_shape(c, r, m)),
            Shape::Quad(c, h) => quads.push(gpu_shape(c, h, m)),
        }
    }
    let uniforms = Uniforms {
        cam_right: v4(rot * Vec3::X),
        cam_up: v4(rot * Vec3::Y),
        cam_back: v4(rot * Vec3::Z),
        cam_pos: v4(Vec3::from(scene.cam_location)),
        frustum: [tx, ty, scene.width as f32, scene.height as f32],
        sh: std::array::from_fn(|k| world.sh9[k]),
        sun_dir,
        sun_color,
        misc: [
            world.rotation,
            spheres.len() as f32,
            quads.len() as f32,
            0.0,
        ],
    };
    // Storage buffers may not be empty.
    let pad = |v: &mut Vec<GpuShape>| {
        if v.is_empty() {
            v.push(GpuShape::zeroed());
        }
    };
    pad(&mut spheres);
    pad(&mut quads);
    let buf = |label: &str, bytes: &[u8], usage| {
        dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytes,
            usage,
        })
    };
    let ubuf = buf(
        "preview uniforms",
        bytemuck::bytes_of(&uniforms),
        wgpu::BufferUsages::UNIFORM,
    );
    let sbuf = buf(
        "preview spheres",
        bytemuck::cast_slice(&spheres),
        wgpu::BufferUsages::STORAGE,
    );
    let qbuf = buf(
        "preview quads",
        bytemuck::cast_slice(&quads),
        wgpu::BufferUsages::STORAGE,
    );
    let out = dev.create_texture(&wgpu::TextureDescriptor {
        label: Some("frap ibl preview"),
        size: wgpu::Extent3d {
            width: scene.width,
            height: scene.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let out_view = out.create_view(&Default::default());
    let bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("frap ibl preview"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: ubuf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: sbuf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: qbuf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&world.specular),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&world.brdf_lut),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&world.bsdf_lut),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(&world.background),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(&world.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::Sampler(&world.background_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(&out_view),
            },
        ],
    });
    let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("frap ibl preview"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.dispatch_workgroups(scene.width.div_ceil(8), scene.height.div_ceil(8), 1);
    }
    gpu.queue.submit([enc.finish()]);
    gpu.read_texture(
        &out,
        scene.width,
        scene.height,
        wgpu::TextureFormat::Rgba32Float,
    )
}
