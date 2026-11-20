// SPDX-License-Identifier: GPL-3.0-or-later
//! The world probe bake on the GPU (`shaders/bake.wgsl`), following EEVEE's world sphere-probe
//! update (blender/source/blender/draw/engines/eevee/eevee_lightprobe_sphere.cc,
//! `SphereProbeModule::remap_to_octahedral_projection`):
//!
//! 1. the world (environment × strength) is drawn into a cube map of `probe_extent / 2` per
//!    face (`update_info_from_probe`: `cube_target_extent = area_extent / 2`);
//! 2. `remap` writes the octahedral mip 0 (`probe_extent`², radiance split at the world's
//!    `sun_threshold`, clamped by `clamp_surface_indirect`) and per-work-group partial sums of
//!    the L1 harmonics and of the sun;
//! 3. `convolve` fills mips 1–4 from the previous mip (196 spherical-gaussian samples);
//! 4. each octahedral mip is resampled into the matching mip of the specular cube map (the
//!    `WorldLighting::specular` contract), face size `probe_extent / 2 >> mip`;
//! 5. the partial sums are read back and reduced on the CPU in a fixed order (`irradiance_sum`,
//!    `sun_extraction`), then the harmonics are deringed (`load_world`).

use bytemuck::{Pod, Zeroable};
use frap_render_scene::{RLight, RLightEevee, RLightKind, RenderError};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use super::equirect::LinearImage;
use super::octahedral::MIPMAP_LEVELS;
use super::sh::ShL1;
use crate::gpu::Gpu;

/// The bake's shader source (`common.wgsl` + `bake.wgsl`).
pub(crate) fn shader_source() -> String {
    format!(
        "{}\n{}",
        include_str!("shaders/common.wgsl"),
        include_str!("shaders/bake.wgsl")
    )
}

/// `World` settings the world probe reads (Blender's defaults: frap's `World` does not bind
/// `probe_resolution`, `sun_threshold`, `sun_angle` or the sun shadow settings yet, so a render
/// always uses `World()`'s values).
#[derive(Clone, Debug, PartialEq)]
pub struct WorldProbeSettings {
    /// Octahedral extent of mip 0 (`World.probe_resolution`, `'1024'`).
    pub probe_extent: u32,
    /// `World.sun_threshold` (10); 0 disables the sun extraction.
    pub sun_threshold: f32,
    /// `World.sun_angle` (0.00918 rad).
    pub sun_angle: f32,
    /// `World.use_sun_shadow`
    pub use_sun_shadow: bool,
    /// `World.sun_shadow_maximum_resolution`
    pub sun_shadow_maximum_resolution: f32,
    /// `World.sun_shadow_filter_radius`
    pub sun_shadow_filter_radius: f32,
    /// `World.use_sun_shadow_jitter`
    pub use_sun_shadow_jitter: bool,
    /// `World.sun_shadow_jitter_overblur`
    pub sun_shadow_jitter_overblur: f32,
    /// `scene.eevee.clamp_surface_indirect` (0 = off).
    pub clamp_surface_indirect: f32,
}

impl Default for WorldProbeSettings {
    fn default() -> Self {
        WorldProbeSettings {
            probe_extent: 1024,
            sun_threshold: 10.0,
            sun_angle: 0.009_180_432,
            use_sun_shadow: true,
            sun_shadow_maximum_resolution: 0.001,
            sun_shadow_filter_radius: 1.0,
            use_sun_shadow_jitter: false,
            sun_shadow_jitter_overblur: 10.0,
            clamp_surface_indirect: 10.0,
        }
    }
}

/// `clamp_value_load` (eevee_sampling.cc): 0 means "no clamp".
fn clamp_value_load(v: f32) -> f32 {
    if v > 0.0 { v } else { 1e20 }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    face_size: i32,
    mip0_extent: i32,
    read_lod: i32,
    groups_x: i32,
    rotation: f32,
    sun_threshold: f32,
    clamp_indirect: f32,
    _pad: f32,
}

/// One work group's partial sums (`Partial` in bake.wgsl).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Partial {
    sh: [[f32; 4]; 4],
    sun_radiance: [f32; 4],
    sun_direction: [f32; 4],
}

/// Remap work-group side (`bake.wgsl::remap`).
const REMAP_GROUP: u32 = 16;

/// The baked world probe.
pub(crate) struct Baked {
    /// The environment as uploaded (equirectangular, `Rgba16Float`).
    pub background: wgpu::Texture,
    /// The prefiltered specular cube map (6 layers, [`MIPMAP_LEVELS`] mips).
    pub specular: wgpu::Texture,
    /// The octahedral probe ([`MIPMAP_LEVELS`] mips), kept for tests.
    #[cfg_attr(not(test), allow(dead_code))]
    pub octahedral: wgpu::Texture,
    /// Deringed L1 harmonics of the (sun-clamped) world radiance.
    pub sh: ShL1,
    /// The extracted world sun.
    pub sun: Option<RLight>,
}

fn storage_view(
    tex: &wgpu::Texture,
    mip: u32,
    dim: wgpu::TextureViewDimension,
) -> wgpu::TextureView {
    tex.create_view(&wgpu::TextureViewDescriptor {
        label: Some("frap ibl storage"),
        dimension: Some(dim),
        base_mip_level: mip,
        mip_level_count: Some(1),
        ..Default::default()
    })
}

struct Ctx<'a> {
    gpu: &'a Gpu,
    module: wgpu::ShaderModule,
}

impl Ctx<'_> {
    fn pipeline(&self, entry: &str) -> wgpu::ComputePipeline {
        self.gpu
            .device
            .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &self.module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
    }

    fn params(&self, p: Params) -> wgpu::Buffer {
        self.gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frap ibl params"),
                contents: bytemuck::bytes_of(&p),
                usage: wgpu::BufferUsages::UNIFORM,
            })
    }

    fn bind(
        &self,
        pipeline: &wgpu::ComputePipeline,
        entries: &[(u32, wgpu::BindingResource<'_>)],
    ) -> wgpu::BindGroup {
        let entries: Vec<wgpu::BindGroupEntry<'_>> = entries
            .iter()
            .map(|(b, r)| wgpu::BindGroupEntry {
                binding: *b,
                resource: r.clone(),
            })
            .collect();
        self.gpu
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frap ibl"),
                layout: &pipeline.get_bind_group_layout(0),
                entries: &entries,
            })
    }
}

fn dispatch(
    enc: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bg: &wgpu::BindGroup,
    groups: [u32; 3],
) {
    let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("frap ibl"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bg, &[]);
    pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
}

/// A 2D texture of `layers` layers and `mips` mips, `Rgba16Float`, sampled and written.
fn probe_texture(gpu: &Gpu, label: &str, size: u32, layers: u32, mips: u32) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba16Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// A linear, clamped sampler (mips linear).
pub(crate) fn clamp_sampler(gpu: &Gpu) -> wgpu::Sampler {
    gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap ibl clamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

/// The world texture's sampler: linear, `u` repeating, `v` clamped.
pub(crate) fn background_sampler(gpu: &Gpu) -> wgpu::Sampler {
    gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap ibl background"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    })
}

/// Uploads the environment (`Rgba16Float`, one mip).
pub(crate) fn upload_background(gpu: &Gpu, image: &LinearImage) -> wgpu::Texture {
    gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("frap ibl background"),
            size: wgpu::Extent3d {
                width: image.width,
                height: image.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &image.to_rgba16f(),
    )
}

/// Bakes the world probe of `image` (already × strength) seen through `Rz(rotation)`.
pub(crate) fn bake(
    gpu: &Gpu,
    image: &LinearImage,
    rotation: f32,
    settings: &WorldProbeSettings,
) -> Result<Baked, RenderError> {
    let dev = &gpu.device;
    let extent = settings.probe_extent.max(64).next_power_of_two();
    let face = extent / 2;
    let ctx = Ctx {
        gpu,
        module: dev.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("frap ibl bake"),
            source: wgpu::ShaderSource::Wgsl(shader_source().into()),
        }),
    };
    let background = upload_background(gpu, image);
    let bg_view = background.create_view(&Default::default());
    let cube = probe_texture(gpu, "frap ibl world cube", face, 6, 1);
    let octahedral = probe_texture(gpu, "frap ibl octahedral", extent, 1, MIPMAP_LEVELS);
    let specular = probe_texture(gpu, "frap ibl specular", face, 6, MIPMAP_LEVELS);
    let clamp = clamp_sampler(gpu);
    let repeat = background_sampler(gpu);
    let groups_x = extent / REMAP_GROUP;
    let group_count = (groups_x * groups_x) as u64;
    let partial_size = std::mem::size_of::<Partial>() as u64;
    let partials = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap ibl partial sums"),
        size: group_count * partial_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let staging = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap ibl partial readback"),
        size: group_count * partial_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let base = Params {
        face_size: face as i32,
        mip0_extent: extent as i32,
        read_lod: 0,
        groups_x: groups_x as i32,
        rotation,
        sun_threshold: clamp_value_load(settings.sun_threshold),
        clamp_indirect: clamp_value_load(settings.clamp_surface_indirect),
        _pad: 0.0,
    };

    let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("frap ibl bake"),
    });
    // 1. World → cube.
    {
        let p = ctx.pipeline("world_to_cube");
        let params = ctx.params(base);
        let out = storage_view(&cube, 0, wgpu::TextureViewDimension::D2Array);
        let bg = ctx.bind(
            &p,
            &[
                (0, params.as_entire_binding()),
                (1, wgpu::BindingResource::Sampler(&repeat)),
                (2, wgpu::BindingResource::TextureView(&bg_view)),
                (3, wgpu::BindingResource::TextureView(&out)),
            ],
        );
        dispatch(&mut enc, &p, &bg, [face.div_ceil(8), face.div_ceil(8), 6]);
    }
    // 2. Cube → octahedral mip 0 + partial sums.
    {
        let p = ctx.pipeline("remap");
        let params = ctx.params(base);
        let cube_view = cube.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let out = storage_view(&octahedral, 0, wgpu::TextureViewDimension::D2);
        let bg = ctx.bind(
            &p,
            &[
                (0, params.as_entire_binding()),
                (1, wgpu::BindingResource::Sampler(&clamp)),
                (4, wgpu::BindingResource::TextureView(&cube_view)),
                (5, wgpu::BindingResource::TextureView(&out)),
                (6, partials.as_entire_binding()),
            ],
        );
        dispatch(&mut enc, &p, &bg, [groups_x, groups_x, 1]);
    }
    // 3. Mip convolution.
    {
        let p = ctx.pipeline("convolve");
        for lod in 0..MIPMAP_LEVELS - 1 {
            let params = ctx.params(Params {
                read_lod: lod as i32,
                ..base
            });
            let input = storage_view(&octahedral, lod, wgpu::TextureViewDimension::D2);
            let out = storage_view(&octahedral, lod + 1, wgpu::TextureViewDimension::D2);
            let bg = ctx.bind(
                &p,
                &[
                    (0, params.as_entire_binding()),
                    (1, wgpu::BindingResource::Sampler(&clamp)),
                    (5, wgpu::BindingResource::TextureView(&out)),
                    (7, wgpu::BindingResource::TextureView(&input)),
                ],
            );
            let n = (extent >> (lod + 1)).div_ceil(16);
            dispatch(&mut enc, &p, &bg, [n, n, 1]);
        }
    }
    // 4. Octahedral mips → specular cube mips.
    {
        let p = ctx.pipeline("octahedral_to_cube");
        for lod in 0..MIPMAP_LEVELS {
            let size = face >> lod;
            let params = ctx.params(Params {
                face_size: size as i32,
                read_lod: lod as i32,
                ..base
            });
            let input = storage_view(&octahedral, lod, wgpu::TextureViewDimension::D2);
            let out = storage_view(&specular, lod, wgpu::TextureViewDimension::D2Array);
            let bg = ctx.bind(
                &p,
                &[
                    (0, params.as_entire_binding()),
                    (1, wgpu::BindingResource::Sampler(&clamp)),
                    (3, wgpu::BindingResource::TextureView(&out)),
                    (7, wgpu::BindingResource::TextureView(&input)),
                ],
            );
            dispatch(&mut enc, &p, &bg, [size.div_ceil(8), size.div_ceil(8), 6]);
        }
    }
    enc.copy_buffer_to_buffer(&partials, 0, &staging, 0, group_count * partial_size);
    gpu.queue.submit([enc.finish()]);

    let parts: Vec<Partial> = map_read(gpu, &staging, |b| bytemuck::cast_slice(b).to_vec())?;
    let (sh, sun) = reduce(&parts);
    let sun = if settings.sun_threshold > 0.0 {
        sun.and_then(|s| world_sun(s.color, s.direction, settings))
    } else {
        None
    };
    Ok(Baked {
        background,
        specular,
        octahedral,
        sh: sh.dering(),
        sun,
    })
}

/// Maps `buf` for reading after the submitted work completes.
pub(crate) fn map_read<T>(
    gpu: &Gpu,
    buf: &wgpu::Buffer,
    f: impl FnOnce(&[u8]) -> T,
) -> Result<T, RenderError> {
    let slice = buf.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    rx.recv()
        .map_err(|e| RenderError::Readback(e.to_string()))?
        .map_err(|e| RenderError::Readback(e.to_string()))?;
    let out = {
        let data = slice
            .get_mapped_range()
            .map_err(|e| RenderError::Readback(format!("{e:?}")))?;
        f(&data)
    };
    buf.unmap();
    Ok(out)
}

/// The summed world sun: radiance × solid angle (irradiance at normal incidence) and the
/// weighted direction sum (`xyz / w` is the mean direction).
#[derive(Clone, Copy, Debug, PartialEq)]
struct SunSum {
    color: [f32; 3],
    direction: [f32; 4],
}

/// `irradiance_sum` and the first half of `sun_extraction`: sums the work groups in index
/// order (in `f64`). Returns the harmonics and, when any radiance passed the threshold, the
/// sun's summed radiance (irradiance at normal incidence) and its mean direction.
fn reduce(parts: &[Partial]) -> (ShL1, Option<SunSum>) {
    let mut sh = [[0.0f64; 3]; 4];
    let mut rad = [0.0f64; 3];
    let mut dir = [0.0f64; 4];
    let add = |acc: &mut [f64], v: &[f32]| {
        for (a, x) in acc.iter_mut().zip(v) {
            *a += f64::from(*x);
        }
    };
    for p in parts {
        for (c, v) in sh.iter_mut().zip(&p.sh) {
            add(c, v);
        }
        add(&mut rad, &p.sun_radiance);
        add(&mut dir, &p.sun_direction);
    }
    let sh = ShL1::from_coefs(sh.map(|c| c.map(|x| x as f32)));
    let color = rad.map(|x| x as f32);
    let sun = (color.iter().any(|&c| c > 0.0) && dir[3] > 0.0).then(|| SunSum {
        color,
        direction: dir.map(|x| x as f32),
    });
    (sh, sun)
}

/// The second half of `sun_extraction` and `LightModule::add_world_sun_light`: the world sun
/// as a sun light. Its colour is the extracted irradiance (`sunlight_buf.color`, at strength 1);
/// the angle and shadow settings are the World's (`sun_angle` from the UI, not the automatic
/// size, as EEVEE does).
fn world_sun(color: [f32; 3], dir_sum: [f32; 4], s: &WorldProbeSettings) -> Option<RLight> {
    let d = Vec3::new(dir_sum[0], dir_sum[1], dir_sum[2]) / dir_sum[3];
    let len = d.length();
    if len.is_nan() || len <= 0.0 {
        return None;
    }
    let z = d / len;
    // `from_up_axis(direction)`: the light's local Z points at the sun (lights shine along -Z).
    let z_sign = if z.z >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (z_sign + z.z);
    let b = z.x * z.y * a;
    let x = Vec3::new(1.0 + z_sign * z.x * z.x * a, z_sign * b, -z_sign * z.x);
    let y = Vec3::new(b, z_sign + z.y * z.y * a, -z.y);
    let energy = color[0].max(color[1]).max(color[2]);
    let mut eevee = RLightEevee::default();
    eevee.shadow_filter_radius = s.sun_shadow_filter_radius;
    eevee.shadow_maximum_resolution = s.sun_shadow_maximum_resolution;
    eevee.use_shadow_jitter = s.use_sun_shadow_jitter;
    eevee.shadow_jitter_overblur = s.sun_shadow_jitter_overblur;
    Some(RLight {
        kind: RLightKind::Sun { angle: s.sun_angle },
        color: color.map(|c| c / energy),
        energy,
        radius: 0.0,
        use_shadow: s.use_sun_shadow,
        matrix_world: Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), Vec4::W),
        eevee,
    })
}

/// Reads mip `mip` of every layer of an `Rgba16Float` texture (tests).
#[cfg(test)]
pub(crate) fn read_rgba16f(
    gpu: &Gpu,
    tex: &wgpu::Texture,
    mip: u32,
) -> Result<Vec<Vec<[f32; 4]>>, RenderError> {
    let size = tex.size();
    let (w, h) = ((size.width >> mip).max(1), (size.height >> mip).max(1));
    let layers = size.depth_or_array_layers;
    let bpr =
        (w * 8).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap ibl texture readback"),
        size: u64::from(bpr) * u64::from(h) * u64::from(layers),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: mip,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bpr),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: layers,
        },
    );
    gpu.queue.submit([enc.finish()]);
    map_read(gpu, &buf, |data| {
        (0..layers as usize)
            .map(|l| {
                let mut out = Vec::with_capacity((w * h) as usize);
                for y in 0..h as usize {
                    let row = (l * h as usize + y) * bpr as usize;
                    for x in 0..w as usize {
                        let t = &data[row + x * 8..row + x * 8 + 8];
                        let c =
                            |i: usize| half::f16::from_le_bytes([t[2 * i], t[2 * i + 1]]).to_f32();
                        out.push([c(0), c(1), c(2), c(3)]);
                    }
                }
                out
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_sun_points_its_z_axis_at_the_sun() {
        let s = WorldProbeSettings::default();
        let l = world_sun([20.0, 10.0, 5.0], [0.0, 0.0, 2.0, 2.0], &s).unwrap();
        assert_eq!(l.energy, 20.0);
        assert_eq!(l.color, [1.0, 0.5, 0.25]);
        assert!((l.matrix_world.z_axis.truncate() - Vec3::Z).length() < 1e-6);
        assert!(matches!(l.kind, RLightKind::Sun { angle } if angle == s.sun_angle));
        let l = world_sun([1.0; 3], [0.0, -0.6, -0.8, 1.0], &s).unwrap();
        let m = l.matrix_world;
        assert!((m.z_axis.truncate() - Vec3::new(0.0, -0.6, -0.8)).length() < 1e-6);
        // Orthonormal, right-handed.
        assert!(
            (m.x_axis.truncate().cross(m.y_axis.truncate()) - m.z_axis.truncate()).length() < 1e-5
        );
        assert!(world_sun([1.0; 3], [0.0, 0.0, 0.0, 1.0], &s).is_none());
    }

    #[test]
    fn reduce_sums_in_order() {
        let mut p = Partial::zeroed();
        p.sh[0] = [1.0, 2.0, 3.0, 0.0];
        p.sun_direction = [0.0, 0.0, 1.0, 1.0];
        let (sh, sun) = reduce(&[p, p]);
        assert_eq!(sh.l0, [2.0, 4.0, 6.0]);
        assert!(sun.is_none(), "no radiance above the threshold");
        p.sun_radiance = [0.5, 0.0, 0.0, 0.0];
        let (_, sun) = reduce(&[p, p]);
        assert_eq!(
            sun,
            Some(SunSum {
                color: [1.0, 0.0, 0.0],
                direction: [0.0, 0.0, 2.0, 2.0]
            })
        );
        assert_eq!(clamp_value_load(0.0), 1e20);
        assert_eq!(clamp_value_load(3.0), 3.0);
    }
}
