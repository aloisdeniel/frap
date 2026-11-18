// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench X-ray and transparency (`shading.show_xray` / `xray_alpha`, materials whose
//! alpha is below 1).
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `workbench_state.cc` (`SceneState::init`): with `shading.type` below SOLID (WIREFRAME) the
//!   X-ray alpha is 0 (see [`super::wire`]); otherwise `SHADING_XRAY_ENABLED` (the X-ray flag
//!   of the shading type, `show_xray` or `show_xray_wireframe`, and its alpha below 1,
//!   `ED_view3d.hh`) gives that alpha, else 1. Any alpha other than 1 is X-ray mode, which
//!   disables shadows, cavity and depth of field.
//! - `workbench_engine.cc` (`draw_to_mesh_pass`): in X-ray mode, or for a material that
//!   `Material::is_transparent` (8-bit packed alpha below 255), a surface goes to the
//!   transparent passes instead of the opaque one.
//! - `workbench_mesh_passes.cc` (`TransparentPass`, `TransparentDepthPass`) and the shaders
//!   (`workbench_prepass.bsl.hh` `frag_transparent`, `workbench_transparent_resolve_frag.glsl`):
//!   weighted blended order-independent transparency (McGuire and Bavoil 2013): surfaces
//!   accumulate `shaded * w, alpha` (one, one / zero, one minus source alpha) and `w` into
//!   half-float targets, depth-tested against the opaque depth without writing it; the resolve
//!   blends `accum.rgb / clamp(w, 1e-4, 5e4)` with alpha `1 - revealage` over the colour; a
//!   last pass writes their depth and object ids (for the outline).

use frap_render_scene::RWorkbenchShading;

/// The effective X-ray alpha (`SceneState::init`): 1 = no X-ray.
pub fn xray_alpha(w: &RWorkbenchShading) -> f32 {
    if super::wire::is_wireframe(&w.shading_type) {
        return 0.0;
    }
    if w.show_xray && w.xray_alpha < 1.0 {
        w.xray_alpha
    } else {
        1.0
    }
}

/// `Material::is_transparent`: the alpha packed in 8 bits is below 255.
pub fn is_transparent(alpha: f32) -> bool {
    pack_unit(alpha) != 255
}

/// `unit_float_to_uchar_clamp` (the packing of `Material::pack_data`).
pub fn pack_unit(v: f32) -> u32 {
    if v.is_nan() || v <= 0.0 {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u32
    }
}

/// The alpha the transparent pass reads (`material_data_get`: 8 bits).
pub fn packed_alpha(alpha: f32) -> f32 {
    pack_unit(alpha) as f32 * (1.0 / 255.0)
}

/// `calculate_transparent_weight` of a fragment at view distance `z` (perspective), or at
/// distance `z` from the near plane (orthographic): eq. 7 of McGuire and Bavoil, clamped.
pub fn transparent_weight(z: f32) -> f32 {
    let a = z.abs() / 5.0;
    let mut b = z.abs() / 200.0;
    b *= b;
    let w = 10.0 / ((1e-5 + a * a) + b * (b * b));
    w.clamp(1e-2, 3e2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xray_alpha_rules() {
        let mut w = RWorkbenchShading::default();
        assert_eq!(xray_alpha(&w), 1.0);
        w.show_xray = true;
        assert_eq!(xray_alpha(&w), 0.5);
        w.xray_alpha = 1.0;
        assert_eq!(xray_alpha(&w), 1.0, "alpha 1 is not X-ray");
        w.shading_type = "WIREFRAME".into();
        assert_eq!(xray_alpha(&w), 0.0);
    }

    #[test]
    fn transparency_follows_the_8_bit_packing() {
        assert!(!is_transparent(1.0));
        assert!(!is_transparent(0.999));
        assert!(is_transparent(0.997));
        assert!(is_transparent(0.5));
        assert_eq!(packed_alpha(0.5), 128.0 / 255.0);
    }

    #[test]
    fn weights_favour_near_surfaces() {
        assert_eq!(transparent_weight(0.0), 3e2);
        assert!(transparent_weight(5.0) < transparent_weight(1.0));
        assert_eq!(transparent_weight(1e4), 1e-2);
        assert!((transparent_weight(5.0) - 10.0 / (1e-5 + 1.0 + 0.025f32.powi(6))).abs() < 1e-3);
    }
}
