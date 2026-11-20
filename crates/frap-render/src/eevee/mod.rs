// SPDX-License-Identifier: GPL-3.0-or-later
//! The EEVEE-like engine (`BLENDER_EEVEE`, ARCHITECTURE §19.3; M5): forward PBR (GGX + Lambert)
//! on `LoweredMaterial`, shadows, world lighting and IBL, transparency, TAA.
//!
//! CONTRACT (M5+M6 W0, this file is the integrator's): the module layout, [`Eevee`] (the
//! `Engine` the render service dispatches `BLENDER_EEVEE` and `CYCLES` to once
//! [`pipeline::IMPLEMENTED`] is true), the shading-feature extension point
//! [`EEVEE_FEATURES`] and the split between the files:
//!
//! | File | Owner | Content |
//! |---|---|---|
//! | `pipeline.rs` | WP 5.1 | passes, the uber-shader pipelines, [`pipeline::MaterialParams`], `IMPLEMENTED` |
//! | `lights.rs` | WP 5.1 | `RLight` → GPU light records, EEVEE's power conversions |
//! | `shadows.rs` | WP 5.1 | cube (point), 2D (spot), cascaded (sun) shadow maps, PCF by `shadow_soft_size` |
//! | `taa.rs` | WP 5.1 | `taa_render_samples` with a fixed Halton sequence (also soft shadows and AA) |
//! | `film.rs` | WP 5.1 | `Rgba32Float` accumulation, `filter_size`, `film_transparent`, readback |
//! | `ibl/**` | WP 5.2 | HDRI world: irradiance SH, prefiltered GGX mips, BRDF LUT ([`ibl::world_lighting`]) |
//! | `features/*.rs` | WP 5.5 | one file per shading feature, each registering an [`EeveeFeature`] |
//! | `../shaders/eevee_*.wgsl` | WP 5.1 | the forward, shadow and world uber-shaders |
//!
//! World lighting: WP 5.1 lights a constant-colour world (`RWorld::Color`) itself (uniform
//! radiance: irradiance `π·L`, prefiltered radiance `L`); an HDRI (`RWorld::Hdri`) comes from
//! [`ibl::world_lighting`] (WP 5.2), so the two land independently in Wave 1.
//!
//! GPU rule (§22.6 platform rules): every GPU test runs on Metal, lavapipe and WARP and skips
//! with a notice under `FRAP_GPU_OPTIONAL=1` when no adapter exists. Shaders must not rely on
//! the driver folding pack/unpack round trips (Mesa's NIR does; see the integer-mask
//! `rtz16` of `workbench.wgsl`).

pub mod film;
pub mod ibl;
pub mod lights;
pub mod pipeline;
pub mod shadows;
pub mod taa;

/// Shading features (WP 5.5): one file per feature (normal maps, alpha blend/dither,
/// transmission, clearcoat, sheen, emission, vertex colours), each registering an
/// [`EeveeFeature`]. Included with `automod`, so adding a file needs no `mod` edit.
pub mod features {
    automod::dir!(pub "src/eevee/features");
}

use frap_material::LoweredMaterial;
use frap_render_scene::{Film, RenderError, RenderScene};

use crate::engine::{Engine, GpuCache};
use crate::gpu::Gpu;

/// One material feature of the forward uber-shader (§19.2: features are uniform flags, never
/// pipeline permutations). WP 5.1's pipeline builds the shader from its own source plus every
/// registered feature's `wgsl` (in `order`), and packs each material's parameters by calling
/// `pack` for the features that apply.
#[derive(Debug)]
pub struct EeveeFeature {
    /// Feature name (`"normal_map"`, `"clearcoat"`, …).
    pub name: &'static str,
    /// Sort key of the WGSL snippets and of `pack` calls.
    pub order: u16,
    /// WGSL functions appended to the forward shader (the entry points call them under the
    /// feature's flag; the call sites and the flag bits are WP 5.1's
    /// [`pipeline::MaterialParams`] layout).
    pub wgsl: &'static str,
    /// Whether `material` uses the feature (its flag bit is set).
    pub applies: fn(&LoweredMaterial) -> bool,
    /// Writes the feature's values (factors, texture slots) into the material's parameters.
    pub pack: fn(&LoweredMaterial, &RenderScene, &mut pipeline::MaterialParams),
}

/// Every registered shading feature.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static EEVEE_FEATURES: [EeveeFeature];

/// The registered features sorted by `order`, then name.
pub fn features() -> Vec<&'static EeveeFeature> {
    let mut v: Vec<&'static EeveeFeature> = EEVEE_FEATURES.iter().collect();
    v.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(b.name)));
    v
}

/// The engine: pipelines and per-engine GPU resources, created on the first render and kept
/// for the service's lifetime (one device per process, §19.2).
#[derive(Default)]
pub struct Eevee {
    /// WP 5.1's state (pipelines, shadow atlases, the world probe of the last render).
    pub(crate) state: pipeline::State,
}

impl Eevee {
    /// A new engine (no GPU work until the first render).
    pub fn new() -> Eevee {
        Eevee::default()
    }
}

impl Engine for Eevee {
    fn render(
        &mut self,
        gpu: &Gpu,
        cache: &mut GpuCache,
        scene: &RenderScene,
    ) -> Result<Film, RenderError> {
        pipeline::render(&mut self.state, gpu, cache, scene)
    }
}

/// Link anchor so the `features/` registrations survive LTO (referenced by `Renderer::new`).
#[inline(never)]
pub fn link() {
    let _ = EEVEE_FEATURES.len();
}
