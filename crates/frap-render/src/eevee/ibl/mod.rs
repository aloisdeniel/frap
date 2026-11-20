// SPDX-License-Identifier: GPL-3.0-or-later
//! World lighting from an HDRI (`RWorld::Hdri`, WP 5.2): EEVEE's world sphere probe, ported
//! from Blender 5.2 (blender/source/blender/draw/engines/eevee/eevee_lightprobe_sphere.cc,
//! eevee_lightprobe.cc, eevee_light.cc `add_world_sun_light`, and the shaders
//! eevee_lightprobe_sphere_bake.bsl.hh, eevee_lightprobe_sphere.bsl.hh,
//! eevee_lightprobe.bsl.hh, eevee_spherical_harmonics.bsl.hh):
//!
//! - **Diffuse**: L1 spherical harmonics of the world radiance, deringed
//!   ([`WorldLighting::sh9`], [`sh`]). EEVEE without irradiance volumes lights every diffuse
//!   closure with `evaluate_lambert(N)` of the world's harmonics (`ibl_sh_lambert` in
//!   [`WGSL`]).
//! - **Specular**: the world radiance prefiltered per roughness into 5 mips
//!   ([`WorldLighting::specular`]); EEVEE samples it at `roughness_to_lod(roughness)` in the
//!   GGX dominant direction and fades to the harmonics between roughness 0.7 and 0.9
//!   (`roughness_to_mix_fac`). The closure colour (split-sum Fresnel) comes from the BRDF LUT
//!   ([`WorldLighting::brdf_lut`], [`lut`]).
//! - **Sun**: radiance brighter than `World.sun_threshold` is removed from the probe and
//!   becomes a sun light ([`WorldLighting::sun`]) that the engine lights and shadows like any
//!   other sun.
//! - **Background**: the environment × strength ([`WorldLighting::background`]), looked up
//!   through Blender's equirectangular mapping after the Mapping node's Z rotation
//!   (`ibl_equirect_tex_coord(direction, rotation)`).
//!
//! The forward shader (WP 5.1) prepends [`WGSL`] and evaluates, per closure:
//!
//! ```text
//! diffuse   += diffuse_color * ibl_sh_lambert(sh9[0..4], N)
//! R          = ibl_dominant_direction_reflection(N, V, roughness)
//! probe      = textureSampleLevel(specular, sampler, R, ibl_roughness_to_lod(roughness)).rgb
//! specular  += reflection_color * mix(probe, ibl_sh_lambert(sh9[0..4], R),
//!                                     ibl_roughness_to_mix_fac(roughness))
//! ```
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

/// WGSL helpers for consumers of [`WorldLighting`] (no bindings): the equirectangular lookup,
/// the harmonics' Lambert evaluation, roughness → probe mip and SH mix factor, the GGX
/// dominant direction, and the probe-bake helpers (octahedral mapping, `sqrt_fast`, …).
pub const WGSL: &str = include_str!("shaders/common.wgsl");

/// Split-sum material helpers in WGSL (`ibl_brdf_lut`, `ibl_principled_*`, …) over the LUT
/// textures of [`WorldLighting`]; requires [`WGSL`] before it.
pub const SHADING_WGSL: &str = include_str!("shaders/shading.wgsl");

/// The HDRI world as the forward shader samples it.
#[non_exhaustive]
pub struct WorldLighting {
    /// Diffuse world lighting: Blender's L1 spherical harmonics of the world radiance (after
    /// the sun extraction and EEVEE's `dering`), RGB in `xyz`. Entries 0–3 are `L0.M0`,
    /// `L1.Mn1`, `L1.M0`, `L1.Mp1` (Blender's basis with the Condon-Shortley phase:
    /// `Y1-1 = -0.4886 y`, `Y10 = 0.4886 z`, `Y11 = -0.4886 x`); the L2 band (4–8) is zero,
    /// as EEVEE stores only L1. Evaluate with `ibl_sh_lambert` ([`WGSL`]), which returns the
    /// Lambertian radiance (irradiance / π) for unit albedo.
    pub sh9: [[f32; 4]; 9],
    /// Radiance prefiltered per roughness (`Rgba16Float` cube map, face size
    /// `probe_resolution / 2`, [`WorldLighting::specular_mips`] mips; mip `m` is convolved for
    /// linear roughness `octahedral::lod_to_roughness(m)`). Sample at
    /// `ibl_roughness_to_lod(roughness)` with a trilinear sampler.
    pub specular: wgpu::TextureView,
    /// Number of mips of [`WorldLighting::specular`] (`SPHERE_PROBE_MIPMAP_LEVELS` = 5).
    pub specular_mips: u32,
    /// The split-sum GGX BRDF LUT (64 × 64 `Rgba16Float`: `scale`, `bias`, `metal_bias`; the
    /// layer `UTIL_BRDF_LAYER` of EEVEE's utility texture, from `assets/brdf_lut.bin`). Look
    /// up at `(roughness, sqrt(1 - cos_theta)) * 63/64 + 0.5/64`.
    pub brdf_lut: wgpu::TextureView,
    /// The environment as the background shows it (equirectangular, `Rgba16Float`, top row
    /// first, already × the Background strength). Look up at
    /// `ibl_equirect_tex_coord(direction, rotation)` with `u` repeating.
    pub background: wgpu::TextureView,
    /// The 16 GGX BSDF layers of EEVEE's utility texture (`Rgba16Float` 2D array: `bsdf_ggx`
    /// in RGB, `btdf_ggx` in A), for dielectric reflectance and transmission
    /// (`ibl_btdf_gt1`, [`SHADING_WGSL`]).
    pub bsdf_lut: wgpu::TextureView,
    /// Mapping node Z rotation of the environment lookup (radians).
    pub rotation: f32,
    /// The world sun extracted above `World.sun_threshold` (a `SUN` light: colour × energy is
    /// its irradiance, `matrix_world`'s Z axis points at the sun, the World's sun angle and
    /// shadow settings), or `None` when nothing passed the threshold.
    pub sun: Option<RLight>,
    /// A trilinear, clamped sampler for [`WorldLighting::specular`] and the LUTs.
    pub sampler: wgpu::Sampler,
    /// The background's sampler (linear, `u` repeating, `v` clamped).
    pub background_sampler: wgpu::Sampler,
    /// The octahedral probe the cube map was resampled from (diagnostics and tests).
    #[allow(dead_code)]
    pub(crate) octahedral: wgpu::Texture,
}

/// Builds the world lighting of `scene` when its world is an HDRI (`Ok(None)` otherwise).
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
    let decoded = scene.images.get(image);
    let linear = match decoded {
        Some(img) => equirect::LinearImage::from_decoded(img, *strength),
        // Blender's missing-texture magenta.
        None => {
            let m = strength.clamp(-equirect::HALF_MAX, equirect::HALF_MAX);
            equirect::LinearImage {
                width: 1,
                height: 1,
                rgb: vec![[m, 0.0, m]],
            }
        }
    };
    let max_side = gpu.device.limits().max_texture_dimension_2d;
    build(gpu, &linear.fit(max_side), *rotation, &settings).map(Some)
}

/// [`world_lighting`] for an already decoded environment (`image` × strength).
pub fn build(
    gpu: &Gpu,
    image: &equirect::LinearImage,
    rotation: f32,
    settings: &WorldProbeSettings,
) -> Result<WorldLighting, RenderError> {
    let baked = bake::bake(gpu, image, rotation, settings)?;
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
        sh9: baked.sh.to_sh9(),
        specular,
        specular_mips: octahedral::MIPMAP_LEVELS,
        brdf_lut,
        background: baked.background.create_view(&Default::default()),
        bsdf_lut,
        rotation,
        sun: baked.sun,
        sampler: bake::clamp_sampler(gpu),
        background_sampler: bake::background_sampler(gpu),
        octahedral: baked.octahedral,
    })
}

impl WorldLighting {
    /// The diffuse harmonics as [`sh::ShL1`].
    pub fn sh(&self) -> sh::ShL1 {
        let c = |k: usize| [self.sh9[k][0], self.sh9[k][1], self.sh9[k][2]];
        sh::ShL1::from_coefs([c(0), c(1), c(2), c(3)])
    }
}
