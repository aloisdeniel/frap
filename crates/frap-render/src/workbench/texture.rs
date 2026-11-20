// SPDX-License-Identifier: GPL-3.0-or-later
//! The Workbench VERTEX and TEXTURE colour sources (WP 5.5): GPU buffers of the colour
//! attribute and UV map a draw reads, the images and samplers of TEXTURE draws.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later) `draw/engines/workbench/workbench_materials.cc`
//! (`MaterialTexture`: filtering from the Image Texture's `interpolation`, wrapping from its
//! `extension`, `alpha_cutoff` unless the image's alpha mode is `NONE` or `CHANNEL_PACKED`),
//! `workbench_mesh_passes.cc` (`image_transparency_cutoff` 0.1) and
//! `shaders/workbench_image.bsl.hh` (`image_color`). The sampler has no mipmap filtering
//! (`GPUSamplerState` without `GPU_SAMPLER_FILTERING_MIPMAP`): the image's full resolution is
//! sampled, bilinear or nearest. Images are uploaded scene linear (sRGB bytes decoded, as the
//! sRGB texture formats Blender uses decode before filtering) with straight alpha, which is
//! what `image_color` returns after its un-premultiplication.

use std::collections::{HashMap, HashSet};

use frap_imbuf::DecodedImage;
use frap_render_scene::{ImageKey, MeshKey, RMesh, RWorkbenchTexture, RenderScene};
use smol_str::SmolStr;
use wgpu::util::DeviceExt;

use crate::eevee::pipeline::image_linear;
use crate::gpu::Gpu;

/// No alpha cutoff (`-FLT_MAX`).
pub const NO_CUTOFF: f32 = -1e30;
/// `Draw.ids.z` bit: the image's `extension` is CLIP.
pub const IMAGE_CLIP: u32 = 1;

/// `image_transparency_cutoff` of a TEXTURE draw.
pub fn cutoff(t: &RWorkbenchTexture) -> f32 {
    if t.alpha_cutoff { 0.1 } else { NO_CUTOFF }
}

/// The `Draw.ids.z` bits of a TEXTURE draw.
pub fn image_bits(t: &RWorkbenchTexture) -> u32 {
    if t.extension == "CLIP" { IMAGE_CLIP } else { 0 }
}

/// The distinct textures of one render; slot 0 is the white placeholder of the other draws.
#[derive(Default)]
pub struct Slots {
    list: Vec<RWorkbenchTexture>,
}

impl Slots {
    /// The bind-group index of `t` (1 + its position among the distinct textures).
    pub fn slot(&mut self, t: &RWorkbenchTexture) -> usize {
        match self.list.iter().position(|x| x == t) {
            Some(i) => i + 1,
            None => {
                self.list.push(t.clone());
                self.list.len()
            }
        }
    }

    /// The distinct textures in slot order (slot `i + 1`).
    pub fn textures(&self) -> &[RWorkbenchTexture] {
        &self.list
    }
}

/// GPU resources of the colour sources, kept between renders while they are used.
#[derive(Default)]
pub struct Cache {
    attributes: HashMap<MeshKey, wgpu::Buffer>,
    uvs: HashMap<MeshKey, wgpu::Buffer>,
    images: HashMap<(ImageKey, bool), wgpu::TextureView>,
    samplers: HashMap<(bool, SmolStr), wgpu::Sampler>,
    zeros: Option<(usize, wgpu::Buffer)>,
    white: Option<wgpu::TextureView>,
    used_meshes: HashSet<MeshKey>,
}

fn buffer<T: bytemuck::Pod>(gpu: &Gpu, label: &str, data: &[T]) -> wgpu::Buffer {
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::VERTEX,
        })
}

/// `src` padded (or cut) to `n` values.
fn fit<T: Copy>(src: &[T], n: usize, fill: T) -> Vec<T> {
    let mut v: Vec<T> = src.iter().copied().take(n).collect();
    v.resize(n.max(1), fill);
    v
}

impl Cache {
    /// Starts a render.
    pub fn begin_render(&mut self) {
        self.used_meshes.clear();
    }

    /// Uploads the colour attribute `color_active` of `mesh` (linear RGBA per vertex).
    pub fn attribute(&mut self, gpu: &Gpu, key: MeshKey, mesh: &RMesh) {
        self.used_meshes.insert(key);
        if self.attributes.contains_key(&key) {
            return;
        }
        let n = mesh.positions.len();
        let src = mesh
            .color_active
            .and_then(|i| mesh.colors.get(i))
            .map_or(&[][..], Vec::as_slice);
        let data = fit(src, n, [0.0f32; 4]);
        self.attributes
            .insert(key, buffer(gpu, "frap workbench colour attribute", &data));
    }

    /// Uploads the UV map `uv_active` of `mesh`.
    pub fn uv(&mut self, gpu: &Gpu, key: MeshKey, mesh: &RMesh) {
        self.used_meshes.insert(key);
        if self.uvs.contains_key(&key) {
            return;
        }
        let n = mesh.positions.len();
        let src = mesh
            .uv_active
            .and_then(|i| mesh.uvs.get(i))
            .map_or(&[][..], Vec::as_slice);
        let data = fit(src, n, [0.0f32; 2]);
        self.uvs
            .insert(key, buffer(gpu, "frap workbench uv map", &data));
    }

    /// Makes the zero buffer bound where a draw reads no attribute cover `vertices` vertices.
    pub fn ensure_zeros(&mut self, gpu: &Gpu, vertices: usize) {
        let need = vertices.max(1);
        if self.zeros.as_ref().is_some_and(|(n, _)| *n >= need) {
            return;
        }
        let data = vec![[0.0f32; 4]; need];
        self.zeros = Some((need, buffer(gpu, "frap workbench zeros", &data)));
    }

    /// The zero buffer ([`Cache::ensure_zeros`] first).
    pub fn zeros(&self) -> &wgpu::Buffer {
        &self.zeros.as_ref().expect("invariant: ensure_zeros ran").1
    }

    /// The colour attribute buffer of a mesh.
    pub fn attribute_buffer(&self, key: MeshKey) -> Option<&wgpu::Buffer> {
        self.attributes.get(&key)
    }

    /// The UV buffer of a mesh.
    pub fn uv_buffer(&self, key: MeshKey) -> Option<&wgpu::Buffer> {
        self.uvs.get(&key)
    }

    /// The image bind groups of a render: slot 0 the white placeholder, then one per texture
    /// of `slots` (images uploaded on first use). Buffers and images this render does not
    /// use are dropped.
    pub fn bind_groups(
        &mut self,
        gpu: &Gpu,
        layout: &wgpu::BindGroupLayout,
        scene: &RenderScene,
        slots: &Slots,
    ) -> Vec<wgpu::BindGroup> {
        let used = &self.used_meshes;
        self.attributes.retain(|k, _| used.contains(k));
        self.uvs.retain(|k, _| used.contains(k));
        let wanted: HashSet<(ImageKey, bool)> = slots
            .textures()
            .iter()
            .map(|t| (t.image, t.is_data))
            .collect();
        self.images.retain(|k, _| wanted.contains(k));
        for t in slots.textures() {
            let k = (t.image, t.is_data);
            if !self.images.contains_key(&k)
                && let Some(img) = scene.images.get(&t.image)
            {
                self.images.insert(k, upload(gpu, img, t.is_data));
            }
            self.samplers
                .entry((t.linear, t.extension.clone()))
                .or_insert_with(|| sampler(gpu, t.linear, &t.extension));
        }
        let white = self.white.get_or_insert_with(|| {
            let img = DecodedImage {
                width: 1,
                height: 1,
                channels: 4,
                pixels: frap_imbuf::PixelData::F32(vec![1.0; 4]),
            };
            upload(gpu, &img, true)
        });
        let default_sampler = self
            .samplers
            .entry((true, SmolStr::new_static("EXTEND")))
            .or_insert_with(|| sampler(gpu, true, "EXTEND"))
            .clone();
        let group = |view: &wgpu::TextureView, smp: &wgpu::Sampler| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frap workbench image"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(smp),
                    },
                ],
            })
        };
        let mut out = vec![group(white, &default_sampler)];
        for t in slots.textures() {
            let view = self.images.get(&(t.image, t.is_data)).unwrap_or(white);
            let smp = self
                .samplers
                .get(&(t.linear, t.extension.clone()))
                .unwrap_or(&default_sampler);
            out.push(group(view, smp));
        }
        out
    }
}

/// The sampler of a TEXTURE draw: bilinear or nearest, no mipmaps; `REPEAT` and `MIRROR` wrap,
/// `EXTEND` clamps, `CLIP` clamps (the shader blanks the outside).
fn sampler(gpu: &Gpu, linear: bool, extension: &str) -> wgpu::Sampler {
    let filter = if linear {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    };
    let address = match extension {
        "REPEAT" => wgpu::AddressMode::Repeat,
        "MIRROR" => wgpu::AddressMode::MirrorRepeat,
        _ => wgpu::AddressMode::ClampToEdge,
    };
    gpu.device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap workbench image"),
        address_mode_u: address,
        address_mode_v: address,
        address_mode_w: address,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

/// 2 x 2 box filter (odd sizes clamp the last row/column).
fn halve(px: &[[f32; 4]], w: u32, h: u32) -> (Vec<[f32; 4]>, u32, u32) {
    let nw = (w / 2).max(1);
    let nh = (h / 2).max(1);
    let at = |x: u32, y: u32| px[(y.min(h - 1) * w + x.min(w - 1)) as usize];
    let mut out = Vec::with_capacity((nw * nh) as usize);
    for y in 0..nh {
        for x in 0..nw {
            let s = [
                at(2 * x, 2 * y),
                at(2 * x + 1, 2 * y),
                at(2 * x, 2 * y + 1),
                at(2 * x + 1, 2 * y + 1),
            ];
            out.push(std::array::from_fn(|c| {
                (s[0][c] + s[1][c] + s[2][c] + s[3][c]) * 0.25
            }));
        }
    }
    (out, nw, nh)
}

/// Uploads an image as linear half floats (one mip level).
fn upload(gpu: &Gpu, img: &DecodedImage, is_data: bool) -> wgpu::TextureView {
    let max = gpu.device.limits().max_texture_dimension_2d;
    let (mut w, mut h) = (img.width.max(1), img.height.max(1));
    let mut px = image_linear(img, is_data);
    if px.len() != (w * h) as usize {
        // Blender's missing-texture magenta.
        px = vec![[1.0, 0.0, 1.0, 1.0]];
        (w, h) = (1, 1);
    }
    while w > max || h > max {
        (px, w, h) = halve(&px, w, h);
    }
    let bytes: Vec<u8> = px
        .iter()
        .flat_map(|p| p.iter().flat_map(|c| half::f16::from_f32(*c).to_le_bytes()))
        .collect();
    let tex = gpu.device.create_texture_with_data(
        &gpu.queue,
        &wgpu::TextureDescriptor {
            label: Some("frap workbench image"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &bytes,
    );
    tex.create_view(&Default::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_and_bits() {
        let a = RWorkbenchTexture::new(ImageKey(1), true, "REPEAT", true);
        let b = RWorkbenchTexture::new(ImageKey(2), false, "CLIP", false);
        let mut s = Slots::default();
        assert_eq!((s.slot(&a), s.slot(&b), s.slot(&a)), (1, 2, 1));
        assert_eq!(s.textures().len(), 2);
        assert_eq!((cutoff(&a), cutoff(&b)), (0.1, NO_CUTOFF));
        assert_eq!((image_bits(&a), image_bits(&b)), (0, IMAGE_CLIP));
        assert_eq!(fit(&[1, 2, 3], 2, 0), [1, 2]);
        assert_eq!(fit(&[1], 3, 0), [1, 0, 0]);
        let (d, w, h) = halve(&[[1.0; 4], [0.0; 4]], 2, 1);
        assert_eq!((w, h, d[0]), (1, 1, [0.5; 4]));
    }
}
