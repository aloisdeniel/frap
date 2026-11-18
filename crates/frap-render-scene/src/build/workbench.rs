// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench colour sources (§19.3): what Blender's Workbench reads for each `color_type` and
//! for the background, resolved here so the engine only sees plain values.
//!
//! - MATERIAL: `Material.diffuse_color` / `.metallic` / `.roughness` (viewport display values;
//!   `BKE_material_default_empty()` for an empty slot) → `RSettings::workbench_materials`.
//! - OBJECT: `Object.color` → `RObject::color`.
//! - RANDOM: `BLI_ghashutil_strhash_p_murmur(ob->id.name)`, mixed with the library path for a
//!   linked object (`workbench_materials.cc`, `Material::Material(::Object &, bool random)`)
//!   → `RObject::random_hash`.
//! - SINGLE: `display.shading.single_color` → `RSettings::shading_single_color`.
//! - Background (verified on 5.2.4): `film_transparent` → transparent; `background_type =
//!   'VIEWPORT'` → `shading.background_color`; otherwise `World.color`, or transparent black
//!   when the scene has no world → `RSettings::workbench_background`.

use frap_scene::Main;
use frap_scene::id::{Material, Object, Scene, World};

use crate::types::RWorkbenchMaterial;

/// `BLI_hash_mm2(data, len, 0)`: MurmurHash2, seed 0 (blenlib `hash_mm2a.cc`).
pub fn murmur2(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let r = chunks.remainder();
    if r.len() >= 3 {
        h ^= u32::from(r[2]) << 16;
    }
    if r.len() >= 2 {
        h ^= u32::from(r[1]) << 8;
    }
    if !r.is_empty() {
        h ^= u32::from(r[0]);
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// `BLI_ghashutil_strhash_p_murmur(s)`: [`murmur2`] of the string's bytes **including** the
/// terminating NUL (`strlen + 1` bytes).
pub fn strhash_murmur(s: &str) -> u32 {
    let mut b = Vec::with_capacity(s.len() + 1);
    b.extend_from_slice(s.as_bytes());
    b.push(0);
    murmur2(&b)
}

/// Workbench RANDOM colour hash of an object (`id.name` is `"OB" + name`).
pub fn random_hash(main: &Main, ob: &Object) -> u32 {
    let mut hash = strhash_murmur(&format!("OB{}", ob.header.name));
    if let Some(lib) = ob.header.library.and_then(|l| main.libraries.get(l)) {
        hash = hash.wrapping_mul(13) ^ strhash_murmur(&lib.filepath);
    }
    hash
}

/// The MATERIAL colour source of one material.
pub fn material(m: &Material) -> RWorkbenchMaterial {
    RWorkbenchMaterial {
        diffuse_color: m.diffuse_color,
        metallic: m.metallic,
        roughness: m.roughness,
    }
}

/// The Workbench render background (straight RGBA).
pub fn background(scene: &Scene, world: Option<&World>) -> [f32; 4] {
    if scene.render.film_transparent {
        return [0.0; 4];
    }
    let shading = &scene.display.shading;
    if shading.background_type == "VIEWPORT" {
        let c = shading.background_color;
        return [c[0], c[1], c[2], 1.0];
    }
    match world {
        Some(w) => [w.color[0], w.color[1], w.color[2], 1.0],
        None => [0.0; 4],
    }
}

/// `scene.display` / `scene.display.shading` values of the complete Workbench (§19.3), plain
/// data for the engine (M2 W0; WP 2.3 owns the engine side).
pub fn shading(scene: &Scene) -> crate::types::RWorkbenchShading {
    let d = &scene.display;
    let s = &d.shading;
    crate::types::RWorkbenchShading {
        shading_type: s.r#type.clone(),
        // `workbench_state.cc`: the MatCap comes from DNA `matcap`, the studio rig from DNA
        // `studio_light` (RNA `studio_light` routes between them by `light`).
        studio_light: if s.light == "MATCAP" {
            s.matcap.clone()
        } else {
            s.studio_light.clone()
        },
        studiolight_rotate_z: s.studiolight_rotate_z,
        use_world_space_lighting: s.use_world_space_lighting,
        show_backface_culling: s.show_backface_culling,
        show_xray: s.show_xray,
        xray_alpha: s.xray_alpha,
        show_xray_wireframe: s.show_xray_wireframe,
        xray_alpha_wireframe: s.xray_alpha_wireframe,
        wireframe_color_type: s.wireframe_color_type.clone(),
        show_object_outline: s.show_object_outline,
        object_outline_color: s.object_outline_color,
        show_shadows: s.show_shadows,
        shadow_intensity: s.shadow_intensity,
        light_direction: d.light_direction,
        shadow_shift: d.shadow_shift,
        shadow_focus: d.shadow_focus,
        show_cavity: s.show_cavity,
        cavity_type: s.cavity_type.clone(),
        cavity_ridge_factor: s.cavity_ridge_factor,
        cavity_valley_factor: s.cavity_valley_factor,
        curvature_ridge_factor: s.curvature_ridge_factor,
        curvature_valley_factor: s.curvature_valley_factor,
        matcap_ssao_distance: d.matcap_ssao_distance,
        matcap_ssao_attenuation: d.matcap_ssao_attenuation,
        matcap_ssao_samples: d.matcap_ssao_samples,
        show_specular_highlight: s.show_specular_highlight,
        use_dof: s.use_dof,
        render_aa: d.render_aa.clone(),
    }
}
