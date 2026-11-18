// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench object outline (`shading.show_object_outline`, `object_outline_color`).
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later): `workbench_effect_outline.cc` and
//! `shaders/workbench_effect_outline_frag.glsl`. Opaque surfaces (and, after the transparent
//! passes, transparent ones) write a per-object id (`resource_id + 1`; the background is 0)
//! into an integer target. A full-screen pass compares each pixel's id with its four
//! neighbours (one pixel away: `ui_scale` is 1 in final renders) and blends
//! `object_outline_color × (1 - matching / 4)` over the colour with premultiplied alpha, before
//! anti-aliasing; both sides of an id boundary get half the outline.

/// The outline opacity of a pixel with id `center` and neighbours `adjacent` (above, below,
/// right, left): `1 - dot(equal(center, adjacent), 0.25)`.
pub fn outline_opacity(center: u32, adjacent: [u32; 4]) -> f32 {
    1.0 - adjacent.iter().filter(|&&a| a == center).count() as f32 * 0.25
}

/// The premultiplied-alpha blend of the outline over `dst` (`DRW_STATE_BLEND_ALPHA_PREMUL`:
/// `src + dst × (1 - src.a)`).
pub fn blend(color: [f32; 3], opacity: f32, dst: [f32; 4]) -> [f32; 4] {
    let src = [
        color[0] * opacity,
        color[1] * opacity,
        color[2] * opacity,
        opacity,
    ];
    std::array::from_fn(|i| src[i] + dst[i] * (1.0 - opacity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_counts_differing_neighbours() {
        assert_eq!(outline_opacity(1, [1, 1, 1, 1]), 0.0);
        assert_eq!(outline_opacity(0, [1, 0, 0, 0]), 0.25);
        assert_eq!(outline_opacity(2, [1, 1, 2, 0]), 0.75);
        assert_eq!(outline_opacity(3, [0, 0, 0, 0]), 1.0);
    }

    #[test]
    fn premultiplied_blend() {
        assert_eq!(
            blend([0.0; 3], 0.5, [1.0, 1.0, 1.0, 1.0]),
            [0.5, 0.5, 0.5, 1.0]
        );
        assert_eq!(blend([1.0, 0.0, 0.0], 1.0, [0.2; 4]), [1.0, 0.0, 0.0, 1.0]);
    }
}
