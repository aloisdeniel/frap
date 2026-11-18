// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench `light='MATCAP'`: the 27 matcaps Blender 5.2.4 ships, embedded from
//! `assets/matcaps/` (CC0; provenance and the 256 x 256 reduction in
//! `assets/matcaps/README.md`), and the matcap lookup.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `workbench_resources.cc` (`get_matcap_tx`, `SceneResources::init`): the matcap is found by
//!   name among the MATCAP studio lights; its diffuse and (when present) specular passes are
//!   the two layers of one half-float texture array; `use_specular` is
//!   `show_specular_highlight` and the matcap having a specular pass
//!   (`STUDIOLIGHT_SPECULAR_HIGHLIGHT_PASS`); the solid lights of a matcap studio light are
//!   unused (`flag` 0) and its ambient is black. An unknown matcap name falls back to the
//!   first matcap (`BKE_studiolight_find` with `STUDIOLIGHT_TYPE_MATCAP`), which is what the
//!   factory DNA value `basic_1.exr` resolves to (`basic_bright.exr`, verified).
//! - `workbench_matcap.bsl.hh` (`matcap_uv_compute`, `get_matcap_lighting`): evaluated in
//!   `workbench.wgsl`; [`matcap_uv`] is the CPU reference used by the tests.
//!
//! Textures are uploaded bottom row first, as Blender's `ImBuf`s are, so the shader's UVs are
//! Blender's.

use std::sync::{Arc, Mutex, OnceLock};

use frap_imbuf::PixelData;

use super::color::f32_to_f16_bits;

/// One embedded matcap: Blender's name and its passes (single-layer RGB half EXRs).
struct Embedded {
    name: &'static str,
    diffuse: &'static [u8],
    specular: Option<&'static [u8]>,
}

macro_rules! matcap {
    ($name:literal, $stem:literal, specular) => {
        Embedded {
            name: $name,
            diffuse: include_bytes!(concat!(
                "../../../../assets/matcaps/",
                $stem,
                ".diffuse.exr"
            )),
            specular: Some(include_bytes!(concat!(
                "../../../../assets/matcaps/",
                $stem,
                ".specular.exr"
            ))),
        }
    };
    ($name:literal, $stem:literal) => {
        Embedded {
            name: $name,
            diffuse: include_bytes!(concat!(
                "../../../../assets/matcaps/",
                $stem,
                ".diffuse.exr"
            )),
            specular: None,
        }
    };
}

/// Blender's `preferences.studio_lights` MATCAP entries, in its order
/// (`assets/matcaps/MANIFEST.txt`).
const MATCAPS: &[Embedded] = &[
    matcap!("basic_bright.exr", "basic_bright", specular),
    matcap!("basic_dark.exr", "basic_dark", specular),
    matcap!("basic_grey.exr", "basic_grey", specular),
    matcap!("basic_side.exr", "basic_side", specular),
    matcap!(
        "brush_thumbnail_preview.exr",
        "brush_thumbnail_preview",
        specular
    ),
    matcap!("ceramic_dark.exr", "ceramic_dark", specular),
    matcap!("ceramic_lightbulb.exr", "ceramic_lightbulb", specular),
    matcap!("check_gradient.exr", "check_gradient", specular),
    matcap!("check_normal+y.exr", "check_normal+y"),
    matcap!(
        "check_reflection_horizontal.exr",
        "check_reflection_horizontal",
        specular
    ),
    matcap!(
        "check_reflection_vertical.exr",
        "check_reflection_vertical",
        specular
    ),
    matcap!("check_rim_dark.exr", "check_rim_dark", specular),
    matcap!("check_rim_light.exr", "check_rim_light", specular),
    matcap!("clay_brown.exr", "clay_brown", specular),
    matcap!("clay_green.exr", "clay_green", specular),
    matcap!("clay_studio.exr", "clay_studio", specular),
    matcap!("clay_warm.exr", "clay_warm", specular),
    matcap!("fullmetal.exr", "fullmetal", specular),
    matcap!("hard_surface_grey.exr", "hard_surface_grey", specular),
    matcap!("hard_surface_red.exr", "hard_surface_red", specular),
    matcap!("metal_bronze.exr", "metal_bronze", specular),
    matcap!("metal_carpaint.exr", "metal_carpaint", specular),
    matcap!("pearl.exr", "pearl", specular),
    matcap!("red_wax.exr", "red_wax", specular),
    matcap!("resin.exr", "resin", specular),
    matcap!("toon_dark.exr", "toon_dark", specular),
    matcap!("toon_light.exr", "toon_light", specular),
];

/// The matcap of a `studio_light` name when it names none (`BKE_studiolight_find` falls back to
/// the first MATCAP studio light).
pub const FALLBACK: &str = "basic_bright.exr";

/// Names of the embedded matcaps (`View3DShading.studio_light` items in MATCAP mode).
pub fn names() -> impl Iterator<Item = &'static str> {
    MATCAPS.iter().map(|m| m.name)
}

/// The matcap a `studio_light` value selects (unknown names: [`FALLBACK`]).
pub fn resolve(name: &str) -> &'static str {
    MATCAPS
        .iter()
        .find(|m| m.name == name)
        .map_or(FALLBACK, |m| m.name)
}

/// A decoded matcap: two `size x size` RGBA layers (diffuse, specular; the specular layer is
/// black without a specular pass), bottom row first, as half-float bits.
#[derive(Debug, PartialEq)]
pub struct Matcap {
    /// Blender's name (`basic_bright.exr`).
    pub name: &'static str,
    /// Side of each layer.
    pub size: u32,
    /// `size * size * 4` half floats per layer, diffuse then specular.
    pub texels: Vec<u16>,
    /// Whether the matcap has a specular pass (`STUDIOLIGHT_SPECULAR_HIGHLIGHT_PASS`).
    pub has_specular: bool,
}

fn decode_layer(bytes: &[u8], what: &str) -> Result<(u32, Vec<[f32; 3]>), String> {
    let img = frap_imbuf::decode(bytes).map_err(|e| format!("matcap {what}: {e}"))?;
    if img.width != img.height || img.channels < 3 {
        return Err(format!(
            "matcap {what}: expected a square RGB image, got {}x{}x{}",
            img.width, img.height, img.channels
        ));
    }
    let PixelData::F32(px) = &img.pixels else {
        return Err(format!("matcap {what}: expected float pixels"));
    };
    let ch = img.channels as usize;
    let w = img.width as usize;
    // Top row first in the file → bottom row first, as Blender's `ImBuf`.
    let mut out = Vec::with_capacity(w * w);
    for row in (0..w).rev() {
        for col in 0..w {
            let i = (row * w + col) * ch;
            out.push([px[i], px[i + 1], px[i + 2]]);
        }
    }
    Ok((img.width, out))
}

fn load(m: &Embedded) -> Result<Matcap, String> {
    let (size, diffuse) = decode_layer(m.diffuse, m.name)?;
    let specular = match m.specular {
        Some(b) => {
            let (s, layer) = decode_layer(b, m.name)?;
            if s != size {
                return Err(format!("matcap {}: pass sizes differ", m.name));
            }
            Some(layer)
        }
        None => None,
    };
    let n = (size * size) as usize;
    let mut texels = Vec::with_capacity(n * 8);
    let one = f32_to_f16_bits(1.0);
    for p in &diffuse {
        texels.extend(p.iter().map(|v| f32_to_f16_bits(*v)));
        texels.push(one);
    }
    match &specular {
        Some(layer) => {
            for p in layer {
                texels.extend(p.iter().map(|v| f32_to_f16_bits(*v)));
                texels.push(one);
            }
        }
        None => texels.extend(std::iter::repeat_n(0u16, n * 4)),
    }
    Ok(Matcap {
        name: m.name,
        size,
        texels,
        has_specular: specular.is_some(),
    })
}

/// The decoded matcap a `studio_light` value selects (decoded once per process, then shared).
pub fn get(name: &str) -> Result<Arc<Matcap>, String> {
    static CACHE: OnceLock<Mutex<Vec<Arc<Matcap>>>> = OnceLock::new();
    let name = resolve(name);
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(m) = cache.iter().find(|m| m.name == name) {
        return Ok(m.clone());
    }
    let e = MATCAPS
        .iter()
        .find(|m| m.name == name)
        .expect("invariant: resolve returns an embedded name");
    let m = Arc::new(load(e)?);
    cache.push(m.clone());
    Ok(m)
}

/// `matcap_uv_compute(I, N, flipped)`: the matcap texture coordinate of view-space normal `n`
/// seen along the view-space incident vector `i` (both normalized).
pub fn matcap_uv(i: [f32; 3], n: [f32; 3], flipped: bool) -> [f32; 2] {
    let a = 1.0 / (1.0 + i[2]);
    let b = -i[0] * i[1] * a;
    let b1 = [1.0 - i[0] * i[0] * a, b, -i[0]];
    let b2 = [b, 1.0 - i[1] * i[1] * a, -i[1]];
    let dot = |u: [f32; 3], v: [f32; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let mut uv = [dot(b1, n), dot(b2, n)];
    if flipped {
        uv[0] = -uv[0];
    }
    [uv[0] * 0.496 + 0.5, uv[1] * 0.496 + 0.5]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_matcap_decodes() {
        let names: Vec<_> = names().collect();
        assert_eq!(names.len(), 27);
        let manifest = include_str!("../../../../assets/matcaps/MANIFEST.txt");
        let listed: Vec<_> = manifest
            .lines()
            .map(|l| l.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(names, listed, "MATCAPS follows MANIFEST.txt");
        for line in manifest.lines() {
            let mut it = line.split_whitespace();
            let name = it.next().unwrap();
            let m = get(name).unwrap();
            assert_eq!(m.name, name);
            assert_eq!(m.size, 256);
            assert_eq!(m.texels.len(), 256 * 256 * 8);
            assert_eq!(m.has_specular, line.contains("specular"), "{name}");
        }
    }

    #[test]
    fn unknown_names_fall_back_to_the_first_matcap() {
        assert_eq!(resolve("basic_1.exr"), "basic_bright.exr");
        assert_eq!(resolve("Default"), "basic_bright.exr");
        assert_eq!(resolve("clay_brown.exr"), "clay_brown.exr");
    }

    #[test]
    fn uv_of_a_facing_normal_is_the_centre() {
        assert_eq!(
            matcap_uv([0.0, 0.0, 1.0], [0.0, 0.0, 1.0], false),
            [0.5, 0.5]
        );
        let right = matcap_uv([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], false);
        assert_eq!(right, [0.996, 0.5]);
        let flipped = matcap_uv([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], true);
        assert!(
            (flipped[0] - 0.004).abs() < 1e-6 && flipped[1] == 0.5,
            "{flipped:?}"
        );
        let up = matcap_uv([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], false);
        assert_eq!(up, [0.5, 0.996]);
    }
}
