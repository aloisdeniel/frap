// SPDX-License-Identifier: GPL-3.0-or-later
//! Render-scene types (ARCHITECTURE §19.2, CONTRACT frozen at M1 W0; `GpuOptions` and
//! `RenderRequest` defined here by RFC 0001 F-REN-1/F-REN-2). Plain data, no wgpu: render tests
//! build synthetic `RenderScene`s.

use std::ops::Range;
use std::sync::Arc;

use frap_imbuf::DecodedImage;
use frap_material::LoweredMaterial;
use frap_rna::{IdKey, IdRef, ImageId};
use glam::Mat4;
use indexmap::IndexMap;
use smol_str::SmolStr;

/// GPU mesh cache key (evaluated fingerprint).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshKey(pub u64);
/// Material key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MatKey(pub u64);
/// Image key (`(IdKey, generation)` hash).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageKey(pub u64);

/// What to render (§19.1 `build(…, &RenderRequest)`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderRequest {
    /// Frame (`None` = `scene.frame_current`).
    pub frame: Option<i32>,
    /// Override of `resolution_x/y × percentage` (contact-sheet tiles).
    pub resolution: Option<[u32; 2]>,
    /// Override camera (contact-sheet synthetic views); `None` = `scene.camera`.
    pub camera: Option<RCamera>,
    /// Restrict to these object names (contact sheet `objects=`); `None` = all visible.
    pub objects: Option<Vec<SmolStr>>,
}

/// Camera: view and projection (sensor fit, shift, clip, ortho scale), pixel aspect.
#[derive(Clone, Debug, PartialEq)]
pub struct RCamera {
    /// World → view.
    pub view: Mat4,
    /// View → clip (wgpu depth range).
    pub projection: Mat4,
    /// Perspective or orthographic.
    pub is_perspective: bool,
    /// `(clip_start, clip_end)`
    pub clip: [f32; 2],
    /// `pixel_aspect_y / pixel_aspect_x`
    pub pixel_aspect: f32,
}

impl Default for RCamera {
    fn default() -> Self {
        RCamera {
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
            is_perspective: true,
            clip: [0.1, 100.0],
            pixel_aspect: 1.0,
        }
    }
}

/// One object instance.
#[derive(Clone, Debug, PartialEq)]
pub struct RObject {
    /// Object name (objects are sorted by it: determinism).
    pub name: SmolStr,
    /// `matrix_world`
    pub matrix_world: Mat4,
    /// Evaluated mesh, if any.
    pub mesh: Option<MeshKey>,
    /// Material per slot.
    pub materials: Vec<Option<MatKey>>,
    /// `Object.color` (Workbench OBJECT colour).
    pub color: [f32; 4],
    /// Workbench RANDOM colour hash: `BLI_ghashutil_strhash_p_murmur(ob->id.name)`, i.e.
    /// MurmurHash2 (seed 0) of the UTF-8 bytes `"OB" + name` including the trailing NUL (a
    /// linked object also mixes in its library path, as `workbench_materials.cc` does). The
    /// renderer applies `BLI_hash_int_01` itself; do not pre-hash. Reference implementation:
    /// `murmur2` in `crates/frap-render/tests/workbench_ref.rs`.
    pub random_hash: u32,
    /// `Object.display_type` (`TEXTURED`, `SOLID`, `WIRE`, `BOUNDS`): Workbench draws `WIRE`
    /// objects as wireframes and `BOUNDS` as their bounding box (M2 W0, WP 2.3).
    pub display_type: SmolStr,
}

/// Mesh data uploaded to the GPU (corner-split, triangulated).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RMesh {
    /// Positions.
    pub positions: Vec<[f32; 3]>,
    /// Normals.
    pub normals: Vec<[f32; 3]>,
    /// UV layers.
    pub uvs: Vec<Vec<[f32; 2]>>,
    /// Colour layers (linear).
    pub colors: Vec<Vec<[f32; 4]>>,
    /// Tangents (xyz + sign).
    pub tangents: Option<Vec<[f32; 4]>>,
    /// Triangle indices.
    pub indices: Vec<u32>,
    /// `(material slot, index range)`.
    pub submeshes: Vec<(u32, Range<u32>)>,
    /// Wireframe edges.
    pub edges: Vec<[u32; 2]>,
    /// Name of each UV layer of [`RMesh::uvs`] (M5+M6 W0: the EEVEE-like engine resolves a
    /// `UvSource::Named` texture coordinate by name).
    pub uv_names: Vec<SmolStr>,
    /// Index into [`RMesh::uvs`] of the active-render UV map (`UvSource::ActiveRender`).
    pub uv_active_render: Option<usize>,
    /// Name of each colour layer of [`RMesh::colors`] (Color Attribute nodes, vertex colours).
    pub color_names: Vec<SmolStr>,
    /// Index into [`RMesh::uvs`] of `active_or_default_uv_map_name()`: the active UV map
    /// (`uv_layers.active`), else the active-render one. Workbench TEXTURE samples it
    /// (M5+M6 W2, WP 5.5).
    pub uv_active: Option<usize>,
    /// Index into [`RMesh::colors`] of the colour Workbench VERTEX draws
    /// (`mesh_has_color_attribute`: `color_attributes.active_color`, else the default
    /// colour; `None` when that name is no colour attribute). WP 5.5.
    pub color_active: Option<usize>,
    /// Index into [`RMesh::colors`] of the default (render) colour attribute
    /// (`color_attributes.default_color_name`), which a Color Attribute node with an empty
    /// name reads (`GPU_attribute_default_color`). WP 5.5.
    pub color_default: Option<usize>,
}

/// Light type with its shape parameters (Blender units preserved).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum RLightKind {
    /// POINT
    Point,
    /// SUN (`energy` in W/m²)
    Sun {
        /// `angle`
        angle: f32,
    },
    /// SPOT
    Spot {
        /// `spot_size`
        size: f32,
        /// `spot_blend`
        blend: f32,
    },
    /// AREA
    Area {
        /// `shape`
        shape: SmolStr,
        /// `(size, size_y)`
        size: [f32; 2],
    },
}

/// One light.
#[derive(Clone, Debug, PartialEq)]
pub struct RLight {
    /// Type.
    pub kind: RLightKind,
    /// `color`
    pub color: [f32; 3],
    /// `energy` (W, or W/m² for sun)
    pub energy: f32,
    /// `shadow_soft_size`
    pub radius: f32,
    /// `use_shadow`
    pub use_shadow: bool,
    /// `matrix_world`
    pub matrix_world: Mat4,
    /// The EEVEE-only light settings (M5+M6 W0 contract delta, read by WP 5.1).
    pub eevee: RLightEevee,
}

/// `Light` settings the EEVEE-like engine reads beyond [`RLight`]'s (Blender units; defaults
/// are `lights.new(…, 'POINT')`'s). M5+M6 W0 contract delta: the builder fills every field,
/// WP 5.1 (`frap-render/src/eevee/lights.rs`, `shadows.rs`) interprets them.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct RLightEevee {
    /// `use_temperature` (the colour is then `color × blackbody(temperature)`).
    pub use_temperature: bool,
    /// `temperature` (K)
    pub temperature: f32,
    /// `diffuse_factor`
    pub diffuse_factor: f32,
    /// `specular_factor`
    pub specular_factor: f32,
    /// `transmission_factor`
    pub transmission_factor: f32,
    /// `volume_factor`
    pub volume_factor: f32,
    /// `use_soft_falloff` (point and spot lights)
    pub use_soft_falloff: bool,
    /// `normalize` (area lights: energy independent of the size)
    pub normalize: bool,
    /// `use_custom_distance` / `cutoff_distance`
    pub cutoff_distance: Option<f32>,
    /// `shadow_filter_radius`
    pub shadow_filter_radius: f32,
    /// `shadow_maximum_resolution`
    pub shadow_maximum_resolution: f32,
    /// `use_absolute_resolution`
    pub use_absolute_resolution: bool,
    /// `use_shadow_jitter`
    pub use_shadow_jitter: bool,
    /// `shadow_jitter_overblur` (percent)
    pub shadow_jitter_overblur: f32,
    /// `spread` (area lights, radians)
    pub spread: f32,
}

impl Default for RLightEevee {
    fn default() -> Self {
        RLightEevee {
            use_temperature: false,
            temperature: 6500.0,
            diffuse_factor: 1.0,
            specular_factor: 1.0,
            transmission_factor: 1.0,
            volume_factor: 1.0,
            use_soft_falloff: true,
            normalize: true,
            cutoff_distance: None,
            shadow_filter_radius: 1.0,
            shadow_maximum_resolution: 0.001,
            use_absolute_resolution: false,
            use_shadow_jitter: false,
            shadow_jitter_overblur: 10.0,
            spread: std::f32::consts::PI,
        }
    }
}

/// World background.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum RWorld {
    /// Background colour × strength.
    Color {
        /// RGB (linear)
        rgb: [f32; 3],
        /// Strength
        strength: f32,
    },
    /// Environment texture.
    Hdri {
        /// The image.
        image: ImageKey,
        /// Z rotation (radians).
        rotation: f32,
        /// Strength
        strength: f32,
    },
}

impl Default for RWorld {
    fn default() -> Self {
        RWorld::Color {
            rgb: [0.050876, 0.050876, 0.050876],
            strength: 1.0,
        }
    }
}

/// Render settings the engines read.
#[derive(Clone, Debug, PartialEq)]
pub struct RSettings {
    /// `render.engine`
    pub engine: SmolStr,
    /// `eevee.taa_render_samples`
    pub taa_render_samples: u32,
    /// `eevee.use_shadows`
    pub use_shadows: bool,
    /// `render.film_transparent`
    pub film_transparent: bool,
    /// `display.shading.light` (Workbench)
    pub shading_light: SmolStr,
    /// `display.shading.color_type` (Workbench)
    pub shading_color_type: SmolStr,
    /// `display.shading.single_color`
    pub shading_single_color: [f32; 3],
    /// `view_settings.view_transform`
    pub view_transform: SmolStr,
    /// `view_settings.look`
    pub look: SmolStr,
    /// `view_settings.exposure`
    pub exposure: f32,
    /// `view_settings.gamma`
    pub gamma: f32,
    /// Frame being rendered.
    pub frame: i32,
    /// Workbench MATERIAL colour source of each material: its viewport display values, which
    /// Blender's Workbench reads instead of the node tree (decision log W1-15). A key missing
    /// here (synthetic scenes) falls back to the lowered material's factors.
    pub workbench_materials: IndexMap<MatKey, RWorkbenchMaterial>,
    /// Workbench render background (straight RGBA) as Blender clears to it when
    /// `film_transparent` is off: `World.color` with alpha 1, `display.shading.background_color`
    /// for `background_type = 'VIEWPORT'`, transparent black without a world (verified on
    /// 5.2.4). `None` (synthetic scenes): the colour of [`RenderScene::world`].
    pub workbench_background: Option<[f32; 4]>,
    /// The rest of `scene.display` / `scene.display.shading` the complete Workbench reads
    /// (matcap, X-ray, wireframe, outline, shadows, cavity; M2 W0 contract delta, WP 2.3).
    pub workbench: RWorkbenchShading,
    /// `scene.eevee` and the film settings the EEVEE-like engine reads (M5+M6 W0 contract
    /// delta, WP 5.1).
    pub eevee: REeveeSettings,
    /// What the engines read from each material besides its `LoweredMaterial` (M5+M6 W2,
    /// WP 5.5): EEVEE's render method and refraction settings, the Workbench TEXTURE image.
    /// A key missing here (synthetic scenes) reads [`RMaterialSettings::default`].
    pub material_settings: IndexMap<MatKey, RMaterialSettings>,
}

/// Per-material settings of the `Material` datablock and its node tree that the
/// glTF-shaped `LoweredMaterial` does not carry (WP 5.5). Defaults: a new material's
/// (verified on 5.2.4).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct RMaterialSettings {
    /// `Material.surface_render_method`: `'DITHERED'` (hashed transparency) or `'BLENDED'`
    /// (sorted, blended after the opaque surfaces).
    pub surface_render_method: SmolStr,
    /// `Material.use_transparency_overlap` (`show_transparent_back`).
    pub use_transparency_overlap: bool,
    /// `Material.use_raytrace_refraction`: with `scene.eevee.use_raytracing`, refraction
    /// traces the opaque surfaces behind; otherwise it samples the light probes (the world).
    pub use_raytrace_refraction: bool,
    /// The Workbench TEXTURE colour source: the image of the material's active texture
    /// node (`ED_object_get_active_image`), `None` when that node is no Image or Environment
    /// Texture with an image.
    pub workbench_texture: Option<RWorkbenchTexture>,
}

impl Default for RMaterialSettings {
    fn default() -> Self {
        RMaterialSettings {
            surface_render_method: SmolStr::new_static("DITHERED"),
            use_transparency_overlap: true,
            use_raytrace_refraction: false,
            workbench_texture: None,
        }
    }
}

impl RMaterialSettings {
    /// Settings with the given render method, the rest at their defaults.
    pub fn with_render_method(method: &str) -> Self {
        RMaterialSettings {
            surface_render_method: SmolStr::new(method),
            ..RMaterialSettings::default()
        }
    }
}

/// The image Workbench TEXTURE draws for a material (`MaterialTexture`,
/// `workbench_materials.cc`) and how it samples it.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct RWorkbenchTexture {
    /// The image ([`RenderScene::images`]).
    pub image: ImageKey,
    /// Linear filtering (`interpolation` other than `Closest`).
    pub linear: bool,
    /// Image Texture `extension` (`REPEAT`, `EXTEND`, `MIRROR`, `CLIP`; `EXTEND` for an
    /// Environment Texture).
    pub extension: SmolStr,
    /// Pixels with alpha below 0.1 are discarded (`alpha_mode` other than `NONE` and
    /// `CHANNEL_PACKED`).
    pub alpha_cutoff: bool,
    /// The image's colour space is data (`Non-Color`): byte pixels are not sRGB decoded.
    pub is_data: bool,
}

impl RWorkbenchTexture {
    /// A texture of `image` with the given sampling.
    pub fn new(image: ImageKey, linear: bool, extension: &str, alpha_cutoff: bool) -> Self {
        RWorkbenchTexture {
            image,
            linear,
            extension: SmolStr::new(extension),
            alpha_cutoff,
            is_data: false,
        }
    }
}

/// `scene.eevee` / `scene.render` values of the EEVEE-like engine (§19.3). Defaults are the
/// factory scene's (verified on 5.2.4, `spec/blender-5.2/startup.json`). `taa_render_samples`,
/// `use_shadows` and `film_transparent` stay in [`RSettings`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct REeveeSettings {
    /// `eevee.shadow_ray_count`
    pub shadow_ray_count: u32,
    /// `eevee.shadow_step_count`
    pub shadow_step_count: u32,
    /// `eevee.shadow_resolution_scale`
    pub shadow_resolution_scale: f32,
    /// `eevee.light_threshold`
    pub light_threshold: f32,
    /// `eevee.clamp_surface_direct` (0 = off)
    pub clamp_surface_direct: f32,
    /// `eevee.clamp_surface_indirect` (0 = off)
    pub clamp_surface_indirect: f32,
    /// `eevee.use_raytracing`
    pub use_raytracing: bool,
    /// `eevee.use_fast_gi`
    pub use_fast_gi: bool,
    /// `render.filter_size` (pixels, Blackman-Harris width of the film filter)
    pub filter_size: f32,
    /// `render.dither_intensity` (applied by the output, kept here for the film)
    pub dither_intensity: f32,
}

impl Default for REeveeSettings {
    fn default() -> Self {
        REeveeSettings {
            shadow_ray_count: 1,
            shadow_step_count: 6,
            shadow_resolution_scale: 1.0,
            light_threshold: 0.01,
            clamp_surface_direct: 0.0,
            clamp_surface_indirect: 10.0,
            use_raytracing: false,
            use_fast_gi: false,
            filter_size: 1.5,
            dither_intensity: 1.0,
        }
    }
}

/// `scene.display` and `scene.display.shading` values of the complete Workbench engine
/// (§19.3; M2 W0, implemented by WP 2.3). Defaults are the factory scene's (verified on 5.2.4,
/// `spec/blender-5.2/startup.json`). `shading_light`, `shading_color_type` and
/// `shading_single_color` stay in [`RSettings`].
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct RWorkbenchShading {
    /// `shading.type` (`SOLID`, `WIREFRAME`; `MATERIAL`/`RENDERED` render as `SOLID`).
    pub shading_type: SmolStr,
    /// `shading.studio_light`: the studio light, or the matcap when `light == 'MATCAP'`.
    pub studio_light: SmolStr,
    /// `shading.studiolight_rotate_z`
    pub studiolight_rotate_z: f32,
    /// `shading.use_world_space_lighting`
    pub use_world_space_lighting: bool,
    /// `shading.show_backface_culling`
    pub show_backface_culling: bool,
    /// `shading.show_xray`
    pub show_xray: bool,
    /// `shading.xray_alpha`
    pub xray_alpha: f32,
    /// `shading.show_xray_wireframe`
    pub show_xray_wireframe: bool,
    /// `shading.xray_alpha_wireframe`
    pub xray_alpha_wireframe: f32,
    /// `shading.wireframe_color_type`
    pub wireframe_color_type: SmolStr,
    /// `shading.show_object_outline`
    pub show_object_outline: bool,
    /// `shading.object_outline_color` (linear RGB)
    pub object_outline_color: [f32; 3],
    /// `shading.show_shadows`
    pub show_shadows: bool,
    /// `shading.shadow_intensity`
    pub shadow_intensity: f32,
    /// `display.light_direction`
    pub light_direction: [f32; 3],
    /// `display.shadow_shift`
    pub shadow_shift: f32,
    /// `display.shadow_focus`
    pub shadow_focus: f32,
    /// `shading.show_cavity`
    pub show_cavity: bool,
    /// `shading.cavity_type` (`WORLD`, `SCREEN`, `BOTH`)
    pub cavity_type: SmolStr,
    /// `shading.cavity_ridge_factor`
    pub cavity_ridge_factor: f32,
    /// `shading.cavity_valley_factor`
    pub cavity_valley_factor: f32,
    /// `shading.curvature_ridge_factor`
    pub curvature_ridge_factor: f32,
    /// `shading.curvature_valley_factor`
    pub curvature_valley_factor: f32,
    /// `display.matcap_ssao_distance`
    pub matcap_ssao_distance: f32,
    /// `display.matcap_ssao_attenuation`
    pub matcap_ssao_attenuation: f32,
    /// `display.matcap_ssao_samples`
    pub matcap_ssao_samples: i32,
    /// `shading.show_specular_highlight`
    pub show_specular_highlight: bool,
    /// `shading.use_dof`
    pub use_dof: bool,
    /// `display.render_aa` (`OFF`, `FXAA`, `5`, `8`, `11`, `16`, `32`)
    pub render_aa: SmolStr,
}

impl Default for RWorkbenchShading {
    /// The factory scene's `scene.display` (verified on 5.2.4).
    fn default() -> Self {
        RWorkbenchShading {
            shading_type: "SOLID".into(),
            studio_light: "Default".into(),
            studiolight_rotate_z: 0.0,
            use_world_space_lighting: false,
            show_backface_culling: false,
            show_xray: false,
            xray_alpha: 0.5,
            show_xray_wireframe: false,
            xray_alpha_wireframe: 0.0,
            wireframe_color_type: SmolStr::default(),
            show_object_outline: false,
            object_outline_color: [0.0, 0.0, 0.0],
            show_shadows: false,
            shadow_intensity: 0.5,
            light_direction: [0.577_350_26, 0.577_350_26, 0.577_350_26],
            shadow_shift: 0.1,
            shadow_focus: 0.0,
            show_cavity: false,
            cavity_type: "WORLD".into(),
            cavity_ridge_factor: 1.0,
            cavity_valley_factor: 1.0,
            curvature_ridge_factor: 0.0,
            curvature_valley_factor: 0.0,
            matcap_ssao_distance: 0.2,
            matcap_ssao_attenuation: 1.0,
            matcap_ssao_samples: 16,
            show_specular_highlight: true,
            use_dof: false,
            render_aa: "8".into(),
        }
    }
}

/// How `frap.render_contact_sheet` lays its tiles out on the sheet (§19.6; M2 W0 contract
/// delta, implemented by WP 2.3). Pixel rectangles are `[x, y, width, height]`, top row first.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SheetLayout {
    /// Sheet size in pixels.
    pub size: [u32; 2],
    /// Background (straight RGBA, scene-linear).
    pub background: [f32; 4],
    /// One tile per rendered film, in the order of the films.
    pub tiles: Vec<SheetTile>,
}

/// One contact-sheet tile.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SheetTile {
    /// `[x, y, width, height]` on the sheet.
    pub rect: [u32; 4],
    /// Label drawn with the embedded bitmap font (`show_labels`), e.g. `FRONT`.
    pub label: Option<SmolStr>,
}

impl SheetLayout {
    /// A layout from its parts (the struct is `#[non_exhaustive]`).
    pub fn new(size: [u32; 2], background: [f32; 4], tiles: Vec<SheetTile>) -> Self {
        SheetLayout {
            size,
            background,
            tiles,
        }
    }
}

impl SheetTile {
    /// A tile from its parts.
    pub fn new(rect: [u32; 4], label: Option<SmolStr>) -> Self {
        SheetTile { rect, label }
    }
}

/// A material's viewport display values (`Material.diffuse_color`, `.metallic`, `.roughness`),
/// the Workbench MATERIAL colour source.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RWorkbenchMaterial {
    /// `Material.diffuse_color` (linear RGBA)
    pub diffuse_color: [f32; 4],
    /// `Material.metallic`
    pub metallic: f32,
    /// `Material.roughness`
    pub roughness: f32,
}

impl Default for RSettings {
    fn default() -> Self {
        RSettings {
            engine: "BLENDER_WORKBENCH".into(),
            taa_render_samples: 64,
            use_shadows: true,
            film_transparent: false,
            shading_light: "STUDIO".into(),
            shading_color_type: "MATERIAL".into(),
            shading_single_color: [0.8, 0.8, 0.8],
            view_transform: "AgX".into(),
            look: "None".into(),
            exposure: 0.0,
            gamma: 1.0,
            frame: 1,
            workbench_materials: IndexMap::new(),
            workbench_background: None,
            workbench: RWorkbenchShading::default(),
            eevee: REeveeSettings::default(),
            material_settings: IndexMap::new(),
        }
    }
}

/// Everything an engine needs, as plain data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderScene {
    /// `resolution_x/y × percentage`
    pub resolution: [u32; 2],
    /// Camera.
    pub camera: RCamera,
    /// Objects, sorted by name.
    pub objects: Vec<RObject>,
    /// Meshes by evaluated fingerprint (GPU cache key).
    pub meshes: IndexMap<MeshKey, Arc<RMesh>>,
    /// Lowered materials.
    pub materials: IndexMap<MatKey, Arc<LoweredMaterial>>,
    /// Decoded images.
    pub images: IndexMap<ImageKey, Arc<DecodedImage>>,
    /// Lights.
    pub lights: Vec<RLight>,
    /// World.
    pub world: RWorld,
    /// Settings.
    pub settings: RSettings,
    /// The [`ImageKey`] of every image in [`RenderScene::images`] by datablock (`TexRef::image`
    /// of the lowered materials, the world's environment image), so engines can resolve a
    /// texture reference (M5+M6 W0 contract delta). See [`RenderScene::image`].
    pub image_ids: IndexMap<IdKey, ImageKey>,
}

impl RenderScene {
    /// The decoded image a material texture (or the world) references, if it was collected.
    pub fn image(&self, image: IdRef<ImageId>) -> Option<&Arc<DecodedImage>> {
        self.image_ids
            .get(&image.key())
            .and_then(|k| self.images.get(k))
    }
}

/// Rendered pixels: scene-linear, straight alpha, top row first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Film {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
    /// `width × height` pixels.
    pub rgba: Vec<[f32; 4]>,
}

/// GPU adapter summary (`--describe`, logs, CI adapter assertions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdapterSummary {
    /// Adapter name.
    pub name: String,
    /// Backend ("Metal", "Vulkan", "Dx12").
    pub backend: String,
    /// Device type.
    pub device_type: String,
    /// Driver and driver info.
    pub driver: String,
}

/// Render failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderError {
    /// No usable adapter (`GPU_UNAVAILABLE`).
    NoAdapter(String),
    /// Device creation or device loss.
    Device(String),
    /// Readback failure.
    Readback(String),
    /// A scene feature the engine cannot render.
    Unsupported(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::NoAdapter(m) => write!(f, "no GPU adapter: {m}"),
            RenderError::Device(m) => write!(f, "GPU device error: {m}"),
            RenderError::Readback(m) => write!(f, "GPU readback error: {m}"),
            RenderError::Unsupported(m) => write!(f, "unsupported: {m}"),
        }
    }
}
impl std::error::Error for RenderError {}

/// The only GPU-facing trait the rest of frap sees; implemented by `frap-render`.
pub trait RenderService: Send {
    /// Renders one scene.
    fn render(&mut self, scene: &RenderScene) -> Result<Film, RenderError>;
    /// Renders several scenes (contact-sheet tiles).
    fn render_tiles(&mut self, scenes: &[RenderScene]) -> Result<Vec<Film>, RenderError>;
    /// Composites rendered tiles into one contact sheet (§19.6, CPU, labels from the embedded
    /// bitmap font). M2 W0 contract delta (additive, default body), implemented by
    /// `frap-render`'s `composite` module (WP 2.3).
    fn composite_sheet(
        &mut self,
        films: &[Film],
        layout: &SheetLayout,
    ) -> Result<Film, RenderError> {
        let _ = (films, layout);
        Err(RenderError::Unsupported(
            "contact-sheet compositing is not available in this render service".into(),
        ))
    }
    /// The adapter in use.
    fn adapter_info(&self) -> AdapterSummary;
    /// Whether this service renders `engine` (`render.engine`) itself rather than falling back to
    /// Workbench (M5+M6 W0 contract delta, additive): `BLENDER_WORKBENCH` always;
    /// `BLENDER_EEVEE` once WP 5.1 enables its engine (`frap-render`'s
    /// `eevee::pipeline::IMPLEMENTED`). `render.render` and `frap.render_contact_sheet` ask
    /// before choosing the engine and its WARNING.
    fn supports_engine(&self, engine: &str) -> bool {
        engine == "BLENDER_WORKBENCH"
    }
}

/// Backend selection (§19.2): `FRAP_WGPU_BACKEND` (`metal|vulkan|dx12|gl`), `FRAP_ADAPTER`,
/// `FRAP_FORCE_FALLBACK=1`. Here (L8) so a build without the `gpu` feature can still name the
/// session's render factory type (F-REN-2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GpuOptions {
    /// Backend override.
    pub backend: Option<String>,
    /// Adapter name substring.
    pub adapter: Option<String>,
    /// Ask for a software adapter.
    pub force_fallback: bool,
}

impl GpuOptions {
    /// Reads the `FRAP_*` environment variables.
    pub fn from_env() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|s| !s.is_empty());
        GpuOptions {
            backend: var("FRAP_WGPU_BACKEND"),
            adapter: var("FRAP_ADAPTER"),
            force_fallback: var("FRAP_FORCE_FALLBACK").is_some_and(|v| v == "1"),
        }
    }
}
