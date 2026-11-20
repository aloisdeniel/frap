// SPDX-License-Identifier: GPL-3.0-or-later
//! World lighting from an HDRI (`RWorld::Hdri`, WP 5.2): EEVEE's world sphere probe, ported
//! from Blender 5.2 (blender/source/blender/draw/engines/eevee/eevee_lightprobe_sphere.cc,
//! eevee_lightprobe.cc, eevee_light.cc `add_world_sun_light`, and the shaders
//! eevee_lightprobe_sphere_bake.bsl.hh, eevee_lightprobe_sphere.bsl.hh,
//! eevee_lightprobe.bsl.hh, eevee_spherical_harmonics.bsl.hh).
//!
//! # Contract (what each field holds and how a consumer evaluates it)
//!
//! `rotation` is `RWorld::Hdri::rotation`, the Mapping node's Z rotation: a world direction
//! `d` looks the environment up at `Rz(rotation) · d` (`ibl_world_lookup`, the Mapping node's
//! `POINT` transform). `strength` is the Background strength (`RWorld::Hdri::strength`, NaN
//! read as 0).
//!
//! | Field | Frame | Strength | Consumer formula |
//! |---|---|---|---|
//! | [`WorldLighting::sh9`] | world (rotation applied) | applied | `E(N)` = Ramamoorthi & Hanrahan's L2 irradiance formula (`ibl_sh9_irradiance`, [`sh::sh9_irradiance`]); diffuse radiance `diffuse_color · E(N) / π` |
//! | [`WorldLighting::specular`] | environment | **not** applied | `strength · textureSampleLevel(specular, ibl_world_lookup(R, rotation), lod)` |
//! | [`WorldLighting::background`] | environment | **not** applied | `strength · textureSampleLevel(background, ibl_equirect_tex_coord(dir, rotation), 0)` |
//! | [`WorldLighting::sun`] | world | applied | one more `SUN` light, lit and shadowed like the scene's own |
//!
//! `sh9` uses the conventional real SH basis (no Condon-Shortley phase): entries 0–3 are
//! `L00`, `L1-1` (`y`), `L10` (`z`), `L11` (`x`); band 2 (`xy, yz, 3z²-1, xz, x²-y²`) is zero
//! because EEVEE keeps only L1, so `E(N) / π` is exactly EEVEE's `evaluate_lambert(N)` of the
//! deringed world harmonics.
//!
//! The forward shader (WP 5.1) prepends [`WGSL`] (and [`SHADING_WGSL`] for the split-sum
//! helpers) and evaluates, per closure, as EEVEE does (`ibl_world_radiance`):
//!
//! ```text
//! diffuse   += diffuse_color * ibl_sh_lambert(sh9, N)          // = E(N) / π
//! R          = ibl_dominant_direction_reflection(N, V, roughness)
//! probe      = strength * textureSampleLevel(specular, sampler,
//!                  ibl_world_lookup(R, rotation), ibl_roughness_to_lod(roughness)).rgb
//! specular  += reflection_color * mix(probe, ibl_sh_lambert(sh9, R),
//!                                     ibl_roughness_to_mix_fac(roughness))
//! ```
//!
//! and adds [`WorldLighting::sun`] to its light list: radiance brighter than
//! `World.sun_threshold` is taken out of the probe and the harmonics (as in EEVEE), so a
//! consumer that ignores `sun` loses that light and its shadows.
//!
//! WP 5.1 lights a constant-colour world (`RWorld::Color`) itself; [`world_lighting`] answers
//! `Ok(None)` for it.
//!
//! The World's probe settings (`probe_resolution`, `sun_threshold`, `sun_angle`, sun shadows)
//! are not bound on frap's `World` yet: the bake uses `World()`'s defaults
//! ([`WorldProbeSettings::default`]). `clamp_surface_indirect` comes from the scene.

pub mod equirect;
pub mod lut;
pub mod octahedral;
pub mod sh;

mod bake;
#[cfg(test)]
mod preview;
#[cfg(test)]
mod tests;

pub use bake::WorldProbeSettings;

use frap_render_scene::{RLight, RWorld, RenderError, RenderScene};

use crate::gpu::Gpu;

/// WGSL helpers for consumers of [`WorldLighting`] (no bindings): the environment lookup
/// (`ibl_world_lookup`, `ibl_equirect_tex_coord`), the harmonics' irradiance and Lambert
/// evaluation (`ibl_sh9_irradiance`, `ibl_sh_lambert`), roughness → probe mip and SH mix
/// factor, the GGX dominant direction, and the probe-bake helpers (octahedral mapping,
/// `sqrt_fast`, …).
pub const WGSL: &str = include_str!("shaders/common.wgsl");

/// Split-sum material helpers in WGSL (`ibl_brdf_lut`, `ibl_principled_*`,
/// `ibl_world_radiance`, …) over the LUT textures of [`WorldLighting`]; requires [`WGSL`]
/// before it.
pub const SHADING_WGSL: &str = include_str!("shaders/shading.wgsl");

/// The HDRI world as the forward shader samples it (see the module's contract table).
#[non_exhaustive]
pub struct WorldLighting {
    /// Diffuse world lighting: irradiance spherical harmonics, RGB in `xyz`, in the conventional
    /// real basis (no Condon-Shortley phase; `L00, L1-1 (y), L10 (z), L11 (x)`, then band 2,
    /// zero), **in the world's frame** (the rotation applied) and **× strength**: EEVEE's
    /// deringed L1 harmonics of the world radiance after the sun extraction. Evaluate `E(N)`
    /// with Ramamoorthi & Hanrahan's formula (`ibl_sh9_irradiance`, [`sh::sh9_irradiance`]);
    /// the Lambertian radiance is `E(N) / π` (`ibl_sh_lambert`).
    pub sh9: [[f32; 4]; 9],
    /// Radiance prefiltered per roughness (`Rgba16Float` cube map, face size
    /// `probe_resolution / 2`, [`WorldLighting::specular_mips`] mips; mip `m` is convolved for
    /// linear roughness `octahedral::lod_to_roughness(m)`), **in the environment's frame** and
    /// **at strength 1**: sample at `ibl_world_lookup(R, rotation)` and
    /// `ibl_roughness_to_lod(roughness)` with a trilinear sampler, then multiply by
    /// [`WorldLighting::strength`].
    pub specular: wgpu::TextureView,
    /// Number of mips of [`WorldLighting::specular`] (`SPHERE_PROBE_MIPMAP_LEVELS` = 5).
    pub specular_mips: u32,
    /// The split-sum GGX BRDF LUT (64 × 64 `Rgba16Float`: `scale`, `bias`, `metal_bias`; the
    /// layer `UTIL_BRDF_LAYER` of EEVEE's utility texture, from `assets/brdf_lut.bin`). Look
    /// up at `(roughness, sqrt(1 - cos_theta)) * 63/64 + 0.5/64`.
    pub brdf_lut: wgpu::TextureView,
    /// The environment as the background shows it (equirectangular, `Rgba16Float`, top row
    /// first, **at strength 1**, the environment's frame). Look up at
    /// `ibl_equirect_tex_coord(direction, rotation)` with `u` repeating and multiply by
    /// [`WorldLighting::strength`].
    pub background: wgpu::TextureView,
    /// The 16 GGX BSDF layers of EEVEE's utility texture (`Rgba16Float` 2D array: `bsdf_ggx`
    /// in RGB, `btdf_ggx` in A), for dielectric reflectance and transmission
    /// (`ibl_btdf_gt1`, [`SHADING_WGSL`]).
    pub bsdf_lut: wgpu::TextureView,
    /// Mapping node Z rotation of the environment lookup (radians, `RWorld::Hdri::rotation`):
    /// world direction `d` looks up the environment at `Rz(rotation) · d`.
    pub rotation: f32,
    /// The Background strength (`RWorld::Hdri::strength`; NaN reads as 0, clamped to the
    /// half-float range), which [`WorldLighting::specular`] and
    /// [`WorldLighting::background`] do not include.
    pub strength: f32,
    /// The world sun extracted above `World.sun_threshold` (a `SUN` light in the world's
    /// frame, strength applied: colour × energy is its irradiance, `matrix_world`'s Z axis
    /// points at the sun, the World's sun angle and shadow settings), or `None` when nothing
    /// passed the threshold. The consumer adds it to the scene's lights: its radiance is not
    /// in the probe or the harmonics.
    pub sun: Option<RLight>,
    /// A trilinear, clamped sampler for [`WorldLighting::specular`] and the LUTs.
    pub sampler: wgpu::Sampler,
    /// The background's sampler (linear, `u` repeating, `v` clamped).
    pub background_sampler: wgpu::Sampler,
    /// The octahedral probe the cube map was resampled from (diagnostics and tests).
    #[allow(dead_code)]
    pub(crate) octahedral: wgpu::Texture,
}

/// Builds the world lighting of `scene` when its world is an HDRI (`Ok(None)` otherwise). An
/// image missing from `scene.images`, or an empty one, is Blender's missing-texture magenta.
pub fn world_lighting(
    gpu: &Gpu,
    scene: &RenderScene,
) -> Result<Option<WorldLighting>, RenderError> {
    let RWorld::Hdri {
        image,
        rotation,
        strength,
    } = &scene.world
    else {
        return Ok(None);
    };
    let settings = WorldProbeSettings {
        clamp_surface_indirect: scene.settings.eevee.clamp_surface_indirect,
        ..WorldProbeSettings::default()
    };
    let linear = match scene.images.get(image) {
        Some(img) => equirect::LinearImage::from_decoded(img, 1.0),
        None => equirect::LinearImage::missing(),
    };
    let max_side = gpu.device.limits().max_texture_dimension_2d;
    build(gpu, &linear.fit(max_side), *rotation, *strength, &settings).map(Some)
}

/// [`world_lighting`] for an already decoded environment (`image` at strength 1). An empty or
/// inconsistent image is Blender's missing-texture magenta.
pub fn build(
    gpu: &Gpu,
    image: &equirect::LinearImage,
    rotation: f32,
    strength: f32,
    settings: &WorldProbeSettings,
) -> Result<WorldLighting, RenderError> {
    let missing;
    let image = if image.is_valid() {
        image
    } else {
        missing = equirect::LinearImage::missing();
        &missing
    };
    let strength = if strength.is_nan() {
        0.0
    } else {
        strength.clamp(-equirect::HALF_MAX, equirect::HALF_MAX)
    };
    let rotation = if rotation.is_finite() { rotation } else { 0.0 };
    let baked = bake::bake(gpu, image, rotation, strength, settings)?;
    let lut = lut::upload(gpu);
    let brdf_lut = lut.create_view(&wgpu::TextureViewDescriptor {
        label: Some("frap ibl brdf lut"),
        dimension: Some(wgpu::TextureViewDimension::D2),
        base_array_layer: 0,
        array_layer_count: Some(1),
        ..Default::default()
    });
    let bsdf_lut = lut.create_view(&wgpu::TextureViewDescriptor {
        label: Some("frap ibl bsdf lut"),
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        base_array_layer: 1,
        array_layer_count: Some(lut::BSDF_LAYERS as u32),
        ..Default::default()
    });
    let specular = baked.specular.create_view(&wgpu::TextureViewDescriptor {
        label: Some("frap ibl specular"),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        ..Default::default()
    });
    Ok(WorldLighting {
        sh9: baked.sh.to_sh9(rotation),
        specular,
        specular_mips: octahedral::MIPMAP_LEVELS,
        brdf_lut,
        background: baked.background.create_view(&Default::default()),
        bsdf_lut,
        rotation,
        strength,
        sun: baked.sun,
        sampler: bake::clamp_sampler(gpu),
        background_sampler: bake::background_sampler(gpu),
        octahedral: baked.octahedral,
    })
}

impl WorldLighting {
    /// The diffuse harmonics as [`sh::ShL1`] (Blender's basis, the world's frame, strength
    /// applied).
    pub fn sh(&self) -> sh::ShL1 {
        sh::ShL1::from_sh9(&self.sh9)
    }
}
