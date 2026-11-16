// SPDX-License-Identifier: GPL-3.0-or-later
//! Render-scene types (ARCHITECTURE §19.2, CONTRACT frozen at M1 W0; `GpuOptions` and
//! `RenderRequest` defined here by RFC 0001 F-REN-1/F-REN-2). Plain data, no wgpu: render tests
//! build synthetic `RenderScene`s.

use std::ops::Range;
use std::sync::Arc;

use frap_imbuf::DecodedImage;
use frap_material::LoweredMaterial;
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
    /// The adapter in use.
    fn adapter_info(&self) -> AdapterSummary;
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
