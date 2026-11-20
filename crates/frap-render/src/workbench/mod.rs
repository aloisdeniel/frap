// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench engine (`BLENDER_WORKBENCH`, §19.3): one uber-shader, every `scene.display` /
//! `display.shading` setting Blender 5.2.4's Workbench reads in final renders. WP 1.7 (M1),
//! completed by WP 2.3 (M2).
//!
//! Per anti-aliasing sample (Blender's `Instance::draw`, `workbench_engine.cc`), with the
//! projection jittered by the sample offset:
//!
//! 1. opaque pass: surfaces shaded forward from Blender's quantised G-buffer values into an
//!    `Rgba32Float` sample target holding half-float values (cleared to the background), plus
//!    the colour each pixel takes in a stencil shadow and the object id;
//! 2. shadows ([`shadow`]): depth-fail stencil volumes, then shadowed pixels take their
//!    shadowed colour;
//! 3. transparency and X-ray ([`xray`]): weighted blended OIT accumulation and resolve, then the
//!    transparent depth/id pass;
//! 4. outline ([`outline`]): id edges blended over the colour;
//! 5. anti-aliasing ([`taa`]): the 3 x 3 Blackman-Harris weighted neighbourhood of the sample,
//!    in `log2(c + 1)` space, added to a half-float accumulator.
//!
//! The accumulator is read back once; the CPU divides by the summed weights, leaves log space
//! and un-premultiplies into a straight-alpha scene-linear [`Film`].
//!
//! Settings and their mapping (Blender 5.2.4, verified with F12 renders):
//! - `shading.type`: SOLID (and MATERIAL / RENDERED, which render as SOLID); WIREFRAME renders
//!   the background (and outlines) only ([`wire`]).
//! - `shading.light`: STUDIO (the `studio_light` rig, [`studio_light`]; view-space lights, or
//!   world-space ones rotated by `studiolight_rotate_z` with `use_world_space_lighting`),
//!   MATCAP ([`matcap`]), FLAT.
//! - `shading.color_type`: MATERIAL (`RSettings::workbench_materials`: the material's viewport
//!   `diffuse_color`/`metallic`/`roughness`; synthetic scenes: the lowered factors), OBJECT
//!   (`Object.color`), SINGLE (`single_color`), RANDOM ([`color::random_color`] of
//!   `RObject::random_hash`), VERTEX (Attribute: the colour attribute `RMesh::color_active`,
//!   with Blender's `Material(float3(-1))` values: alpha 1, roughness 0.4, metallic 0) and
//!   TEXTURE (the material's values with the colour of its active texture node's image on
//!   `RMesh::uv_active`, [`texture`]; WP 5.5). Per object, as `ObjectState`: VERTEX without a
//!   colour attribute and TEXTURE without a UV map or with a `display_type` other than
//!   TEXTURED draw the MATERIAL colour; a material without an active image draws its own
//!   colour.
//! - `show_xray`/`xray_alpha`, materials with alpha < 1, `show_object_outline` /
//!   `object_outline_color`, `show_shadows` / `shadow_intensity` with `display.light_direction`
//!   / `shadow_shift` / `shadow_focus`, `show_backface_culling`, `show_specular_highlight`.
//! - `display.render_aa`: OFF (one sample at pixel centres), 5 / 8 / 11 / 16 / 32 samples; FXAA
//!   renders one centred sample through the TAA filter without SMAA ([`taa`]).
//! - Background: `render.film_transparent` gives alpha 0; otherwise
//!   `RSettings::workbench_background` (Blender's rule, resolved by the builder), else the world
//!   colour (`RWorld::Color::rgb`; Workbench ignores the strength; an HDRI world uses the
//!   factory world colour).
//! - Not rendered (P2, [`cavity`]): cavity, depth of field.

pub mod cavity;
pub mod color;
pub mod matcap;
pub mod outline;
pub mod shadow;
pub mod studio_light;
pub mod taa;
pub mod texture;
pub mod wire;
pub mod xray;

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use frap_render_scene::{
    Film, MeshKey, RObject, RSettings, RWorkbenchTexture, RWorld, RenderError, RenderScene,
};
use glam::{Mat3, Mat4, Vec3, Vec4};

use crate::engine::{Engine, GpuCache, VERTEX_STRIDE};
use crate::gpu::Gpu;
use shadow::{Adjacency, ShadowParams};
use studio_light::StudioLight;

/// The uber-shader (all entry points, features selected by uniform flags).
pub const SHADER_WGSL: &str = include_str!("workbench.wgsl");

/// Samples per render (`scene.display.render_aa` factory value `'8'`).
pub const DEFAULT_SAMPLES: u32 = 8;

/// Format of the anti-aliasing accumulator (§19.2: `Rgba32Float`; it holds half-float values,
/// as Blender's `SFLOAT_16_16_16_16` accumulation target does).
pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
/// The colour target of each sample (`color_tx`: `SFLOAT_16_16_16_16` in Blender, and
/// blendable, which `Rgba32Float` is not without an optional feature).
const SAMPLE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The shadowed colour and the transparent accumulation (half floats, as Blender's).
const AUX_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// Transparent revealage (`SFLOAT_16`).
const REVEAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;
/// Object ids (Blender: `UINT_16`).
const ID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Uint;
/// Depth and the shadow stencil (Blender: `SFLOAT_32_DEPTH_UINT_8`, an optional wgpu feature;
/// 24-bit depth is always available).
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
/// Factory `world.color` (verified), used when the world is not a plain colour.
const FACTORY_WORLD_COLOR: [f32; 3] = [0.050876, 0.050876, 0.050876];

/// `shading.light`, as the shader sees it (values of Blender's `WORKBENCH_LIGHTING_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lighting {
    /// STUDIO (`WORKBENCH_LIGHTING_STUDIO`)
    Studio,
    /// MATCAP (`WORKBENCH_LIGHTING_MATCAP`)
    Matcap,
    /// FLAT (`WORKBENCH_LIGHTING_FLAT`)
    Flat,
}

impl Lighting {
    fn shader_value(self) -> u32 {
        match self {
            Lighting::Studio => 0,
            Lighting::Matcap => 1,
            Lighting::Flat => 2,
        }
    }
}

/// `shading.color_type` sources.
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
    /// VERTEX (the colour attribute)
    Vertex,
    /// TEXTURE (the active texture node's image)
    Texture,
}

/// Where a draw's surface colour comes from (`Draw.ids.y` in `workbench.wgsl`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorSource {
    /// [`Surface::base_color`] (MATERIAL, OBJECT, SINGLE, RANDOM; VERTEX and TEXTURE where
    /// they fall back).
    #[default]
    Uniform,
    /// The colour attribute `RMesh::color_active` (VERTEX).
    Attribute,
    /// The image of the material's `RMaterialSettings::workbench_texture` on
    /// `RMesh::uv_active` (TEXTURE).
    Image,
}

impl ColorSource {
    fn shader_value(self) -> u32 {
        match self {
            ColorSource::Uniform => 0,
            ColorSource::Attribute => 1,
            ColorSource::Image => 2,
        }
    }
}

/// The resolved `scene.display` / `display.shading` of one render (`SceneState::init`,
/// `SceneResources::init`, `ShadowPass::init`).
#[derive(Clone, Debug, PartialEq)]
pub struct Shading {
    /// Lighting mode.
    pub lighting: Lighting,
    /// Colour source.
    pub color_type: ColorType,
    /// `single_color`
    pub single_color: [f32; 3],
    /// Studio rig (`studio_light` in STUDIO mode, factory "Default").
    pub studio: StudioLight,
    /// The matcap (`studio_light` in MATCAP mode, [`matcap::resolve`]d).
    pub matcap: &'static str,
    /// `use_specular`: `show_specular_highlight` and the rig's (or matcap's) specular pass.
    pub use_specular: bool,
    /// `use_world_space_lighting`
    pub world_space_lighting: bool,
    /// `studiolight_rotate_z`
    pub studiolight_rotate_z: f32,
    /// Effective X-ray alpha (1: no X-ray, [`xray::xray_alpha`]).
    pub xray_alpha: f32,
    /// `draw_shadows` (off in X-ray mode).
    pub draw_shadows: bool,
    /// Shadow settings (`ShadowParams::off()` without shadows).
    pub shadow: ShadowParams,
    /// `draw_outline`
    pub draw_outline: bool,
    /// `object_outline_color`
    pub outline_color: [f32; 3],
    /// `show_backface_culling`
    pub backface_culling: bool,
    /// `draw_aa`: anti-aliasing on (`render_aa` other than OFF).
    pub draw_aa: bool,
    /// `samples_len`
    pub samples: u32,
}

impl Shading {
    /// Reads the Workbench part of the render settings.
    pub fn from_settings(s: &RSettings) -> Shading {
        let w = &s.workbench;
        let wireframe = wire::is_wireframe(&w.shading_type);
        let (light, color_type) = if wireframe {
            ("FLAT", "OBJECT")
        } else {
            (s.shading_light.as_str(), s.shading_color_type.as_str())
        };
        let lighting = match light {
            "FLAT" => Lighting::Flat,
            "MATCAP" => Lighting::Matcap,
            _ => Lighting::Studio,
        };
        let color_type = match color_type {
            "OBJECT" => ColorType::Object,
            "SINGLE" => ColorType::Single,
            "RANDOM" => ColorType::Random,
            "VERTEX" => ColorType::Vertex,
            "TEXTURE" => ColorType::Texture,
            _ => ColorType::Material,
        };
        // `BKE_studiolight_find(studio_light, STUDIOLIGHT_TYPE_STUDIO)` falls back to the
        // default rig.
        let studio = studio_light::get(&w.studio_light).unwrap_or_else(studio_light::default_rig);
        let matcap = matcap::resolve(&w.studio_light);
        let use_specular = w.show_specular_highlight
            && match lighting {
                Lighting::Matcap => matcap::get(matcap).is_ok_and(|m| m.has_specular),
                _ => studio.specular_highlight_pass,
            };
        let xray_alpha = xray::xray_alpha(w);
        let xray_mode = xray_alpha != 1.0;
        let draw_shadows = w.show_shadows && !xray_mode;
        let shadow = if draw_shadows {
            ShadowParams::new(
                w.light_direction,
                w.shadow_shift,
                w.shadow_focus,
                w.shadow_intensity,
            )
        } else {
            ShadowParams::off()
        };
        let (draw_aa, samples) = taa::samples_from_render_aa(&w.render_aa);
        Shading {
            lighting,
            color_type,
            single_color: s.shading_single_color,
            studio,
            matcap,
            use_specular,
            world_space_lighting: w.use_world_space_lighting,
            studiolight_rotate_z: w.studiolight_rotate_z,
            xray_alpha,
            draw_shadows,
            shadow,
            draw_outline: w.show_object_outline,
            outline_color: w.object_outline_color,
            backface_culling: w.show_backface_culling,
            draw_aa,
            samples,
        }
    }

    /// The four solid lights in view space (`get_light_data_from_studio_solidlight`): the rig's
    /// directions as they are, or, with world-space lighting, rotated by
    /// `get_world_shading_rotation_matrix(studiolight_rotate_z)` (the view rotation, a
    /// `-rotate_z` turn about Z, and the rig's y/z axes swapped with z negated).
    pub fn light_directions(&self, view: &Mat4) -> [[f32; 3]; 4] {
        let rot = Mat3::from_mat4(*view) * Mat3::from_rotation_z(-self.studiolight_rotate_z);
        std::array::from_fn(|i| {
            let d = self.studio.lights[i].direction;
            if self.world_space_lighting {
                (rot * Vec3::new(d[0], -d[2], d[1])).to_array()
            } else {
                d
            }
        })
    }
}

/// What the surface shader receives for one object / material slot.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Surface {
    /// Base colour (linear) and alpha (8-bit quantised, as `Material::pack_data`).
    pub base_color: [f32; 4],
    /// Metallic as the opaque lighting sees it (quantised like Blender's G-buffer).
    pub metallic: f32,
    /// Roughness as the opaque lighting sees it (quantised like Blender's G-buffer).
    pub roughness: f32,
    /// `(roughness, metallic)` as the transparent pass sees them (8-bit packing only).
    pub transparent_roughness_metallic: (f32, f32),
    /// `Material::is_transparent`: drawn in the transparent passes.
    pub is_transparent: bool,
    /// Where the colour comes from (the per-fragment sources replace `base_color`'s RGB).
    pub source: ColorSource,
}

/// The colour type `obj` draws with (`ObjectState::ObjectState`): TEXTURE needs a UV map and
/// `display_type = 'TEXTURED'`, VERTEX a colour attribute; otherwise MATERIAL.
pub fn object_color_type(scene: &RenderScene, obj: &RObject, shading: &Shading) -> ColorType {
    let mesh = obj.mesh.and_then(|k| scene.meshes.get(&k));
    match shading.color_type {
        ColorType::Texture
            if mesh.is_none_or(|m| m.uv_active.is_none()) || obj.display_type != "TEXTURED" =>
        {
            ColorType::Material
        }
        ColorType::Vertex if mesh.is_none_or(|m| m.color_active.is_none()) => {
            ColorType::Material
        }
        t => t,
    }
}

/// The TEXTURE image of `obj`'s material `slot` (`MaterialTexture(ob, slot)`): `None` for an
/// empty slot or a material without an active image.
pub fn slot_texture<'a>(
    scene: &'a RenderScene,
    obj: &RObject,
    slot: u32,
) -> Option<&'a RWorkbenchTexture> {
    let key = obj.materials.get(slot as usize).copied().flatten()?;
    scene
        .settings
        .material_settings
        .get(&key)?
        .workbench_texture
        .as_ref()
        .filter(|t| scene.images.contains_key(&t.image))
}

/// Blender's surface values for `obj`'s material `slot` under `shading`
/// (`workbench_materials.cc`, `Instance::get_material`).
pub fn surface(scene: &RenderScene, obj: &RObject, slot: u32, shading: &Shading) -> Surface {
    let color_type = object_color_type(scene, obj, shading);
    let mut source = ColorSource::Uniform;
    let (rgb, alpha, metallic, roughness) = match color_type {
        ColorType::Material | ColorType::Texture => {
            if color_type == ColorType::Texture && slot_texture(scene, obj, slot).is_some() {
                source = ColorSource::Image;
            }
            let key = obj.materials.get(slot as usize).copied().flatten();
            // Blender's Workbench reads `Material.diffuse_color` / `.metallic` / `.roughness`
            // (viewport display values, fresh-material roughness 0.4), not the Principled node:
            // the render-scene builder fills `RSettings::workbench_materials` (decision log
            // W1-15). Synthetic scenes without that entry use the lowered factors.
            let viewport = key.and_then(|k| scene.settings.workbench_materials.get(&k));
            let mat = key.and_then(|k| scene.materials.get(&k));
            match (viewport, mat) {
                (Some(v), _) => {
                    let c = v.diffuse_color;
                    ([c[0], c[1], c[2]], c[3], v.metallic, v.roughness)
                }
                (None, Some(m)) => {
                    let c = m.base_color.factor;
                    (
                        [c[0], c[1], c[2]],
                        c[3],
                        m.metallic.factor,
                        m.roughness.factor,
                    )
                }
                // `BKE_material_default_empty()`: 0.8 grey, metallic 0, roughness 0.4.
                (None, None) => ([0.8; 3], 1.0, 0.0, 0.4),
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
        // `material_attribute_color = Material(float3(-1.0f))`: the colour comes from the
        // attribute; alpha 1, roughness 0.4, metallic 0.
        ColorType::Vertex => {
            source = ColorSource::Attribute;
            ([-1.0; 3], 1.0, 0.0, 0.4)
        }
    };
    let (g_roughness, g_metallic) = color::gbuffer_roughness_metallic(roughness, metallic);
    Surface {
        base_color: [rgb[0], rgb[1], rgb[2], xray::packed_alpha(alpha)],
        metallic: g_metallic,
        roughness: g_roughness,
        transparent_roughness_metallic: color::packed_roughness_metallic(roughness, metallic),
        is_transparent: xray::is_transparent(alpha),
        source,
    }
}

/// The colour the sample target is cleared to (straight alpha).
pub fn background(scene: &RenderScene) -> [f32; 4] {
    if scene.settings.film_transparent {
        return [0.0; 4];
    }
    // The render-scene builder resolves Blender's rule (`World.color`, the VIEWPORT background
    // colour, or transparent without a world) into `workbench_background`. Synthetic scenes
    // without it use the `RWorld::Color` colour; an HDRI world falls back to the factory grey.
    if let Some(bg) = scene.settings.workbench_background {
        return bg;
    }
    let rgb = match &scene.world {
        RWorld::Color { rgb, .. } => *rgb,
        _ => FACTORY_WORLD_COLOR,
    };
    [rgb[0], rgb[1], rgb[2], 1.0]
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
    shadow: [f32; 4],
    shadow_direction: [f32; 4],
    params: [f32; 4],
    outline_color: [f32; 4],
    taa_weights: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct DrawU {
    model: [f32; 16],
    normal_matrix: [f32; 16],
    base_color: [f32; 4],
    params: [f32; 4],
    params2: [f32; 4],
    ids: [u32; 4],
}

// `Frame.mode.w` flags (workbench.wgsl).
const FLAG_BACKFACE_CULLING: u32 = 1;
const FLAG_TAA: u32 = 4;
const FLAG_FIRST_SAMPLE: u32 = 8;

struct DrawCmd {
    mesh: MeshKey,
    range: Range<u32>,
    uniform: usize,
    /// Index into the render's image bind groups (0: the white placeholder).
    image: usize,
    /// The colour attribute is bound (else the zero buffer).
    attribute: bool,
    /// The UV map is bound (else the zero buffer).
    uv: bool,
}

struct Pipelines {
    device: wgpu::Device,
    frame_bgl: wgpu::BindGroupLayout,
    draw_bgl: wgpu::BindGroupLayout,
    fs_bgl: wgpu::BindGroupLayout,
    image_bgl: wgpu::BindGroupLayout,
    opaque: wgpu::RenderPipeline,
    transparent: wgpu::RenderPipeline,
    object_id: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    shadow_resolve: wgpu::RenderPipeline,
    transparent_resolve: wgpu::RenderPipeline,
    outline: wgpu::RenderPipeline,
    taa: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

/// A matcap uploaded as a two-layer texture.
struct MatcapTexture {
    name: &'static str,
    view: wgpu::TextureView,
}

/// The Workbench engine. Pipelines are created on the first render and reused.
pub struct Workbench {
    samples: Option<u32>,
    pipes: Option<Pipelines>,
    pipeline_builds: u32,
    matcap: Option<MatcapTexture>,
    adjacency: HashMap<MeshKey, Arc<Adjacency>>,
    /// Colour-attribute and UV buffers, images and samplers of VERTEX / TEXTURE draws.
    textures: texture::Cache,
}

impl Default for Workbench {
    fn default() -> Self {
        Workbench::new()
    }
}

impl Workbench {
    /// An engine rendering the samples `scene.display.render_aa` asks for.
    pub fn new() -> Workbench {
        Workbench {
            samples: None,
            pipes: None,
            pipeline_builds: 0,
            matcap: None,
            adjacency: HashMap::new(),
            textures: texture::Cache::default(),
        }
    }

    /// An engine rendering `samples` samples per pixel whatever `render_aa` says: 1 is
    /// Blender's `render_aa = 'OFF'` (pixel centres, no filter), 5 / 8 / 11 / 16 / 32 its
    /// sample tables (other counts use the 5-sample table, as Blender's `default:` case).
    pub fn with_samples(samples: u32) -> Workbench {
        Workbench {
            samples: Some(samples.max(1)),
            ..Workbench::new()
        }
    }

    /// How many times the pipeline set (built from the one uber-shader) was built: once per
    /// device, never per material, lighting mode or feature.
    pub fn pipeline_builds(&self) -> u32 {
        self.pipeline_builds
    }

    /// Samples per pixel of the next render of `scene`.
    pub fn samples(&self, scene: &RenderScene) -> u32 {
        self.effective_aa(&Shading::from_settings(&scene.settings))
            .1
    }

    fn effective_aa(&self, shading: &Shading) -> (bool, u32) {
        match self.samples {
            Some(1) => (false, 1),
            Some(n) => (true, n),
            None => (shading.draw_aa, shading.samples),
        }
    }

    fn pipelines(&mut self, gpu: &Gpu) -> &Pipelines {
        if self.pipes.as_ref().is_none_or(|p| p.device != gpu.device) {
            self.pipes = Some(create_pipelines(gpu));
            self.pipeline_builds += 1;
            self.matcap = None;
            self.textures = texture::Cache::default();
        }
        self.pipes.as_ref().expect("invariant: just created")
    }

    /// The matcap texture view (a 1 x 1 black two-layer texture for other lighting modes).
    fn matcap_view(&mut self, gpu: &Gpu, name: Option<&'static str>) -> Result<(), RenderError> {
        let want = name.unwrap_or("");
        if self.matcap.as_ref().is_some_and(|m| m.name == want) {
            return Ok(());
        }
        let (size, texels) = match name {
            Some(n) => {
                let m = matcap::get(n).map_err(RenderError::Unsupported)?;
                (m.size, m.texels.clone())
            }
            None => (1, vec![0u16; 8]),
        };
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frap workbench matcap"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 2,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&texels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 8),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 2,
            },
        );
        let view = tex.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        self.matcap = Some(MatcapTexture { name: want, view });
        Ok(())
    }

    /// The edge adjacency of a mesh (cached by key; entries unused by a render are dropped).
    fn adjacency(&mut self, key: MeshKey, mesh: &frap_render_scene::RMesh) -> Arc<Adjacency> {
        self.adjacency
            .entry(key)
            .or_insert_with(|| Arc::new(shadow::adjacency(&mesh.positions, &mesh.indices)))
            .clone()
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

fn texture_entry(
    binding: u32,
    sample_type: wgpu::TextureSampleType,
    dim: wgpu::TextureViewDimension,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

fn color_target(
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> Option<wgpu::ColorTargetState> {
    Some(wgpu::ColorTargetState {
        format,
        blend,
        write_mask: wgpu::ColorWrites::ALL,
    })
}

fn blend(
    src: wgpu::BlendFactor,
    dst: wgpu::BlendFactor,
    src_a: wgpu::BlendFactor,
    dst_a: wgpu::BlendFactor,
) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: src,
            dst_factor: dst,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: src_a,
            dst_factor: dst_a,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn depth_state(write: bool, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(write),
        depth_compare: Some(compare),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn create_pipelines(gpu: &Gpu) -> Pipelines {
    use wgpu::BlendFactor as F;
    use wgpu::CompareFunction as C;
    let dev = &gpu.device;
    let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("frap workbench"),
        source: wgpu::ShaderSource::Wgsl(SHADER_WGSL.into()),
    });
    let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
    let float = |filterable| wgpu::TextureSampleType::Float { filterable };
    let d2 = wgpu::TextureViewDimension::D2;
    let frame_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench frame"),
        entries: &[
            uniform_entry(0, vf, std::mem::size_of::<FrameU>()),
            texture_entry(1, float(true), wgpu::TextureViewDimension::D2Array),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let draw_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench draw"),
        entries: &[uniform_entry(0, vf, std::mem::size_of::<DrawU>())],
    });
    let fs_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench full-screen"),
        entries: &[
            texture_entry(0, float(false), d2),
            texture_entry(1, float(false), d2),
            texture_entry(2, wgpu::TextureSampleType::Uint, d2),
        ],
    });
    let image_bgl = dev.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("frap workbench image"),
        entries: &[
            texture_entry(0, float(true), d2),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let geom_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap workbench geometry"),
        bind_group_layouts: &[Some(&frame_bgl), Some(&draw_bgl), Some(&image_bgl)],
        immediate_size: 0,
    });
    let frame_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap workbench frame only"),
        bind_group_layouts: &[Some(&frame_bgl)],
        immediate_size: 0,
    });
    let fs_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("frap workbench full-screen"),
        bind_group_layouts: &[Some(&frame_bgl), None, Some(&fs_bgl)],
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
    let color_attributes = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x4,
        offset: 0,
        shader_location: 2,
    }];
    let uv_attributes = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x2,
        offset: 0,
        shader_location: 3,
    }];
    let mesh_buffers = [
        Some(wgpu::VertexBufferLayout {
            array_stride: VERTEX_STRIDE,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &color_attributes,
        }),
        Some(wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &uv_attributes,
        }),
    ];
    let shadow_attributes = [wgpu::VertexAttribute {
        format: wgpu::VertexFormat::Float32x3,
        offset: 0,
        shader_location: 0,
    }];
    let shadow_buffers = [Some(wgpu::VertexBufferLayout {
        array_stride: 12,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &shadow_attributes,
    })];
    let tris = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: None,
        ..Default::default()
    };
    let pipeline = |label: &str,
                    layout: &wgpu::PipelineLayout,
                    vs: &str,
                    buffers: &[Option<wgpu::VertexBufferLayout>],
                    fs: Option<&str>,
                    targets: &[Option<wgpu::ColorTargetState>],
                    depth: Option<wgpu::DepthStencilState>| {
        dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some(vs),
                compilation_options: Default::default(),
                buffers,
            },
            primitive: tris,
            depth_stencil: depth,
            multisample: wgpu::MultisampleState::default(),
            fragment: fs.map(|entry| wgpu::FragmentState {
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                targets,
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let opaque = pipeline(
        "frap workbench opaque",
        &geom_layout,
        "vs_geom",
        &mesh_buffers,
        Some("fs_opaque"),
        &[
            color_target(SAMPLE_FORMAT, None),
            color_target(AUX_FORMAT, None),
            color_target(ID_FORMAT, None),
        ],
        Some(depth_state(true, C::LessEqual)),
    );
    // `DRW_STATE_BLEND_OIT`: rgb one/one, alpha zero/one-minus-source-alpha; revealage add.
    let oit = blend(F::One, F::One, F::Zero, F::OneMinusSrcAlpha);
    let add = blend(F::One, F::One, F::One, F::One);
    let transparent = pipeline(
        "frap workbench transparent",
        &geom_layout,
        "vs_geom",
        &mesh_buffers,
        Some("fs_transparent"),
        &[
            color_target(AUX_FORMAT, Some(oit)),
            color_target(REVEAL_FORMAT, Some(add)),
        ],
        Some(depth_state(false, C::LessEqual)),
    );
    let object_id = pipeline(
        "frap workbench transparent depth",
        &geom_layout,
        "vs_geom",
        &mesh_buffers,
        Some("fs_object_id"),
        &[color_target(ID_FORMAT, None)],
        Some(depth_state(true, C::LessEqual)),
    );
    // `GPU_STENCIL_OP_COUNT_DEPTH_FAIL`: back faces decrement, front faces increment.
    let face = |depth_fail_op| wgpu::StencilFaceState {
        compare: C::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op,
        pass_op: wgpu::StencilOperation::Keep,
    };
    let shadow_depth = wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: Some(false),
        depth_compare: Some(C::Less),
        stencil: wgpu::StencilState {
            front: face(wgpu::StencilOperation::IncrementWrap),
            back: face(wgpu::StencilOperation::DecrementWrap),
            read_mask: 0xff,
            write_mask: 0xff,
        },
        bias: wgpu::DepthBiasState::default(),
    };
    let shadow = pipeline(
        "frap workbench shadow volumes",
        &frame_layout,
        "vs_shadow",
        &shadow_buffers,
        None,
        &[],
        Some(shadow_depth),
    );
    let not_zero = wgpu::StencilFaceState {
        compare: C::NotEqual,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    let shadow_resolve = pipeline(
        "frap workbench shadow resolve",
        &fs_layout,
        "vs_full_far",
        &[],
        Some("fs_shadow_resolve"),
        &[color_target(SAMPLE_FORMAT, None)],
        Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(false),
            // Geometry only (the triangle is on the far plane, the background at depth 1).
            depth_compare: Some(C::Greater),
            stencil: wgpu::StencilState {
                front: not_zero,
                back: not_zero,
                read_mask: 0xff,
                write_mask: 0,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
    );
    // `DRW_STATE_BLEND_ALPHA`.
    let alpha = blend(
        F::SrcAlpha,
        F::OneMinusSrcAlpha,
        F::One,
        F::OneMinusSrcAlpha,
    );
    let transparent_resolve = pipeline(
        "frap workbench transparent resolve",
        &fs_layout,
        "vs_full",
        &[],
        Some("fs_transparent_resolve"),
        &[color_target(SAMPLE_FORMAT, Some(alpha))],
        None,
    );
    // `DRW_STATE_BLEND_ALPHA_PREMUL`.
    let premul = blend(F::One, F::OneMinusSrcAlpha, F::One, F::OneMinusSrcAlpha);
    let outline = pipeline(
        "frap workbench outline",
        &fs_layout,
        "vs_full",
        &[],
        Some("fs_outline"),
        &[color_target(SAMPLE_FORMAT, Some(premul))],
        None,
    );
    let taa = pipeline(
        "frap workbench accumulate",
        &fs_layout,
        "vs_full",
        &[],
        Some("fs_taa_accumulate"),
        &[color_target(COLOR_FORMAT, None)],
        None,
    );
    let sampler = dev.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("frap workbench matcap"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    Pipelines {
        device: dev.clone(),
        frame_bgl,
        draw_bgl,
        fs_bgl,
        image_bgl,
        opaque,
        transparent,
        object_id,
        shadow,
        shadow_resolve,
        transparent_resolve,
        outline,
        taa,
        sampler,
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

fn pass_desc<'a>(
    label: &'a str,
    color: &'a [Option<wgpu::RenderPassColorAttachment<'a>>],
    depth: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
) -> wgpu::RenderPassDescriptor<'a> {
    wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: color,
        depth_stencil_attachment: depth,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    }
}

/// Maps wgpu's `[0, w]` clip depth back to OpenGL's `[-w, w]` (Blender's `winmat`).
const WGPU_TO_GL_DEPTH: Mat4 = Mat4::from_cols(
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 2.0, 0.0),
    Vec4::new(0.0, 0.0, -1.0, 1.0),
);

/// Per-sample uniforms and the sum of the TAA weights (`weight_accum_`).
fn frame_uniforms(
    scene: &RenderScene,
    shading: &Shading,
    (draw_aa, samples): (bool, u32),
) -> (Vec<FrameU>, f32) {
    let [w, h] = scene.resolution;
    let view = scene.camera.view;
    let dirs = shading.light_directions(&view);
    let lights = std::array::from_fn(|i| {
        let l = &shading.studio.lights[i];
        if shading.lighting != Lighting::Matcap && l.use_light {
            LightU {
                direction: [dirs[i][0], dirs[i][1], dirs[i][2], 0.0],
                specular_color: [l.specular[0], l.specular[1], l.specular[2], 0.0],
                diffuse_color_wrap: [l.diffuse[0], l.diffuse[1], l.diffuse[2], l.smooth],
            }
        } else {
            // `get_light_data_from_studio_solidlight` for a disabled light (a matcap's lights
            // are all disabled).
            LightU {
                direction: [1.0, 0.0, 0.0, 0.0],
                specular_color: [0.0; 4],
                diffuse_color_wrap: [0.0; 4],
            }
        }
    });
    let a = if shading.lighting == Lighting::Matcap {
        [0.0; 3]
    } else {
        shading.studio.ambient
    };
    let gl_winmat = WGPU_TO_GL_DEPTH * scene.camera.projection;
    let sd = (Mat3::from_mat4(view) * shading.shadow.direction_ws).to_array();
    let mut flags = 0u32;
    if shading.backface_culling {
        flags |= FLAG_BACKFACE_CULLING;
    }
    if draw_aa {
        flags |= FLAG_TAA;
    }
    let mut weight_accum = 0.0f32;
    let frames = (0..samples)
        .map(|sample| {
            let (proj, weights) = if draw_aa {
                let offset = taa::sample_offset(samples, sample);
                let (weights, sum) = taa::taa_weights(offset);
                weight_accum += sum;
                let t = taa::jitter_matrix(&gl_winmat, &view, offset, [w, h]);
                (t * scene.camera.projection, weights)
            } else {
                (scene.camera.projection, [0.0; 9])
            };
            let first = if sample == 0 { FLAG_FIRST_SAMPLE } else { 0 };
            FrameU {
                view: view.to_cols_array(),
                proj: proj.to_cols_array(),
                lights,
                ambient_color: [a[0], a[1], a[2], 0.0],
                mode: [
                    shading.lighting.shader_value(),
                    u32::from(shading.use_specular),
                    u32::from(scene.camera.is_perspective),
                    flags | first,
                ],
                shadow: [
                    shading.shadow.shift,
                    shading.shadow.focus,
                    shading.shadow.mul,
                    shading.shadow.add,
                ],
                shadow_direction: [sd[0], sd[1], sd[2], 0.0],
                params: [
                    shading.xray_alpha,
                    scene.camera.clip[0],
                    scene.camera.clip[1],
                    0.0,
                ],
                outline_color: [
                    shading.outline_color[0],
                    shading.outline_color[1],
                    shading.outline_color[2],
                    1.0,
                ],
                taa_weights: [
                    [weights[0], weights[1], weights[2], weights[3]],
                    [weights[4], weights[5], weights[6], weights[7]],
                    [weights[8], 0.0, 0.0, 0.0],
                ],
            }
        })
        .collect();
    (frames, weight_accum)
}

/// The accumulated pixel → Blender's final colour (premultiplied): `workbench_effect_smaa_frag`
/// stage 2 without SMAA (`mix_factor` is 0 for 5 samples and more): divided by the summed
/// weights, out of log space, alpha above 0.999 snapped to 1, stored in a half-float target
/// (rounded toward zero, see [`color::round_f16_toward_zero`]).
pub fn resolve_taa(acc: [f32; 4], weight_accum: Option<f32>) -> [f32; 4] {
    let out = match weight_accum {
        Some(w) => {
            let c = acc.map(|v| v / w);
            let mut out = [
                c[0].exp2() - 1.0,
                c[1].exp2() - 1.0,
                c[2].exp2() - 1.0,
                c[3],
            ];
            if out[3] > 0.999 {
                out[3] = 1.0;
            }
            out
        }
        // `render_aa = 'OFF'`: the colour target as it is.
        None => acc,
    };
    out.map(color::round_f16_toward_zero)
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
        let aa = self.effective_aa(&shading);
        let xray_mode = shading.xray_alpha != 1.0;

        // Draw lists: one draw per object and material slot range, opaque or transparent.
        let view = scene.camera.view;
        let mut opaque: Vec<DrawCmd> = vec![];
        let mut transparent: Vec<DrawCmd> = vec![];
        let mut draw_data: Vec<DrawU> = vec![];
        let mut casters: Vec<(MeshKey, Mat4)> = vec![];
        let mut image_slots = texture::Slots::default();
        for (index, obj) in scene.objects.iter().enumerate() {
            let Some(key) = obj.mesh else { continue };
            let rmesh = scene.meshes.get(&key).ok_or_else(|| {
                RenderError::Unsupported(format!(
                    "object {:?} names mesh {key:?}, which the render scene lacks",
                    obj.name
                ))
            })?;
            if !wire::draws_surface(&obj.display_type) {
                continue;
            }
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
            // `resource_id + 1`: 0 is the background.
            let object_id = index as u32 + 1;
            for (slot, range) in subs {
                let range = range.start.min(count)..range.end.min(count);
                if range.is_empty() {
                    continue;
                }
                let s = surface(scene, obj, *slot, &shading);
                let tex = (s.source == ColorSource::Image)
                    .then(|| slot_texture(scene, obj, *slot))
                    .flatten();
                let (cutoff, image_bits) = tex.map_or((texture::NO_CUTOFF, 0), |t| {
                    (texture::cutoff(t), texture::image_bits(t))
                });
                draw_data.push(DrawU {
                    model: obj.matrix_world.to_cols_array(),
                    normal_matrix: normal_matrix.to_cols_array(),
                    base_color: s.base_color,
                    params: [s.metallic, s.roughness, flip, cutoff],
                    params2: [
                        s.transparent_roughness_metallic.0,
                        s.transparent_roughness_metallic.1,
                        0.0,
                        0.0,
                    ],
                    ids: [object_id, s.source.shader_value(), image_bits, 0],
                });
                let cmd = DrawCmd {
                    mesh: key,
                    range,
                    uniform: draw_data.len() - 1,
                    image: tex.map_or(0, |t| image_slots.slot(t)),
                    attribute: s.source == ColorSource::Attribute,
                    uv: s.source == ColorSource::Image,
                };
                if xray_mode || s.is_transparent {
                    transparent.push(cmd);
                } else {
                    opaque.push(cmd);
                }
            }
            if shading.draw_shadows {
                casters.push((key, obj.matrix_world));
            }
        }

        // Shadow volumes (world space), built once for every sample.
        let mut volumes: Vec<[f32; 3]> = vec![];
        if shading.draw_shadows && !opaque.is_empty() {
            let gl_winmat = WGPU_TO_GL_DEPTH * scene.camera.projection;
            let far = shadow::far_plane(&gl_winmat, &view);
            let used: Vec<MeshKey> = casters.iter().map(|c| c.0).collect();
            self.adjacency.retain(|k, _| used.contains(k));
            for (key, model) in &casters {
                let rmesh = &scene.meshes[key];
                let adj = self.adjacency(*key, rmesh);
                shadow::volume(&adj, model, &shading.shadow, &far, &mut volumes);
            }
        }

        let matcap = (shading.lighting == Lighting::Matcap).then_some(shading.matcap);
        self.pipelines(gpu);
        self.matcap_view(gpu, matcap)?;
        // Colour attributes, UV maps and images of the VERTEX / TEXTURE draws.
        self.textures.begin_render();
        for cmd in opaque.iter().chain(&transparent) {
            let rmesh = &scene.meshes[&cmd.mesh];
            if cmd.attribute {
                self.textures.attribute(gpu, cmd.mesh, rmesh);
            }
            if cmd.uv {
                self.textures.uv(gpu, cmd.mesh, rmesh);
            }
        }
        let max_vertices = opaque
            .iter()
            .chain(&transparent)
            .map(|c| scene.meshes[&c.mesh].positions.len())
            .max()
            .unwrap_or(0);
        self.textures.ensure_zeros(gpu, max_vertices);
        let pipes = self.pipes.as_ref().expect("invariant: built above");
        let image_bgs = self
            .textures
            .bind_groups(gpu, &pipes.image_bgl, scene, &image_slots);
        let textures = &self.textures;
        let matcap_view = &self.matcap.as_ref().expect("invariant: set above").view;
        let dev = &gpu.device;
        let align = u64::from(dev.limits().min_uniform_buffer_offset_alignment);
        let frame_stride = align_up(std::mem::size_of::<FrameU>() as u64, align);
        let draw_stride = align_up(std::mem::size_of::<DrawU>() as u64, align);
        let (frames, weight_accum) = frame_uniforms(scene, &shading, aa);

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
        let volume_buf = (!volumes.is_empty()).then(|| {
            dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("frap workbench shadow volumes"),
                contents: bytemuck::cast_slice(&volumes),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let tu = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let sample_tex = texture(dev, "frap workbench sample", size, SAMPLE_FORMAT, tu);
        let aux_tex = texture(dev, "frap workbench shadowed", size, AUX_FORMAT, tu);
        let accum_tex = texture(dev, "frap workbench transparent", size, AUX_FORMAT, tu);
        let reveal_tex = texture(dev, "frap workbench revealage", size, REVEAL_FORMAT, tu);
        let id_tex = texture(dev, "frap workbench object id", size, ID_FORMAT, tu);
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
        let aux_view = aux_tex.create_view(&dv);
        let accum_view = accum_tex.create_view(&dv);
        let reveal_view = reveal_tex.create_view(&dv);
        let id_view = id_tex.create_view(&dv);
        let depth_view = depth_tex.create_view(&dv);
        let acc_view = [acc_tex[0].create_view(&dv), acc_tex[1].create_view(&dv)];

        let frame_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frap workbench frame"),
            layout: &pipes.frame_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &frame_buf,
                        offset: 0,
                        size: wgpu::BufferSize::new(std::mem::size_of::<FrameU>() as u64),
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(matcap_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&pipes.sampler),
                },
            ],
        });
        let draw_bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frap workbench draw"),
            layout: &pipes.draw_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &draw_buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<DrawU>() as u64),
                }),
            }],
        });
        let fs_bg = |label: &str, a: &wgpu::TextureView, b: &wgpu::TextureView| {
            dev.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &pipes.fs_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(a),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(b),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&id_view),
                    },
                ],
            })
        };
        let shadow_bg = fs_bg("frap workbench shadow resolve", &aux_view, &aux_view);
        let transparent_bg = fs_bg(
            "frap workbench transparent resolve",
            &accum_view,
            &reveal_view,
        );
        let outline_bg = fs_bg("frap workbench outline", &aux_view, &aux_view);
        // taa_bg[i] reads accumulator i (and the sample) and writes accumulator 1 - i.
        let taa_bg =
            [0usize, 1].map(|i| fs_bg("frap workbench accumulate", &acc_view[i], &sample_view));

        // Stored as the colour target's half floats (rounded toward zero, as everywhere).
        let bg = background(scene).map(color::round_f16_toward_zero);
        let clear = |c: [f32; 4]| wgpu::Color {
            r: f64::from(c[0]),
            g: f64::from(c[1]),
            b: f64::from(c[2]),
            a: f64::from(c[3]),
        };
        let attach = |view, load| {
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
        let depth_load = |depth: wgpu::LoadOp<f32>, stencil: wgpu::LoadOp<u32>| {
            Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: depth,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: Some(wgpu::Operations {
                    load: stencil,
                    store: wgpu::StoreOp::Store,
                }),
            })
        };
        let draw_meshes = |pass: &mut wgpu::RenderPass<'_>, cmds: &[DrawCmd]| {
            for cmd in cmds {
                let gm = cache
                    .get(cmd.mesh)
                    .expect("invariant: uploaded while building the draw list");
                pass.set_bind_group(1, &draw_bg, &[(cmd.uniform as u64 * draw_stride) as u32]);
                pass.set_bind_group(2, &image_bgs[cmd.image], &[]);
                pass.set_vertex_buffer(0, gm.vertices.slice(..));
                let zeros = textures.zeros();
                let color = if cmd.attribute {
                    textures.attribute_buffer(cmd.mesh).unwrap_or(zeros)
                } else {
                    zeros
                };
                let uv = if cmd.uv {
                    textures.uv_buffer(cmd.mesh).unwrap_or(zeros)
                } else {
                    zeros
                };
                pass.set_vertex_buffer(1, color.slice(..));
                pass.set_vertex_buffer(2, uv.slice(..));
                pass.set_index_buffer(gm.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(cmd.range.clone(), 0, 0..1);
            }
        };

        let mut enc = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frap workbench"),
        });
        for i in 0..frames.len() {
            let frame_offset = [(i as u64 * frame_stride) as u32];
            {
                // 1. Opaque surfaces.
                let mut pass = enc.begin_render_pass(&pass_desc(
                    "frap workbench opaque",
                    &[
                        attach(&sample_view, wgpu::LoadOp::Clear(clear(bg))),
                        attach(&aux_view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)),
                        attach(&id_view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)),
                    ],
                    depth_load(wgpu::LoadOp::Clear(1.0), wgpu::LoadOp::Clear(0)),
                ));
                pass.set_pipeline(&pipes.opaque);
                pass.set_bind_group(0, &frame_bg, &frame_offset);
                draw_meshes(&mut pass, &opaque);
            }
            if let Some(vb) = &volume_buf {
                // 2. Stencil shadow volumes, then shadowed pixels take their shadowed colour.
                {
                    let mut pass = enc.begin_render_pass(&pass_desc(
                        "frap workbench shadow volumes",
                        &[],
                        depth_load(wgpu::LoadOp::Load, wgpu::LoadOp::Load),
                    ));
                    pass.set_pipeline(&pipes.shadow);
                    pass.set_bind_group(0, &frame_bg, &frame_offset);
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.draw(0..volumes.len() as u32, 0..1);
                }
                let mut pass = enc.begin_render_pass(&pass_desc(
                    "frap workbench shadow resolve",
                    &[attach(&sample_view, wgpu::LoadOp::Load)],
                    depth_load(wgpu::LoadOp::Load, wgpu::LoadOp::Load),
                ));
                pass.set_pipeline(&pipes.shadow_resolve);
                pass.set_stencil_reference(0);
                pass.set_bind_group(0, &frame_bg, &frame_offset);
                pass.set_bind_group(2, &shadow_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            if !transparent.is_empty() {
                // 3. Transparent accumulation and resolve, then their depth and ids.
                {
                    let mut pass = enc.begin_render_pass(&pass_desc(
                        "frap workbench transparent",
                        &[
                            attach(
                                &accum_view,
                                wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.0,
                                    g: 0.0,
                                    b: 0.0,
                                    a: 1.0,
                                }),
                            ),
                            attach(&reveal_view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)),
                        ],
                        depth_load(wgpu::LoadOp::Load, wgpu::LoadOp::Load),
                    ));
                    pass.set_pipeline(&pipes.transparent);
                    pass.set_bind_group(0, &frame_bg, &frame_offset);
                    draw_meshes(&mut pass, &transparent);
                }
                {
                    let mut pass = enc.begin_render_pass(&pass_desc(
                        "frap workbench transparent resolve",
                        &[attach(&sample_view, wgpu::LoadOp::Load)],
                        None,
                    ));
                    pass.set_pipeline(&pipes.transparent_resolve);
                    pass.set_bind_group(0, &frame_bg, &frame_offset);
                    pass.set_bind_group(2, &transparent_bg, &[]);
                    pass.draw(0..3, 0..1);
                }
                if shading.draw_outline {
                    let mut pass = enc.begin_render_pass(&pass_desc(
                        "frap workbench transparent depth",
                        &[attach(&id_view, wgpu::LoadOp::Load)],
                        depth_load(wgpu::LoadOp::Load, wgpu::LoadOp::Load),
                    ));
                    pass.set_pipeline(&pipes.object_id);
                    pass.set_bind_group(0, &frame_bg, &frame_offset);
                    draw_meshes(&mut pass, &transparent);
                }
            }
            if shading.draw_outline {
                // 4. Outline.
                let mut pass = enc.begin_render_pass(&pass_desc(
                    "frap workbench outline",
                    &[attach(&sample_view, wgpu::LoadOp::Load)],
                    None,
                ));
                pass.set_pipeline(&pipes.outline);
                pass.set_bind_group(0, &frame_bg, &frame_offset);
                pass.set_bind_group(2, &outline_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            {
                // 5. Anti-aliasing accumulation.
                let mut pass = enc.begin_render_pass(&pass_desc(
                    "frap workbench accumulate",
                    &[attach(
                        &acc_view[(i + 1) % 2],
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    )],
                    None,
                ));
                pass.set_pipeline(&pipes.taa);
                pass.set_bind_group(0, &frame_bg, &frame_offset);
                pass.set_bind_group(2, &taa_bg[i % 2], &[]);
                pass.draw(0..3, 0..1);
            }
        }
        gpu.queue.submit([enc.finish()]);

        let result = &acc_tex[frames.len() % 2];
        let mut rgba = gpu.read_texture(result, width, height, COLOR_FORMAT)?;
        let weights = aa.0.then_some(weight_accum);
        for p in &mut rgba {
            *p = unpremultiply(resolve_taa(*p, weights));
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
        // WGSL sizes: Frame = 2 mat4 (128) + 4 × Light (48) + 6 vec4 (96) + 3 vec4 (48) = 464;
        // Draw = 2 mat4 (128) + 4 vec4 (64) = 192.
        assert_eq!(std::mem::size_of::<FrameU>(), 464);
        assert_eq!(std::mem::size_of::<DrawU>(), 192);
    }

    #[test]
    fn unpremultiply_edges() {
        assert_eq!(unpremultiply([0.0; 4]), [0.0; 4]);
        assert_eq!(unpremultiply([0.2, 0.4, 0.6, 1.0]), [0.2, 0.4, 0.6, 1.0]);
        assert_eq!(unpremultiply([0.1, 0.2, 0.3, 0.5]), [0.2, 0.4, 0.6, 0.5]);
    }

    #[test]
    fn taa_resolve_leaves_log_space() {
        // A flat colour c seen by every sample: acc = Σ wsum × log2(1 + c).
        let c = 0.5f32;
        let w = 7.5f32;
        let acc = [w * (1.0 + c).log2(); 3];
        let out = resolve_taa([acc[0], acc[1], acc[2], w], Some(w));
        assert!((out[0] - c).abs() < 1e-3, "{out:?}");
        assert_eq!(out[3], 1.0);
        assert_eq!(
            resolve_taa([0.1, 0.2, 0.3, 1.0], None),
            [0.1, 0.2, 0.3, 1.0].map(color::round_f16_toward_zero)
        );
    }

    /// The CPU emulation of the accumulation of a flat region (every neighbour the same half
    /// float colour), as `fs_taa_accumulate` does it on the GPU.
    pub(crate) fn emulate_flat(c: f32, samples: u32) -> [f32; 4] {
        let rtz = color::round_f16_toward_zero;
        let c = rtz(c);
        let lc = (c + 1.0).log2();
        let mut acc = [0.0f32; 2];
        let mut w_total = 0.0f32;
        for s in 0..samples {
            let (w, sum) = taa::taa_weights(taa::sample_offset(samples, s));
            w_total += sum;
            let mut v = [0.0f32; 2];
            for wi in w {
                v[0] += lc * wi;
                v[1] += wi;
            }
            acc = if s == 0 {
                v.map(rtz)
            } else {
                [rtz(acc[0] + v[0]), rtz(acc[1] + v[1])]
            };
        }
        resolve_taa([acc[0], acc[0], acc[0], acc[1]], Some(w_total))
    }

    #[test]
    fn flat_accumulation_matches_blender() {
        // Blender 5.2.4 Workbench, FLAT SINGLE (0.123457, 0.987654, 0.333333), centre pixel of
        // the scene-linear render (premultiplied), render_aa 8 and 32 (Metal).
        let r8 = emulate_flat(0.987_654, 8);
        assert_eq!(r8[3], 0.998_046_9);
        assert_eq!(r8[1], 0.984_863_3, "{r8:?}");
        let r32 = emulate_flat(0.987_654, 32);
        assert_eq!(r32[3], 0.993_164_06);
        assert_eq!(r32[1], 0.979_980_47, "{r32:?}");
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
        assert_eq!((d.draw_aa, d.samples), (true, 8));
        assert_eq!(d.xray_alpha, 1.0);
        assert!(!d.draw_shadows && !d.draw_outline);
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
            (m.lighting, m.color_type, m.matcap),
            (Lighting::Matcap, ColorType::Material, "basic_bright.exr")
        );
        assert!(m.use_specular, "basic_bright has a specular pass");
        s.workbench.studio_light = "check_normal+y.exr".into();
        assert!(!Shading::from_settings(&s).use_specular);

        // WIREFRAME: FLAT, OBJECT, X-ray alpha 0 (no shadows).
        let mut w = RSettings::default();
        w.workbench.shading_type = "WIREFRAME".into();
        w.workbench.show_shadows = true;
        let wf = Shading::from_settings(&w);
        assert_eq!(
            (wf.lighting, wf.color_type, wf.xray_alpha, wf.draw_shadows),
            (Lighting::Flat, ColorType::Object, 0.0, false)
        );
        // Shadows and AA settings.
        w.workbench.shading_type = "SOLID".into();
        w.workbench.render_aa = "OFF".into();
        let sh = Shading::from_settings(&w);
        assert!(sh.draw_shadows);
        assert_eq!((sh.draw_aa, sh.samples), (false, 1));
        assert_eq!((sh.shadow.mul, sh.shadow.add), (0.5, 0.5));
        // X-ray turns shadows off.
        w.workbench.show_xray = true;
        assert!(!Shading::from_settings(&w).draw_shadows);
    }

    #[test]
    fn world_space_lighting_rotates_the_rig() {
        let mut s = RSettings::default();
        let view = Mat4::IDENTITY;
        let base = Shading::from_settings(&s).light_directions(&view);
        assert_eq!(base[1], studio_light::default_rig().lights[1].direction);
        s.workbench.use_world_space_lighting = true;
        let ws = Shading::from_settings(&s).light_directions(&view);
        let d = studio_light::default_rig().lights[1].direction;
        // Identity view, no rotation: (x, -z, y).
        assert!((Vec3::from(ws[1]) - Vec3::new(d[0], -d[2], d[1])).length() < 1e-6);
        s.workbench.studiolight_rotate_z = std::f32::consts::FRAC_PI_2;
        let r = Shading::from_settings(&s).light_directions(&view);
        // A -90° turn about Z maps (x, y) to (y, -x).
        let want = Vec3::new(-d[2], -d[0], d[1]);
        assert!((Vec3::from(r[1]) - want).length() < 1e-5, "{:?}", r[1]);
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
        // The builder's resolved Workbench background wins over the world colour.
        sc.settings.workbench_background = Some([0.0; 4]);
        sc.settings.film_transparent = false;
        assert_eq!(background(&sc), [0.0; 4]);
        sc.settings.workbench_background = Some([0.9, 0.1, 0.1, 1.0]);
        assert_eq!(background(&sc), [0.9, 0.1, 0.1, 1.0]);
        sc.settings.film_transparent = true;
        assert_eq!(background(&sc), [0.0; 4]);
    }

    #[test]
    fn material_colour_prefers_viewport_values() {
        use frap_render_scene::{MatKey, RWorkbenchMaterial};
        let mut sc = RenderScene::default();
        let obj = RObject {
            name: "Cube".into(),
            matrix_world: Mat4::IDENTITY,
            mesh: None,
            materials: vec![Some(MatKey(7)), None],
            color: [1.0; 4],
            random_hash: 0,
            display_type: "TEXTURED".into(),
        };
        let shading = Shading::from_settings(&sc.settings);
        let gb = |r: f32, m: f32| color::gbuffer_roughness_metallic(r, m);
        // No entry anywhere: Blender's default material.
        let s = surface(&sc, &obj, 0, &shading);
        assert_eq!(s.base_color, [0.8, 0.8, 0.8, 1.0]);
        assert_eq!((s.roughness, s.metallic), gb(0.4, 0.0));
        assert!(!s.is_transparent);
        sc.settings.workbench_materials.insert(
            MatKey(7),
            RWorkbenchMaterial {
                diffuse_color: [0.6, 0.6, 0.7, 0.5],
                metallic: 0.25,
                roughness: 0.75,
            },
        );
        let s = surface(&sc, &obj, 0, &shading);
        assert_eq!(s.base_color, [0.6, 0.6, 0.7, 128.0 / 255.0]);
        assert_eq!((s.roughness, s.metallic), gb(0.75, 0.25));
        assert!(s.is_transparent);
        assert_eq!(
            s.transparent_roughness_metallic,
            color::packed_roughness_metallic(0.75, 0.25)
        );
        // An empty slot keeps the default material.
        let s = surface(&sc, &obj, 1, &shading);
        assert_eq!(s.base_color, [0.8, 0.8, 0.8, 1.0]);
    }
}
