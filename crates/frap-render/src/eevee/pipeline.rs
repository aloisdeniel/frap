// SPDX-License-Identifier: GPL-3.0-or-later
//! EEVEE passes and pipelines (WP 5.1): per TAA sample, the shadow maps (`shadows.rs`), the
//! world background, the forward uber-shader (opaque, alpha-clipped and DITHERED materials,
//! then the transmissive ones over a copy of the opaque colour, then BLENDED materials sorted
//! back to front per object), and the film accumulation (`film.rs`).
//!
//! **One uber-shader** (`shaders/eevee_forward.wgsl`) renders every material: material
//! features are uniform flags and values ([`MaterialParams`] → [`GpuMaterial`]), never
//! pipeline permutations (§19.2), so a cold render compiles four pipelines (forward, blended,
//! background, shadow) plus the film's, once per device.
//!
//! # The shading-feature contract (WP 5.5)
//!
//! A feature (`eevee/features/<name>.rs`) registers an [`EeveeFeature`](super::EeveeFeature):
//!
//! - flag bit: the `i`-th feature of [`super::features()`] owns bit `1 << i` of
//!   [`MaterialParams::flags`]; the pipeline sets it when `applies(material)` is true, then calls
//!   `pack(material, scene, &mut params)`. `pack` writes the [`MaterialParams`] fields its feature
//!   drives (the named fields below, [`MaterialParams::extra`], the texture slots in
//!   [`MaterialParams::tex`] through [`MaterialParams::add_texture`], the colour attribute of the
//!   vertex colours);
//! - WGSL: the feature's `wgsl` must define `fn feature_<name>(s: ptr<function, Surface>,
//!   v: ptr<function, Inputs>)`. The forward shader calls it (under the flag bit) after
//!   `surface_init` filled the [`Surface`] from the core material values and before
//!   transparency and lighting. It may read the global `material` (`Material`, the
//!   [`GpuMaterial`] layout), sample a slot with `tex_sample(slot, v)`, and set any `Surface`
//!   field: `base_color`, `alpha`, `metallic`, `roughness`, `ior`, `specular_ior_level`,
//!   `specular_tint`, `N` (shading normal), `emission`/`emission_strength`, `coat_*`
//!   (`coat_N` included), `sheen_*`, `transmission_weight`. `Inputs` carries the world
//!   position `P`, normals `N`/`Ng`, view vector `V`, `uv0`/`uv1` (the material's first two UV
//!   sources, see [`MaterialParams::uv_sources`]), `color` (the colour attribute of
//!   [`MaterialParams::color_attribute`], `(0, 0, 0, 1)` when missing, as Blender), `tangent`
//!   (world, sign in `w`), `frag` and `front`.
//!
//! The core lights every closure the Principled BSDF produces from the `Surface`
//! (`node_bsdf_principled`: sheen, coat, emission, metallic, transmission, specular, diffuse),
//! so a feature only provides inputs.
//!
//! [`Surface`]: ../../shaders/eevee_forward.wgsl

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use smol_str::SmolStr;

use frap_imbuf::{DecodedImage, PixelData};
use frap_material::{
    AlphaMode, Filter, LoweredMaterial, Sampler, TexRef, TexTransform, UvSource, Wrap,
};
use frap_render_scene::{
    Film, ImageKey, MatKey, MeshKey, RMesh, RObject, RWorld, RenderError, RenderScene,
};

use super::film::{self, FilmPipeline, FilmTargets};
use super::lights::{self, GpuLight, LightRecord};
use super::shadows::{self, Aabb};
use super::taa;
use crate::engine::{GpuCache, VERTEX_STRIDE};
use crate::gpu::Gpu;

/// Whether the EEVEE-like engine renders (`RenderService::supports_engine("BLENDER_EEVEE")`):
/// `render.engine = 'BLENDER_EEVEE'` (and `'CYCLES'`, D-12) render with it.
pub const IMPLEMENTED: bool = true;

/// The forward uber-shader (core; feature snippets and the dispatcher are appended).
pub const FORWARD_WGSL: &str = include_str!("../shaders/eevee_forward.wgsl");
/// The shadow-map shader.
pub const SHADOW_WGSL: &str = include_str!("../shaders/eevee_shadow.wgsl");
/// EEVEE's utility texture (`UtilityTexture`, `eevee_pipeline.hh`): 20 layers of 64 x 64
/// RGBA half floats — blue noise, the LTC disk integral (A of layer 1), the LTC GGX matrices,
/// the GGX BRDF split-sum and the 16 BSDF/BTDF layers — extracted from Blender 5.2.4's
/// `eevee_lut.cc` (GPL-2.0-or-later) and stored as Blender uploads them (`SFLOAT_16_16_16_16`).
pub const UTILITY_TX: &[u8] = include_bytes!("pipeline/utility_tx.bin");
/// Layers of [`UTILITY_TX`].
pub const UTILITY_LAYERS: u32 = 20;
/// Depth format of the camera pass.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Texture slots per material.
pub const MAX_TEXTURES: usize = 8;

/// Transparency of a material (`material.ints.y`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum BlendMode {
    /// Opaque (alpha ignored).
    #[default]
    Opaque = 0,
    /// Alpha clip at [`MaterialParams::alpha_cutoff`].
    Clip = 1,
    /// Hashed transparency (`surface_render_method = 'DITHERED'`, Blender's default): the TAA
    /// samples average to the blend.
    Dithered = 2,
    /// `surface_render_method = 'BLENDED'`: drawn after the opaque pass, sorted back to front per
    /// object, premultiplied blending.
    Blended = 3,
}

/// One texture slot of a material.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureSlot {
    /// The image.
    pub image: ImageKey,
    /// Filter and wrap.
    pub sampler: Sampler,
    /// UV map.
    pub uv: UvSource,
    /// `KHR_texture_transform` (glTF convention).
    pub transform: Option<TexTransform>,
    /// Non-Color data (no sRGB decoding).
    pub is_data: bool,
}

/// Indices into [`MaterialParams::tex`] of each texture channel.
pub mod tex {
    /// Base colour (RGBA).
    pub const BASE_COLOR: (usize, usize) = (0, 0);
    /// Metallic (B) and roughness (G).
    pub const METALLIC_ROUGHNESS: (usize, usize) = (0, 1);
    /// Tangent-space normal map.
    pub const NORMAL: (usize, usize) = (0, 2);
    /// Emission (RGB).
    pub const EMISSION: (usize, usize) = (0, 3);
    /// Coat weight (R).
    pub const COAT: (usize, usize) = (1, 0);
    /// Coat roughness (G).
    pub const COAT_ROUGHNESS: (usize, usize) = (1, 1);
    /// Coat normal map.
    pub const COAT_NORMAL: (usize, usize) = (1, 2);
    /// Sheen colour (RGB).
    pub const SHEEN_COLOR: (usize, usize) = (1, 3);
    /// Sheen roughness (A).
    pub const SHEEN_ROUGHNESS: (usize, usize) = (2, 0);
    /// Transmission (R).
    pub const TRANSMISSION: (usize, usize) = (2, 1);
    /// Specular IOR level (A).
    pub const SPECULAR: (usize, usize) = (2, 2);
    /// Specular tint (RGB).
    pub const SPECULAR_TINT: (usize, usize) = (2, 3);
}

/// Per-material parameters of the forward uber-shader. The core (WP 5.1) fills the Principled
/// inputs every material has; the shading features (WP 5.5) the rest. Defaults: Blender's
/// default surface (`BKE_material_default_surface`: Principled with base colour 0.8).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct MaterialParams {
    /// Feature flag bits (bit `i` = the `i`-th feature of [`super::features()`]).
    pub flags: u32,
    /// `Base Color` (linear RGBA; alpha = `Alpha`).
    pub base_color: [f32; 4],
    /// `Metallic`
    pub metallic: f32,
    /// `Roughness`
    pub roughness: f32,
    /// `IOR`
    pub ior: f32,
    /// `Specular IOR Level`
    pub specular_ior_level: f32,
    /// `Specular Tint`
    pub specular_tint: [f32; 3],
    /// Alpha-clip threshold ([`BlendMode::Clip`]).
    pub alpha_cutoff: f32,
    /// `Emission Color`
    pub emission: [f32; 3],
    /// `Emission Strength`
    pub emission_strength: f32,
    /// `Coat Weight`
    pub coat_weight: f32,
    /// `Coat Roughness`
    pub coat_roughness: f32,
    /// `Coat IOR`
    pub coat_ior: f32,
    /// `Coat Tint`
    pub coat_tint: [f32; 3],
    /// Coat normal-map strength.
    pub coat_normal_scale: f32,
    /// `Sheen Weight`
    pub sheen_weight: f32,
    /// `Sheen Roughness`
    pub sheen_roughness: f32,
    /// `Sheen Tint`
    pub sheen_tint: [f32; 3],
    /// `Transmission Weight`
    pub transmission_weight: f32,
    /// Normal-map strength.
    pub normal_scale: f32,
    /// glTF occlusion strength (EEVEE ignores occlusion; kept for features).
    pub occlusion_strength: f32,
    /// Values of the shading features (`material.extra` in WGSL).
    pub extra: [[f32; 4]; 4],
    /// Texture slot of each channel ([`tex`]; -1: none). Row 3 is feature-defined.
    pub tex: [[i32; 4]; 4],
    /// Transparency.
    pub blend: BlendMode,
    /// Vertex-colour mode for the vertex-colour feature (`material.ints.z`).
    pub vertex_color: u32,
    /// `!use_backface_culling`
    pub double_sided: bool,
    /// Unlit (emits the base colour).
    pub unlit: bool,
    /// Multiple-scattering GGX (`MULTI_GGX`, the Principled default).
    pub multiscatter: bool,
    /// The material's textures (at most [`MAX_TEXTURES`]).
    pub textures: Vec<TextureSlot>,
    /// Colour attribute bound as `Inputs.color` (`None`: the mesh's default colour attribute,
    /// else its first one; `(0, 0, 0, 1)` when the attribute is missing, as Blender reads a
    /// missing attribute).
    pub color_attribute: Option<SmolStr>,
    /// Refraction traces the opaque surfaces behind (screen space, `use_raytrace_refraction`
    /// with `scene.eevee.use_raytracing`; set by the transmission feature, WP 5.5); otherwise
    /// it samples the world probe, as Blender's default.
    pub screen_refraction: bool,
}

impl Default for MaterialParams {
    fn default() -> Self {
        MaterialParams {
            flags: 0,
            base_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ior: 1.5,
            specular_ior_level: 0.5,
            specular_tint: [1.0; 3],
            alpha_cutoff: 0.5,
            emission: [1.0; 3],
            emission_strength: 0.0,
            coat_weight: 0.0,
            coat_roughness: 0.03,
            coat_ior: 1.5,
            coat_tint: [1.0; 3],
            coat_normal_scale: 1.0,
            sheen_weight: 0.0,
            sheen_roughness: 0.5,
            sheen_tint: [1.0; 3],
            transmission_weight: 0.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            extra: [[0.0; 4]; 4],
            tex: [[-1; 4]; 4],
            blend: BlendMode::Opaque,
            vertex_color: 0,
            double_sided: true,
            unlit: false,
            multiscatter: true,
            textures: Vec::new(),
            color_attribute: None,
            screen_refraction: false,
        }
    }
}

impl MaterialParams {
    /// Adds a texture slot for `t` (an existing identical slot is reused) and returns its index,
    /// or -1 when the image is not in the scene or every slot is taken.
    pub fn add_texture(&mut self, t: &TexRef, scene: &RenderScene) -> i32 {
        let Some(&image) = scene.image_ids.get(&t.image.key()) else {
            return -1;
        };
        let slot = TextureSlot {
            image,
            sampler: t.sampler,
            uv: t.uv.clone(),
            transform: t.transform,
            is_data: t.colorspace_is_data,
        };
        if let Some(i) = self.textures.iter().position(|s| *s == slot) {
            return i as i32;
        }
        if self.textures.len() >= MAX_TEXTURES {
            return -1;
        }
        self.textures.push(slot);
        (self.textures.len() - 1) as i32
    }

    /// Sets channel `ch` ([`tex`]) to the slot of `t`.
    pub fn set_texture(&mut self, ch: (usize, usize), t: &TexRef, scene: &RenderScene) {
        self.tex[ch.0][ch.1] = self.add_texture(t, scene);
    }

    /// The core values of `m` (WP 5.1): base colour, metallic, roughness, IOR, specular level
    /// and tint (`KHR_materials_specular`), their textures, transparency, culling, unlit.
    pub fn core(m: &LoweredMaterial, scene: &RenderScene) -> MaterialParams {
        let mut p = MaterialParams {
            base_color: m.base_color.factor,
            metallic: m.metallic.factor,
            roughness: m.roughness.factor,
            ior: m.ext.ior.unwrap_or(1.5),
            emission: m.emissive.factor,
            emission_strength: 0.0,
            occlusion_strength: m.occlusion_strength,
            double_sided: m.double_sided,
            unlit: m.unlit,
            ..MaterialParams::default()
        };
        if let Some(t) = &m.base_color.texture {
            p.set_texture(tex::BASE_COLOR, t, scene);
        }
        // glTF ORM packing: metallic in B, roughness in G of the same image; separate images
        // each feed their channel through the same lookup.
        if let Some(t) = m.metallic.texture.as_ref().or(m.roughness.texture.as_ref()) {
            p.set_texture(tex::METALLIC_ROUGHNESS, t, scene);
        }
        if let Some(s) = &m.ext.specular {
            // KHR_materials_specular: `specularFactor` = `Specular IOR Level` × 2.
            p.specular_ior_level = s.factor.factor / 2.0;
            p.specular_tint = s.color.factor;
            if let Some(t) = &s.factor.texture {
                p.set_texture(tex::SPECULAR, t, scene);
            }
            if let Some(t) = &s.color.texture {
                p.set_texture(tex::SPECULAR_TINT, t, scene);
            }
        }
        p.blend = match m.alpha_mode {
            AlphaMode::Opaque => BlendMode::Opaque,
            AlphaMode::Mask { cutoff } => {
                p.alpha_cutoff = cutoff;
                BlendMode::Clip
            }
            // `surface_render_method` defaults to DITHERED in Blender 5.2.
            AlphaMode::Blend => BlendMode::Dithered,
        };
        p
    }

    /// The parameters of `m`: [`MaterialParams::core`], then every registered feature that
    /// applies (its flag bit set, its `pack` called).
    pub fn for_material(m: &LoweredMaterial, scene: &RenderScene) -> MaterialParams {
        let mut p = MaterialParams::core(m, scene);
        for (bit, f) in super::features().iter().enumerate().take(32) {
            if (f.applies)(m) {
                p.flags |= 1 << bit;
                (f.pack)(m, scene, &mut p);
            }
        }
        p
    }

    /// The distinct UV sources of the textures, in slot order: the first is `Inputs.uv0`, the
    /// second `Inputs.uv1` (later ones share `uv0`).
    pub fn uv_sources(&self) -> Vec<UvSource> {
        let mut v: Vec<UvSource> = vec![];
        for s in &self.textures {
            if !v.contains(&s.uv) {
                v.push(s.uv.clone());
            }
        }
        v
    }

    /// The GPU record.
    pub fn gpu(&self) -> GpuMaterial {
        let uvs = self.uv_sources();
        let mut slots = [GpuTexSlot::zeroed(); MAX_TEXTURES];
        for (i, s) in self.textures.iter().enumerate().take(MAX_TEXTURES) {
            let ch = uvs.iter().position(|u| *u == s.uv).unwrap_or(0).min(1) as f32;
            let [r0, r1] = uv_matrix(s.transform);
            slots[i] = GpuTexSlot {
                m0: [r0[0], r0[1], r0[2], ch],
                m1: [r1[0], r1[1], r1[2], 1.0],
            };
        }
        let mut bits = 0u32;
        if self.double_sided {
            bits |= 1;
        }
        if self.unlit {
            bits |= 2;
        }
        if self.multiscatter {
            bits |= 4;
        }
        if self.screen_refraction {
            bits |= 8;
        }
        let rgb = |c: [f32; 3], w: f32| [c[0], c[1], c[2], w];
        GpuMaterial {
            base_color: self.base_color,
            pbr: [
                self.metallic,
                self.roughness,
                self.ior,
                self.specular_ior_level,
            ],
            specular_tint: rgb(self.specular_tint, self.alpha_cutoff),
            emission: rgb(self.emission, self.emission_strength),
            coat: [
                self.coat_weight,
                self.coat_roughness,
                self.coat_ior,
                self.coat_normal_scale,
            ],
            coat_tint: rgb(self.coat_tint, self.normal_scale),
            sheen: rgb(self.sheen_tint, self.sheen_roughness),
            misc: [
                self.transmission_weight,
                self.sheen_weight,
                self.occlusion_strength,
                0.0,
            ],
            extra: self.extra,
            tex: self.tex,
            ints: [self.flags, self.blend as u32, self.vertex_color, bits],
            slots,
        }
    }

    /// Whether the material needs the opaque colour behind it (screen-space refraction).
    pub fn refracts(&self) -> bool {
        self.screen_refraction
            && (self.transmission_weight > 0.0
                || self.tex[tex::TRANSMISSION.0][tex::TRANSMISSION.1] >= 0)
    }
}

/// The 2 x 3 glTF texture transform (rows): `uv' = T · R · S · uv`, with the rotation as
/// Blender's glTF add-on converts Mapping nodes (`x' = cos·x + sin·y`, `y' = −sin·x + cos·y`).
pub fn uv_matrix(t: Option<TexTransform>) -> [[f32; 3]; 2] {
    let Some(t) = t else {
        return [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    };
    let (s, c) = t.rotation.sin_cos();
    [
        [c * t.scale[0], s * t.scale[1], t.offset[0]],
        [-s * t.scale[0], c * t.scale[1], t.offset[1]],
    ]
}

/// One texture slot as the shader reads it (`TexSlot`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuTexSlot {
    /// Row 0 of the transform, `w` UV channel.
    pub m0: [f32; 4],
    /// Row 1 of the transform, `w` 1 when bound.
    pub m1: [f32; 4],
}

/// [`MaterialParams`] as the forward shader reads it (`Material` in `eevee_forward.wgsl`).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable, PartialEq)]
pub struct GpuMaterial {
    /// `base_color`
    pub base_color: [f32; 4],
    /// metallic, roughness, ior, specular_ior_level
    pub pbr: [f32; 4],
    /// specular tint, alpha cutoff
    pub specular_tint: [f32; 4],
    /// emission colour, strength
    pub emission: [f32; 4],
    /// coat weight, roughness, ior, normal scale
    pub coat: [f32; 4],
    /// coat tint, normal scale
    pub coat_tint: [f32; 4],
    /// sheen tint, sheen roughness
    pub sheen: [f32; 4],
    /// transmission weight, sheen weight, occlusion strength, 0
    pub misc: [f32; 4],
    /// feature values
    pub extra: [[f32; 4]; 4],
    /// texture slot per channel
    pub tex: [[i32; 4]; 4],
    /// flags, blend mode, vertex-colour mode, bits
    pub ints: [u32; 4],
    /// texture slots
    pub slots: [GpuTexSlot; MAX_TEXTURES],
}

/// Per-sample uniforms (`Frame`).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FrameU {
    view: [f32; 16],
    proj: [f32; 16],
    view_proj: [f32; 16],
    inv_view_proj: [f32; 16],
    camera_pos: [f32; 4],
    camera_fwd: [f32; 4],
    world_color: [f32; 4],
    resolution: [f32; 4],
    counts: [u32; 4],
    clamp: [f32; 4],
    world_params: [f32; 4],
    shadow: [f32; 4],
    sh: [[f32; 4]; 9],
}

/// Per-shadow-map uniforms of the shadow pass (`Layer` in `eevee_shadow.wgsl`).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ShadowLayerU {
    view_proj: [f32; 16],
    /// x: the sample's transparency random (transparent shadows, WP 5.5), y: the map index
    /// (the transparent-shadow hash key).
    params: [f32; 4],
}

/// Per-draw uniforms (`Draw`).
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawU {
    model: [f32; 16],
    normal_matrix: [f32; 16],
    flags: [u32; 4],
}

const FRAME_FILM_TRANSPARENT: u32 = 1;
const FRAME_WORLD_HDRI: u32 = 2;
const FRAME_PERSPECTIVE: u32 = 4;
const FRAME_REFRACTION: u32 = 8;

/// The forward shader with the registered features and their dispatcher.
pub fn forward_source() -> String {
    let feats = super::features();
    let mut s = String::from(FORWARD_WGSL);
    for f in &feats {
        s.push_str("\n// ---- feature ");
        s.push_str(f.name);
        s.push('\n');
        s.push_str(f.wgsl);
        s.push('\n');
    }
    s.push_str(
        "\nfn eevee_features_apply(s: ptr<function, Surface>, v: ptr<function, Inputs>) {\n",
    );
    for (bit, f) in feats.iter().enumerate().take(32) {
        s.push_str(&format!(
            "    if ((material.ints.x & {}u) != 0u) {{ feature_{}(s, v); }}\n",
            1u32 << bit,
            f.name
        ));
    }
    s.push_str("}\n");
    s
}

/// Mesh attributes beyond position and normal (the [`GpuCache`] keeps those), per mesh.
struct MeshAttrs {
    uvs: Vec<wgpu::Buffer>,
    colors: Vec<wgpu::Buffer>,
    tangents: Option<wgpu::Buffer>,
    zero_uv: wgpu::Buffer,
    /// `(0, 0, 0, 1)`: a missing colour attribute (Blender 5.2.4 renders a Color Attribute
    /// node naming a missing attribute black with alpha 1).
    missing_color: wgpu::Buffer,
    zero_tangent: wgpu::Buffer,
    color_names: Vec<SmolStr>,
    color_default: Option<usize>,
    uv_names: Vec<SmolStr>,
    uv_active_render: Option<usize>,
    bounds: Aabb,
}

struct GpuImage {
    view: wgpu::TextureView,
}

/// GPU objects created once per device.
struct Resources {
    forward: wgpu::RenderPipeline,
    blended: wgpu::RenderPipeline,
    background: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    film: FilmPipeline,
    frame_bgl: wgpu::BindGroupLayout,
    draw_bgl: wgpu::BindGroupLayout,
    material_bgl: wgpu::BindGroupLayout,
    layer_bgl: wgpu::BindGroupLayout,
    util_view: wgpu::TextureView,
    linear: wgpu::Sampler,
    shadow_sampler: wgpu::Sampler,
    samplers: HashMap<Sampler, wgpu::Sampler>,
    white: wgpu::TextureView,
    black_cube: wgpu::TextureView,
}

/// Engine state kept between renders (pipelines, the utility texture, samplers, mesh
/// attributes and textures).
#[derive(Default)]
#[non_exhaustive]
pub struct State {
    res: Option<Resources>,
    meshes: HashMap<MeshKey, MeshAttrs>,
    images: HashMap<(ImageKey, bool), GpuImage>,
    shadow_atlas: Option<(u32, u32, wgpu::Texture)>,
    pipeline_builds: u32,
}

impl State {
    /// Number of times the pipelines were built (once per device).
    pub fn pipeline_builds(&self) -> u32 {
        self.pipeline_builds
    }
}

fn layout_entry(
    binding: u32,
    vis: wgpu::ShaderStages,
    ty: wgpu::BindingType,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: vis,
        ty,
        count: None,
    }
}

fn tex_ty(
    sample_type: wgpu::TextureSampleType,
    dim: wgpu::TextureViewDimension,
) -> wgpu::BindingType {
    wgpu::BindingType::Texture {
        sample_type,
        view_dimension: dim,
        multisampled: false,
    }
}

fn uniform_ty(dynamic: bool, size: usize) -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: dynamic,
        min_binding_size: wgpu::BufferSize::new(size as u64),
    }
}

fn storage_ty() -> wgpu::BindingType {
    wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    }
}

fn texture_2d(
    gpu: &Gpu,
    label: &str,
    size: [u32; 2],
    layers: u32,
    mips: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

fn write_layer(
    gpu: &Gpu,
    tex: &wgpu::Texture,
    mip: u32,
    layer: u32,
    size: [u32; 2],
    bytes: &[u8],
    texel: u32,
) {
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: mip,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size[0] * texel),
            rows_per_image: Some(size[1]),
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
}

const fn attr(loc: u32, format: wgpu::VertexFormat, offset: u64) -> wgpu::VertexAttribute {
    wgpu::VertexAttribute {
        format,
        offset,
        shader_location: loc,
    }
}
const ATTR_POS_NOR: [wgpu::VertexAttribute; 2] = [
    attr(0, wgpu::VertexFormat::Float32x3, 0),
    attr(1, wgpu::VertexFormat::Float32x3, 12),
];
const ATTR_POS: [wgpu::VertexAttribute; 1] = [attr(0, wgpu::VertexFormat::Float32x3, 0)];
const ATTR_UV0: [wgpu::VertexAttribute; 1] = [attr(2, wgpu::VertexFormat::Float32x2, 0)];
const ATTR_UV1: [wgpu::VertexAttribute; 1] = [attr(3, wgpu::VertexFormat::Float32x2, 0)];
const ATTR_COLOR: [wgpu::VertexAttribute; 1] = [attr(4, wgpu::VertexFormat::Float32x4, 0)];
const ATTR_TANGENT: [wgpu::VertexAttribute; 1] = [attr(5, wgpu::VertexFormat::Float32x4, 0)];

fn create_resources(gpu: &Gpu) -> Resources {
    let dev = &gpu.device;
    let frag = wgpu::ShaderStages::FRAGMENT;
    let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
    let float = wgpu::TextureSampleType::Float { filterable: true };
    let d2 = wgpu::TextureViewDimension::D2;
    let filtering = wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering);
    let frame_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap eevee frame"),
        entries: &[
            layout_entry(0, vf, uniform_ty(false, std::mem::size_of::<FrameU>())),
            layout_entry(1, frag, storage_ty()),
            layout_entry(2, frag, storage_ty()),
            layout_entry(3, frag, tex_ty(float, wgpu::TextureViewDimension::D2Array)),
            layout_entry(4, frag, filtering),
            layout_entry(
                5,
                frag,
                tex_ty(
                    wgpu::TextureSampleType::Depth,
                    wgpu::TextureViewDimension::D2Array,
                ),
            ),
            layout_entry(
                6,
                frag,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
            ),
            layout_entry(7, frag, tex_ty(float, wgpu::TextureViewDimension::Cube)),
            layout_entry(8, frag, tex_ty(float, d2)),
            layout_entry(9, frag, tex_ty(float, d2)),
        ],
    });
    let draw_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap eevee draw"),
        entries: &[layout_entry(
            0,
            vf,
            uniform_ty(true, std::mem::size_of::<DrawU>()),
        )],
    });
    let mut mat_entries = vec![layout_entry(
        0,
        frag,
        uniform_ty(false, std::mem::size_of::<GpuMaterial>()),
    )];
    for i in 0..MAX_TEXTURES as u32 {
        mat_entries.push(layout_entry(1 + i, frag, tex_ty(float, d2)));
    }
    for i in 0..MAX_TEXTURES as u32 {
        mat_entries.push(layout_entry(1 + MAX_TEXTURES as u32 + i, frag, filtering));
    }
    let material_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap eevee material"),
        entries: &mat_entries,
    });
    let layer_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap eevee shadow layer"),
        entries: &[layout_entry(
            0,
            vf,
            uniform_ty(true, std::mem::size_of::<ShadowLayerU>()),
        )],
    });

    let forward_module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frap eevee forward"),
        source: wgpu::ShaderSource::Wgsl(forward_source().into()),
    });
    let shadow_module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frap eevee shadow"),
        source: wgpu::ShaderSource::Wgsl(SHADOW_WGSL.into()),
    });
    let forward_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap eevee forward"),
        bind_group_layouts: &[Some(&frame_bgl), Some(&draw_bgl), Some(&material_bgl)],
        immediate_size: 0,
    });
    let bg_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap eevee background"),
        bind_group_layouts: &[Some(&frame_bgl)],
        immediate_size: 0,
    });
    let shadow_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap eevee shadow"),
        bind_group_layouts: &[Some(&layer_bgl), Some(&draw_bgl)],
        immediate_size: 0,
    });

    let vb = |stride: u64, attributes: &'static [wgpu::VertexAttribute]| {
        Some(wgpu::VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes,
        })
    };
    let forward_buffers = [
        vb(VERTEX_STRIDE, &ATTR_POS_NOR),
        vb(8, &ATTR_UV0),
        vb(8, &ATTR_UV1),
        vb(16, &ATTR_COLOR),
        vb(16, &ATTR_TANGENT),
    ];
    let shadow_buffers = [vb(VERTEX_STRIDE, &ATTR_POS)];

    let premultiplied = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    };
    let color_target = |blend: Option<wgpu::BlendState>| {
        [Some(wgpu::ColorTargetState {
            format: film::SAMPLE_FORMAT,
            blend,
            write_mask: wgpu::ColorWrites::ALL,
        })]
    };
    let depth = |write: bool, compare: wgpu::CompareFunction| wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    };
    let tris = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: None,
        ..Default::default()
    };
    let forward_pipeline = |label: &str, blend: Option<wgpu::BlendState>, write: bool| {
        let targets = color_target(blend);
        dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&forward_layout),
            vertex: wgpu::VertexState {
                module: &forward_module,
                entry_point: Some("vs_forward"),
                compilation_options: Default::default(),
                buffers: &forward_buffers,
            },
            primitive: tris,
            depth_stencil: Some(depth(write, wgpu::CompareFunction::LessEqual)),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &forward_module,
                entry_point: Some("fs_forward"),
                compilation_options: Default::default(),
                targets: &targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let forward = forward_pipeline("frap eevee forward", None, true);
    let blended = forward_pipeline("frap eevee blended", Some(premultiplied), false);
    let bg_targets = color_target(None);
    let background = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("frap eevee background"),
        layout: Some(&bg_layout),
        vertex: wgpu::VertexState {
            module: &forward_module,
            entry_point: Some("vs_background"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(depth(false, wgpu::CompareFunction::Always)),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &forward_module,
            entry_point: Some("fs_background"),
            compilation_options: Default::default(),
            targets: &bg_targets,
        }),
        multiview_mask: None,
        cache: None,
    });
    let shadow = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("frap eevee shadow"),
        layout: Some(&shadow_layout),
        vertex: wgpu::VertexState {
            module: &shadow_module,
            entry_point: Some("vs_shadow"),
            compilation_options: Default::default(),
            buffers: &shadow_buffers,
        },
        primitive: tris,
        depth_stencil: Some(wgpu::DepthStencilState {
            format: shadows::FORMAT,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Greater),
            stencil: wgpu::StencilState::default(),
            // The slope bias is applied by `fs_shadow`, which writes the depth
            // (eevee_shadow.wgsl).
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        // Hashed transparent shadows (WP 5.5): discards by the caster's alpha.
        fragment: Some(wgpu::FragmentState {
            module: &shadow_module,
            entry_point: Some("fs_shadow"),
            compilation_options: Default::default(),
            targets: &[],
        }),
        multiview_mask: None,
        cache: None,
    });

    // Utility texture.
    let util = texture_2d(
        gpu,
        "frap eevee utility",
        [64, 64],
        UTILITY_LAYERS,
        1,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let layer_bytes = 64 * 64 * 8;
    for l in 0..UTILITY_LAYERS {
        let b = &UTILITY_TX[(l as usize) * layer_bytes..(l as usize + 1) * layer_bytes];
        write_layer(gpu, &util, 0, l, [64, 64], b, 8);
    }
    let util_view = util.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let linear = dev.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap eevee linear"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    let shadow_sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap eevee shadow"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        compare: Some(wgpu::CompareFunction::GreaterEqual),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });
    let white_tex = texture_2d(
        gpu,
        "frap eevee white",
        [1, 1],
        1,
        1,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let one = half::f16::from_f32(1.0).to_le_bytes();
    write_layer(gpu, &white_tex, 0, 0, [1, 1], &one.repeat(4), 8);
    let cube = texture_2d(
        gpu,
        "frap eevee black cube",
        [1, 1],
        6,
        1,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    for l in 0..6 {
        write_layer(gpu, &cube, 0, l, [1, 1], &[0u8; 8], 8);
    }
    Resources {
        forward,
        blended,
        background,
        shadow,
        film: FilmPipeline::new(gpu),
        frame_bgl,
        draw_bgl,
        material_bgl,
        layer_bgl,
        util_view,
        linear,
        shadow_sampler,
        samplers: HashMap::new(),
        white: white_tex.create_view(&Default::default()),
        black_cube: cube.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        }),
    }
}

fn address(w: Wrap) -> wgpu::AddressMode {
    match w {
        Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
        Wrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        _ => wgpu::AddressMode::Repeat,
    }
}

fn sampler<'a>(
    gpu: &Gpu,
    cache: &'a mut HashMap<Sampler, wgpu::Sampler>,
    s: Sampler,
) -> &'a wgpu::Sampler {
    cache.entry(s).or_insert_with(|| {
        let nearest = s.filter == Filter::Closest;
        let f = if nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("frap eevee material"),
            address_mode_u: address(s.wrap_s),
            address_mode_v: address(s.wrap_t),
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: f,
            min_filter: f,
            mipmap_filter: if nearest {
                wgpu::MipmapFilterMode::Nearest
            } else {
                wgpu::MipmapFilterMode::Linear
            },
            ..Default::default()
        })
    })
}

/// Linear RGBA of a decoded image (sRGB-encoded 8/16-bit images decoded unless `is_data`;
/// float images are scene-linear; grey and missing alpha expanded).
pub fn image_linear(img: &DecodedImage, is_data: bool) -> Vec<[f32; 4]> {
    let n = img.width as usize * img.height as usize;
    let ch = img.channels.clamp(1, 4) as usize;
    let decode = |v: f32| {
        if is_data {
            v
        } else {
            frap_color::spaces::srgb_to_linear(v)
        }
    };
    let get = |i: usize| -> f32 {
        match &img.pixels {
            PixelData::U8(p) => p.get(i).map_or(0.0, |&v| f32::from(v) / 255.0),
            PixelData::U16(p) => p.get(i).map_or(0.0, |&v| f32::from(v) / 65535.0),
            PixelData::F32(p) => p.get(i).copied().unwrap_or(0.0),
            #[allow(unreachable_patterns)]
            _ => 0.0,
        }
    };
    let float = matches!(img.pixels, PixelData::F32(_));
    let color = |v: f32| if float { v } else { decode(v) };
    (0..n)
        .map(|p| {
            let b = p * ch;
            match ch {
                1 => {
                    let g = color(get(b));
                    [g, g, g, 1.0]
                }
                2 => {
                    let g = color(get(b));
                    [g, g, g, get(b + 1)]
                }
                3 => [color(get(b)), color(get(b + 1)), color(get(b + 2)), 1.0],
                _ => [
                    color(get(b)),
                    color(get(b + 1)),
                    color(get(b + 2)),
                    get(b + 3),
                ],
            }
        })
        .collect()
}

fn upload_image(gpu: &Gpu, img: &DecodedImage, is_data: bool) -> GpuImage {
    let max = gpu.device.limits().max_texture_dimension_2d;
    let mut w = img.width.max(1);
    let mut h = img.height.max(1);
    let mut px = image_linear(img, is_data);
    if px.len() != (w * h) as usize {
        px = vec![[1.0, 0.0, 1.0, 1.0]];
        w = 1;
        h = 1;
    }
    // Images past the device limit are box-filtered down first.
    while w > max || h > max {
        (px, w, h) = downsample(&px, w, h);
    }
    let mips = 32 - w.max(h).leading_zeros();
    let tex = texture_2d(
        gpu,
        "frap eevee image",
        [w, h],
        1,
        mips,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let (mut lw, mut lh, mut level) = (w, h, px);
    for mip in 0..mips {
        let bytes: Vec<u8> = level
            .iter()
            .flat_map(|p| p.iter().flat_map(|c| half::f16::from_f32(*c).to_le_bytes()))
            .collect();
        write_layer(gpu, &tex, mip, 0, [lw, lh], &bytes, 8);
        if mip + 1 < mips {
            (level, lw, lh) = downsample(&level, lw, lh);
        }
    }
    GpuImage {
        view: tex.create_view(&Default::default()),
    }
}

/// 2 x 2 box filter (odd sizes clamp the last row/column).
fn downsample(px: &[[f32; 4]], w: u32, h: u32) -> (Vec<[f32; 4]>, u32, u32) {
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

fn vertex_buffer<T: Pod>(gpu: &Gpu, label: &str, data: &[T]) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    gpu.device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(data),
            usage: wgpu::BufferUsages::VERTEX,
        })
}

fn mesh_attrs(gpu: &Gpu, m: &RMesh) -> MeshAttrs {
    let n = m.positions.len().max(1);
    let fit2 = |v: &Vec<[f32; 2]>| {
        let mut v = v.clone();
        v.resize(n, [0.0; 2]);
        v
    };
    let fit4 = |v: &Vec<[f32; 4]>, fill: [f32; 4]| {
        let mut v = v.clone();
        v.resize(n, fill);
        v
    };
    let mut bounds = Aabb::EMPTY;
    for p in &m.positions {
        bounds.add(Vec3::from(*p));
    }
    MeshAttrs {
        uvs: m
            .uvs
            .iter()
            .map(|u| vertex_buffer(gpu, "frap eevee uv", &fit2(u)))
            .collect(),
        colors: m
            .colors
            .iter()
            .map(|c| vertex_buffer(gpu, "frap eevee colour", &fit4(c, [1.0; 4])))
            .collect(),
        tangents: m
            .tangents
            .as_ref()
            .map(|t| vertex_buffer(gpu, "frap eevee tangent", &fit4(t, [1.0, 0.0, 0.0, 1.0]))),
        zero_uv: vertex_buffer(gpu, "frap eevee zero uv", &vec![[0.0f32; 2]; n]),
        missing_color: vertex_buffer(
            gpu,
            "frap eevee missing colour",
            &vec![[0.0f32, 0.0, 0.0, 1.0]; n],
        ),
        zero_tangent: vertex_buffer(gpu, "frap eevee tangent", &vec![[1.0f32, 0.0, 0.0, 1.0]; n]),
        color_names: m.color_names.clone(),
        color_default: m.color_default,
        uv_names: m.uv_names.clone(),
        uv_active_render: m.uv_active_render,
        bounds,
    }
}

impl MeshAttrs {
    fn uv(&self, src: Option<&UvSource>) -> &wgpu::Buffer {
        let idx = match src {
            None | Some(UvSource::ActiveRender) => self.uv_active_render.or(Some(0)),
            Some(UvSource::Named(n)) => self.uv_names.iter().position(|x| x == n),
            Some(UvSource::Index(i)) => Some(*i as usize),
            #[allow(unreachable_patterns)]
            Some(_) => None,
        };
        idx.and_then(|i| self.uvs.get(i)).unwrap_or(&self.zero_uv)
    }
    fn color(&self, name: Option<&SmolStr>) -> &wgpu::Buffer {
        let idx = match name {
            Some(n) => self.color_names.iter().position(|x| x == n),
            // A Color Attribute node without a name reads the default colour attribute.
            None => self.color_default.or(Some(0)),
        };
        idx.and_then(|i| self.colors.get(i))
            .unwrap_or(&self.missing_color)
    }
}

/// One draw: an object's material slot range.
struct DrawCmd {
    mesh: MeshKey,
    range: std::ops::Range<u32>,
    draw: usize,
    material: usize,
    /// View-space depth of the object origin (BLENDED sorting).
    depth: f32,
}

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

fn strided<T: Pod>(items: &[T], stride: u64) -> Vec<u8> {
    let mut out = vec![0u8; items.len().max(1) * stride as usize];
    for (i, it) in items.iter().enumerate() {
        let b = bytemuck::bytes_of(it);
        out[i * stride as usize..i * stride as usize + b.len()].copy_from_slice(b);
    }
    out
}

/// The world as the forward shader lights it: constant radiance, or the HDRI probe.
enum World {
    Color([f32; 3]),
    Hdri(Box<super::ibl::WorldLighting>),
}

fn world(gpu: &Gpu, scene: &RenderScene) -> Result<World, RenderError> {
    match &scene.world {
        RWorld::Color { rgb, strength } => Ok(World::Color(rgb.map(|c| c * strength))),
        RWorld::Hdri { .. } => match super::ibl::world_lighting(gpu, scene) {
            Ok(Some(w)) => Ok(World::Hdri(Box::new(w))),
            // Until the HDRI probe exists (WP 5.2), the factory world colour lights the scene.
            Ok(None) | Err(RenderError::Unsupported(_)) => match RWorld::default() {
                RWorld::Color { rgb, strength } => Ok(World::Color(rgb.map(|c| c * strength))),
                _ => Ok(World::Color([0.0; 3])),
            },
            Err(e) => Err(e),
        },
        #[allow(unreachable_patterns)]
        _ => Ok(World::Color([0.0; 3])),
    }
}

/// Renders `scene` with the EEVEE-like engine.
pub fn render(
    state: &mut State,
    gpu: &Gpu,
    cache: &mut GpuCache,
    scene: &RenderScene,
) -> Result<Film, RenderError> {
    let [width, height] = scene.resolution;
    let limits = gpu.device.limits();
    let max = limits.max_texture_dimension_2d;
    if width == 0 || height == 0 || width > max || height > max {
        return Err(RenderError::Unsupported(format!(
            "resolution {width}x{height} (this adapter renders 1..={max} pixels per side)"
        )));
    }
    cache.begin_render();
    if state.res.is_none() {
        state.res = Some(create_resources(gpu));
        state.pipeline_builds += 1;
    }
    let world = world(gpu, scene)?;
    let dev = &gpu.device;

    // Meshes and their attributes; drop those this scene does not use.
    let used: std::collections::HashSet<MeshKey> =
        scene.objects.iter().filter_map(|o| o.mesh).collect();
    state.meshes.retain(|k, _| used.contains(k));
    for key in &used {
        if let Some(m) = scene.meshes.get(key)
            && !state.meshes.contains_key(key)
        {
            state.meshes.insert(*key, mesh_attrs(gpu, m));
        }
    }

    // Materials: one parameter record per material key, plus the default surface.
    let mut mat_index: HashMap<Option<MatKey>, usize> = HashMap::new();
    let mut mats: Vec<MaterialParams> = vec![];
    let material_of = |key: Option<MatKey>,
                       mats: &mut Vec<MaterialParams>,
                       index: &mut HashMap<Option<MatKey>, usize>|
     -> usize {
        *index.entry(key).or_insert_with(|| {
            let p = key
                .and_then(|k| scene.materials.get(&k))
                .map(|m| MaterialParams::for_material(m, scene))
                .unwrap_or_default();
            mats.push(p);
            mats.len() - 1
        })
    };

    // Draw lists.
    let view = scene.camera.view;
    let mut draws: Vec<DrawU> = vec![];
    let mut opaque: Vec<DrawCmd> = vec![];
    let mut refractive: Vec<DrawCmd> = vec![];
    let mut blended: Vec<DrawCmd> = vec![];
    // Shadow casters: `(mesh, draw record, index range)`; a transparent material slot casts
    // through its own draw record carrying its alpha (`transparent_shadow_draw`).
    let mut casters: Vec<(MeshKey, usize, std::ops::Range<u32>)> = vec![];
    let mut transparent_casters = false;
    let mut bounds = Aabb::EMPTY;
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
        let m3 = Mat3::from_mat4(obj.matrix_world);
        let det = m3.determinant();
        if !det.is_finite() || det == 0.0 {
            continue;
        }
        let attrs = &state.meshes[&key];
        if !attrs.bounds.is_empty() {
            let lb = attrs.bounds;
            for c in lb.corners() {
                bounds.add(obj.matrix_world.transform_point3(c));
            }
        }
        let normal_matrix = Mat4::from_mat3(m3.inverse().transpose());
        draws.push(DrawU {
            model: obj.matrix_world.to_cols_array(),
            normal_matrix: normal_matrix.to_cols_array(),
            flags: [u32::from(det < 0.0), draws.len() as u32, 0, 0],
        });
        let draw = draws.len() - 1;
        let depth = view.transform_point3(obj.matrix_world.w_axis.truncate()).z;
        for (slot, range) in submeshes(rmesh, count) {
            let mkey = material_key(obj, slot);
            let material = material_of(mkey, &mut mats, &mut mat_index);
            let cmd = DrawCmd {
                mesh: key,
                range,
                draw,
                material,
                depth,
            };
            let p = &mats[material];
            // `use_transparent_shadow` (Blender's default): a transparent material casts a
            // hashed shadow of its alpha (WP 5.5).
            match transparent_shadow_alpha(p) {
                None => casters.push((key, draw, cmd.range.clone())),
                Some(a) if a > 0.0 => {
                    let mut d = draws[draw];
                    d.flags[2] = a.to_bits();
                    d.flags[3] = 1;
                    draws.push(d);
                    casters.push((key, draws.len() - 1, cmd.range.clone()));
                    transparent_casters = true;
                }
                Some(_) => {}
            }
            if p.blend == BlendMode::Blended {
                blended.push(cmd);
            } else if p.refracts() {
                refractive.push(cmd);
            } else {
                opaque.push(cmd);
            }
        }
    }
    // Back to front (view space looks down -Z: the most negative depth first).
    blended.sort_by(|a, b| a.depth.total_cmp(&b.depth));

    // Lights.
    // The world sun extracted from the HDRI (WP 5.2) lights the scene like a sun lamp.
    let world_sun = match &world {
        World::Hdri(w) => w.sun.as_ref(),
        World::Color(_) => None,
    };
    let records: Vec<LightRecord> = scene
        .lights
        .iter()
        .chain(world_sun)
        .map(|l| lights::sync(l, scene.settings.eevee.light_threshold))
        .filter(LightRecord::is_visible)
        .collect();
    let plan = shadows::plan(
        &records,
        scene.settings.use_shadows,
        shadows::map_size(scene.settings.eevee.shadow_resolution_scale, max),
        limits.max_texture_array_layers,
        max,
    );
    let shadow_size = plan.size;
    // Jittered lights and hashed transparent casters re-render their maps every sample
    // (`has_jittered_transparency` with `use_jitter`, always on for image renders,
    // eevee_shadow.cc).
    let rerender_shadows = shadows::jittered(&plan, &records) || transparent_casters;

    // Textures and samplers of the materials.
    {
        let res = state.res.as_mut().expect("invariant: created above");
        for p in &mats {
            for t in &p.textures {
                sampler(gpu, &mut res.samplers, t.sampler);
            }
        }
    }
    let mut used_images = std::collections::HashSet::new();
    for p in &mats {
        for t in &p.textures {
            let k = (t.image, t.is_data);
            used_images.insert(k);
            if !state.images.contains_key(&k)
                && let Some(img) = scene.images.get(&t.image)
            {
                state.images.insert(k, upload_image(gpu, img, t.is_data));
            }
        }
    }
    state.images.retain(|k, _| used_images.contains(k));

    // Buffers.
    use wgpu::util::DeviceExt;
    let align = u64::from(limits.min_uniform_buffer_offset_alignment);
    let draw_stride = align_up(std::mem::size_of::<DrawU>() as u64, align);
    let draw_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("frap eevee draws"),
        contents: &strided(&draws, draw_stride),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let mat_stride = align_up(std::mem::size_of::<GpuMaterial>() as u64, align);
    let gpu_mats: Vec<GpuMaterial> = mats.iter().map(MaterialParams::gpu).collect();
    let mat_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("frap eevee materials"),
        contents: &strided(&gpu_mats, mat_stride),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let frame_buf = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap eevee frame"),
        size: std::mem::size_of::<FrameU>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let light_buf = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap eevee lights"),
        size: (records.len().max(1) * std::mem::size_of::<GpuLight>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layers = plan.layers;
    let maps = plan.maps.max(1);
    let shadow_mat_buf = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap eevee shadow matrices"),
        size: u64::from(maps) * 64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let layer_stride = align_up(std::mem::size_of::<ShadowLayerU>() as u64, align);
    let layer_buf = dev.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frap eevee shadow layers"),
        size: u64::from(maps) * layer_stride,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Targets.
    let size = [width, height];
    let sample_tex = texture_2d(
        gpu,
        "frap eevee sample",
        size,
        1,
        1,
        film::SAMPLE_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
    );
    let sample_view = sample_tex.create_view(&Default::default());
    let depth_tex = texture_2d(
        gpu,
        "frap eevee depth",
        size,
        1,
        1,
        DEPTH_FORMAT,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let depth_view = depth_tex.create_view(&Default::default());
    let refraction = (!refractive.is_empty()).then(|| {
        texture_2d(
            gpu,
            "frap eevee refraction",
            size,
            1,
            1,
            film::SAMPLE_FORMAT,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        )
    });
    let refraction_view = refraction
        .as_ref()
        .map(|t| t.create_view(&Default::default()));
    let layer_size = plan.layer_size();
    let atlas_ok =
        matches!(&state.shadow_atlas, Some((s, l, _)) if *s == layer_size && *l == layers);
    if !atlas_ok {
        state.shadow_atlas = Some((
            layer_size,
            layers,
            texture_2d(
                gpu,
                "frap eevee shadow maps",
                [layer_size, layer_size],
                layers,
                1,
                shadows::FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
        ));
    }
    let state = &*state;
    let res = state.res.as_ref().expect("invariant: created above");
    let atlas = &state.shadow_atlas.as_ref().expect("invariant: set above").2;
    let atlas_view = atlas.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    let layer_views: Vec<wgpu::TextureView> = (0..layers)
        .map(|l| {
            atlas.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: l,
                array_layer_count: Some(1),
                ..Default::default()
            })
        })
        .collect();

    // Bind groups.
    let (spec_view, bg_view, mips) = match &world {
        World::Hdri(w) => (&w.specular, &w.background, w.specular_mips),
        World::Color(_) => (&res.black_cube, &res.white, 1),
    };
    let frame_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("frap eevee frame"),
        layout: &res.frame_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: light_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: shadow_mat_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&res.util_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&res.linear),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&atlas_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(&res.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(spec_view),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                resource: wgpu::BindingResource::TextureView(bg_view),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::TextureView(
                    refraction_view.as_ref().unwrap_or(&res.white),
                ),
            },
        ],
    });
    let draw_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("frap eevee draw"),
        layout: &res.draw_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &draw_buf,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64),
            }),
        }],
    });
    let layer_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("frap eevee shadow layer"),
        layout: &res.layer_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &layer_buf,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<ShadowLayerU>() as u64),
            }),
        }],
    });
    let material_bgs: Vec<wgpu::BindGroup> = mats
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let views: Vec<&wgpu::TextureView> = (0..MAX_TEXTURES)
                .map(|s| {
                    p.textures
                        .get(s)
                        .and_then(|t| state.images.get(&(t.image, t.is_data)))
                        .map_or(&res.white, |g| &g.view)
                })
                .collect();
            let samps: Vec<&wgpu::Sampler> = (0..MAX_TEXTURES)
                .map(|s| {
                    p.textures
                        .get(s)
                        .and_then(|t| res.samplers.get(&t.sampler))
                        .unwrap_or(&res.linear)
                })
                .collect();
            let mut entries = vec![wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &mat_buf,
                    offset: i as u64 * mat_stride,
                    size: wgpu::BufferSize::new(std::mem::size_of::<GpuMaterial>() as u64),
                }),
            }];
            for (s, v) in views.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1 + s as u32,
                    resource: wgpu::BindingResource::TextureView(v),
                });
            }
            for (s, smp) in samps.iter().enumerate() {
                entries.push(wgpu::BindGroupEntry {
                    binding: 1 + MAX_TEXTURES as u32 + s as u32,
                    resource: wgpu::BindingResource::Sampler(smp),
                });
            }
            dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("frap eevee material"),
                layout: &res.material_bgl,
                entries: &entries,
            })
        })
        .collect();

    let mut film_targets = FilmTargets::new(gpu, &res.film, size, &sample_view);

    // Per-sample constants.
    let samples = taa::samples(scene.settings.taa_render_samples);
    let filter_radius = film::filter_radius(scene.settings.eevee.filter_size, samples.len() as u32);
    let inv_view = view.inverse();
    let mut counts_bits = 0u32;
    if scene.settings.film_transparent {
        counts_bits |= FRAME_FILM_TRANSPARENT;
    }
    if scene.camera.is_perspective {
        counts_bits |= FRAME_PERSPECTIVE;
    }
    if refraction.is_some() {
        counts_bits |= FRAME_REFRACTION;
    }
    let (world_color, world_params, sh) = match &world {
        World::Color(c) => ([c[0], c[1], c[2], 0.0], [1.0, 0.0, 1.0, 0.0], [[0.0; 4]; 9]),
        World::Hdri(w) => {
            counts_bits |= FRAME_WORLD_HDRI;
            // `w.strength` is the sanitised Background strength (NaN reads as 0).
            let rot = w.rotation;
            ([0.0; 4], [rot.cos(), rot.sin(), w.strength, 0.0], w.sh9)
        }
    };
    let clamp_value = |v: f32| if v > 0.0 { v } else { 0.0 };

    let draw_meshes = |pass: &mut wgpu::RenderPass<'_>, cmds: &[DrawCmd], state: &State| {
        for cmd in cmds {
            let gm = cache
                .get(cmd.mesh)
                .expect("invariant: uploaded while building the draw lists");
            let attrs = &state.meshes[&cmd.mesh];
            let p = &mats[cmd.material];
            let uvs = p.uv_sources();
            pass.set_bind_group(1, &draw_bg, &[(cmd.draw as u64 * draw_stride) as u32]);
            pass.set_bind_group(2, &material_bgs[cmd.material], &[]);
            pass.set_vertex_buffer(0, gm.vertices.slice(..));
            pass.set_vertex_buffer(1, attrs.uv(uvs.first()).slice(..));
            pass.set_vertex_buffer(2, attrs.uv(uvs.get(1)).slice(..));
            pass.set_vertex_buffer(3, attrs.color(p.color_attribute.as_ref()).slice(..));
            pass.set_vertex_buffer(
                4,
                attrs
                    .tangents
                    .as_ref()
                    .unwrap_or(&attrs.zero_tangent)
                    .slice(..),
            );
            pass.set_index_buffer(gm.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(cmd.range.clone(), 0, 0..1);
        }
    };

    for (i, sample) in samples.enumerate() {
        frap_math::budget::check_now();
        let offset = sample.pixel_offset(filter_radius);
        let proj = taa::jitter_projection(scene.camera.projection, offset, size);
        let view_proj = proj * view;
        let frame = FrameU {
            view: view.to_cols_array(),
            proj: proj.to_cols_array(),
            view_proj: view_proj.to_cols_array(),
            inv_view_proj: view_proj.inverse().to_cols_array(),
            camera_pos: inv_view.w_axis.to_array(),
            camera_fwd: (-inv_view.z_axis.truncate()).extend(0.0).to_array(),
            world_color,
            resolution: [
                width as f32,
                height as f32,
                1.0 / width as f32,
                1.0 / height as f32,
            ],
            counts: [records.len() as u32, i as u32, mips, counts_bits],
            clamp: [
                clamp_value(scene.settings.eevee.clamp_surface_direct),
                clamp_value(scene.settings.eevee.clamp_surface_indirect),
                sample.transparency,
                0.0,
            ],
            world_params,
            shadow: [
                sample.pcf[0],
                sample.pcf[1],
                plan.tiles as f32,
                1.0 / shadow_size as f32,
            ],
            sh,
        };
        gpu.queue
            .write_buffer(&frame_buf, 0, bytemuck::bytes_of(&frame));
        let draw_shadows = plan.maps > 0
            && plan
                .lights
                .iter()
                .any(|(k, _)| *k != lights::ShadowKind::None)
            && (i == 0 || rerender_shadows);
        let (bindings, matrices) =
            shadows::sample_maps(&plan, &records, &sample, &scene.camera, &bounds);
        let gl: Vec<GpuLight> = records
            .iter()
            .zip(&bindings)
            .map(|(r, b)| r.gpu(b))
            .collect();
        if !gl.is_empty() {
            gpu.queue
                .write_buffer(&light_buf, 0, bytemuck::cast_slice(&gl));
        }
        let mats_flat: Vec<[f32; 16]> = matrices.iter().map(Mat4::to_cols_array).collect();
        gpu.queue
            .write_buffer(&shadow_mat_buf, 0, bytemuck::cast_slice(&mats_flat));
        let layers_u: Vec<ShadowLayerU> = mats_flat
            .iter()
            .enumerate()
            .map(|(i, m)| ShadowLayerU {
                view_proj: *m,
                params: [sample.transparency, i as f32, 0.0, 0.0],
            })
            .collect();
        if draw_shadows {
            gpu.queue
                .write_buffer(&layer_buf, 0, &strided(&layers_u, layer_stride));
        }

        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frap eevee sample"),
        });
        if draw_shadows {
            let per_layer = plan.tiles * plan.tiles;
            for (l, lv) in layer_views.iter().enumerate() {
                let maps_of_layer: Vec<u32> = (l as u32 * per_layer..(l as u32 + 1) * per_layer)
                    .filter(|m| plan.is_used(*m))
                    .collect();
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("frap eevee shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: lv,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                if maps_of_layer.is_empty() {
                    continue;
                }
                pass.set_pipeline(&res.shadow);
                for m in maps_of_layer {
                    // Each map renders into its tile (the whole layer without tiling).
                    let (_, [x, y]) = plan.tile_of(m);
                    pass.set_viewport(
                        x as f32,
                        y as f32,
                        shadow_size as f32,
                        shadow_size as f32,
                        0.0,
                        1.0,
                    );
                    pass.set_scissor_rect(x, y, shadow_size, shadow_size);
                    pass.set_bind_group(0, &layer_bg, &[(u64::from(m) * layer_stride) as u32]);
                    for (key, draw, range) in &casters {
                        let gm = cache.get(*key).expect("invariant: uploaded above");
                        pass.set_bind_group(1, &draw_bg, &[(*draw as u64 * draw_stride) as u32]);
                        pass.set_vertex_buffer(0, gm.vertices.slice(..));
                        pass.set_index_buffer(gm.indices.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(range.clone(), 0, 0..1);
                    }
                }
            }
        }
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frap eevee forward"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &sample_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &frame_bg, &[]);
            pass.set_pipeline(&res.background);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&res.forward);
            draw_meshes(&mut pass, &opaque, state);
        }
        if let Some(rt) = &refraction {
            enc.copy_texture_to_texture(
                sample_tex.as_image_copy(),
                rt.as_image_copy(),
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        if !refractive.is_empty() || !blended.is_empty() {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("frap eevee transparent"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &sample_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &frame_bg, &[]);
            pass.set_pipeline(&res.forward);
            draw_meshes(&mut pass, &refractive, state);
            pass.set_pipeline(&res.blended);
            draw_meshes(&mut pass, &blended, state);
        }
        let taps = film::sample_table(filter_radius, offset, sample.pcf);
        film_targets.accumulate(gpu, &res.film, &mut enc, &taps);
        gpu.queue.submit([enc.finish()]);
    }
    film_targets.resolve(gpu)
}

/// The alpha a material slot's shadow is hashed with (`use_transparent_shadow`, on by
/// default: the shadow pass discards a fragment when its hash exceeds the alpha, and the TAA
/// samples, which re-render jittered shadows, average to an alpha-weighted shadow); `None` for
/// an opaque (or alpha-clipped) material, which casts a full shadow. The alpha is the
/// material's `Alpha` factor (textures and vertex colours are not read by the shadow pass).
pub fn transparent_shadow_alpha(p: &MaterialParams) -> Option<f32> {
    match p.blend {
        BlendMode::Dithered | BlendMode::Blended if p.base_color[3] < 1.0 => {
            Some(p.base_color[3].max(0.0))
        }
        _ => None,
    }
}

/// The `(material slot, index range)` draws of a mesh (one draw for the whole mesh without
/// submeshes), clamped to the uploaded index count.
fn submeshes(m: &RMesh, count: u32) -> Vec<(u32, std::ops::Range<u32>)> {
    if m.submeshes.is_empty() {
        return vec![(0, 0..count)];
    }
    m.submeshes
        .iter()
        .map(|(s, r)| (*s, r.start.min(count)..r.end.min(count)))
        .filter(|(_, r)| !r.is_empty())
        .collect()
}

/// The material of an object's slot (`None`: the default surface).
fn material_key(obj: &RObject, slot: u32) -> Option<MatKey> {
    obj.materials.get(slot as usize).copied().flatten()
}

#[cfg(test)]
mod tests;
