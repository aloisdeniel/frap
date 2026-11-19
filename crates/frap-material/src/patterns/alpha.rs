// SPDX-License-Identifier: GPL-3.0-or-later
//! Alpha: Principled BSDF `Alpha` → `baseColorFactor[3]`, `alphaMode` and `alphaCutoff`
//! (ARCHITECTURE §16.2). The alpha mode comes from the node graph, never from
//! `blend_method` [RNA §0.9]. Whether an image's alpha is packed into the base-colour texture
//! is decided by `base_color.rs`, which owns that texture.
//!
//! Lowering ports `gather_alpha_info` / `detect_alpha_clip`
//! (`io_scene_gltf2/blender/exp/material/search_node_tree.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors): constant 1 → OPAQUE; clip setups (Math ROUND →
//! cutoff 0.5; `1 − (x < c)`; `1 − (c > x)`; legacy `x > c`, `c < x`) → MASK; then up to two
//! multiplications are peeled off, by a constant (the factor) and by a colour attribute (its
//! alpha, `vertex_color.rs`); without a clip, a factor 0 → MASK 0.5, 1 → OPAQUE, anything else
//! (another constant, a texture, an attribute) → BLEND.
//!
//! Raising ports `set_eevee_surface_render_method` (`imp/material.py`): BLEND gives
//! `surface_render_method = 'BLENDED'` (`blend_method` mirror `'BLEND'`), else `'DITHERED'`
//! (`'HASHED'`), verified on 5.2.4. The alpha nodes (clip frame, factor) are built with the
//! base colour by `base_color.rs`, as the importer's `base_color()` does.

use smol_str::SmolStr;

use crate::graph::{In, Konst};
use crate::{AlphaMode, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// What `gather_alpha_info` reads from an Alpha socket.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AlphaInfo {
    /// `alphaMode`
    pub mode: AlphaMode,
    /// `alphaFactor` (`None`: none found, i.e. 1).
    pub factor: Option<f32>,
    /// `alphaColorAttrib` and whether it multiplies (`detect_multiply_by_color_attrib`).
    pub attrib: Option<(SmolStr, bool)>,
}

/// `detect_alpha_clip`: the cutoff and the socket behind the clip nodes.
fn detect_clip<'a>(cx: &LowerCx<'a>, alpha: In<'a>) -> Option<(f32, In<'a>)> {
    let (n, _) = cx.peek_back(alpha)?;
    if n.bl_idname != "ShaderNodeMath" {
        return None;
    }
    let k = |node, index| {
        cx.get_constant(In { node, index })
            .and_then(Konst::value)
    };
    match n.prop_str("operation") {
        "ROUND" => return Some((0.5, In { node: n, index: 0 })),
        "SUBTRACT" if k(n, 0) == Some(1.0) => {
            if let Some((n2, _)) = cx.peek_back(In { node: n, index: 1 })
                && n2.bl_idname == "ShaderNodeMath"
            {
                let (in0, in1) = (
                    cx.get_constant(In { node: n2, index: 0 }),
                    cx.get_constant(In { node: n2, index: 1 }),
                );
                match (n2.prop_str("operation"), in0, in1) {
                    ("LESS_THAN", None, Some(c)) => {
                        return Some((konst_f32(c), In { node: n2, index: 0 }));
                    }
                    ("GREATER_THAN", Some(c), None) => {
                        return Some((konst_f32(c), In { node: n2, index: 1 }));
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    let (in0, in1) = (
        cx.get_constant(In { node: n, index: 0 }),
        cx.get_constant(In { node: n, index: 1 }),
    );
    match (n.prop_str("operation"), in0, in1) {
        ("GREATER_THAN", _, Some(c)) => Some((konst_f32(c), In { node: n, index: 0 })),
        ("LESS_THAN", Some(c), _) => Some((konst_f32(c), In { node: n, index: 1 })),
        _ => None,
    }
}

fn konst_f32(k: Konst) -> f32 {
    match k {
        Konst::Value(v) => v,
        Konst::Rgb(c) | Konst::Vector(c) => c[0],
    }
}

/// `gather_alpha_info(alpha_nav)`.
pub(crate) fn gather_alpha_info<'a>(cx: &LowerCx<'a>, alpha: In<'a>) -> AlphaInfo {
    if cx.get_constant(alpha) == Some(Konst::Value(1.0)) {
        return AlphaInfo {
            mode: AlphaMode::Opaque,
            factor: None,
            attrib: None,
        };
    }
    let (cutoff, mut nav) = match detect_clip(cx, alpha) {
        Some((c, rest)) => (Some(c), rest),
        None => (None, alpha),
    };
    let mut factor = None;
    let mut attrib = None;
    for _ in 0..2 {
        if factor.is_none() {
            if let Some(a) = cx.get_constant(nav) {
                factor = Some(konst_f32(a));
                break;
            }
            if let Some((a, rest)) = cx.multiply_by_constant(nav) {
                factor = Some(konst_f32(a));
                nav = rest;
                continue;
            }
        }
        if attrib.is_none() {
            if let Some(a) = cx.color_attrib(nav) {
                attrib = Some((a, false));
                break;
            }
            if let Some((a, rest)) = cx.multiply_by_color_attrib(nav) {
                attrib = Some((a, true));
                nav = rest;
                continue;
            }
        }
        break;
    }
    let mode = match (cutoff, factor) {
        (Some(c), _) => AlphaMode::Mask { cutoff: c },
        (None, Some(0.0)) => AlphaMode::Mask { cutoff: 0.5 },
        (None, Some(1.0)) => AlphaMode::Opaque,
        _ => AlphaMode::Blend,
    };
    AlphaInfo {
        mode,
        factor,
        attrib,
    }
}

/// `gather_alpha_info` of the Principled BSDF's `Alpha` (`None` without a Principled BSDF).
pub(crate) fn alpha_info(cx: &LowerCx<'_>) -> Option<AlphaInfo> {
    cx.get_socket("Alpha").map(|a| gather_alpha_info(cx, a))
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit {
        return;
    }
    let Some(info) = alpha_info(cx) else { return };
    out.alpha_mode = info.mode;
    out.base_color.factor[3] = info.factor.unwrap_or(1.0);
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    cx.into.surface_render_method = match lm.alpha_mode {
        AlphaMode::Blend => "BLENDED",
        _ => "DITHERED",
    }
    .into();
    cx.into.blend_method = match lm.alpha_mode {
        AlphaMode::Blend => "BLEND",
        _ => "HASHED",
    }
    .into();
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "alpha",
    order: 15,
    lower,
    raise: Some(raise),
};

#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;
    use crate::{AlphaMode, ChannelSel};

    fn clip(op_lt: bool, cutoff: f32) -> (frap_scene::id::Material, Images) {
        let mut images = Images::default();
        let img = images.add("leaf", false);
        let mut m = mat("Clip");
        let t = image_node(&mut m, img);
        let sub = add(&mut m, "ShaderNodeMath");
        let cmp = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, sub, "operation", "SUBTRACT");
        set_in_at(&mut m, sub, 0, float(1.0));
        if op_lt {
            // 1 - (alpha < cutoff)
            set_enum(&mut m, cmp, "operation", "LESS_THAN");
            set_in_at(&mut m, cmp, 1, float(cutoff));
            link_at(&mut m, (t, 1), (cmp, 0));
        } else {
            // 1 - (cutoff > alpha)
            set_enum(&mut m, cmp, "operation", "GREATER_THAN");
            set_in_at(&mut m, cmp, 0, float(cutoff));
            link_at(&mut m, (t, 1), (cmp, 1));
        }
        link_at(&mut m, (cmp, 0), (sub, 1));
        link(&mut m, (sub, "Value"), (BSDF, "Alpha"));
        link(&mut m, (t, "Color"), (BSDF, "Base Color"));
        (m, images)
    }

    #[test]
    fn constants_decide_the_mode() {
        let images = Images::default();
        for (a, mode) in [
            (1.0, AlphaMode::Opaque),
            (0.0, AlphaMode::Mask { cutoff: 0.5 }),
            (0.5, AlphaMode::Blend),
        ] {
            let mut m = mat("M");
            set_in(&mut m, BSDF, "Alpha", float(a));
            // `blend_method` is ignored [RNA §0.9].
            m.blend_method = "OPAQUE".into();
            let lm = lower_m(&m, &images);
            assert_eq!((lm.alpha_mode, lm.base_color.factor[3]), (mode, a));
            assert_law(&lm, &images);
        }
    }

    #[test]
    fn clip_setups_give_mask() {
        for lt in [true, false] {
            let (m, images) = clip(lt, 0.3);
            let lm = lower_m(&m, &images);
            assert_eq!(lm.alpha_mode, AlphaMode::Mask { cutoff: 0.3 });
            assert_eq!(lm.base_color.factor[3], 1.0);
            assert_eq!(
                lm.base_color.texture.as_ref().unwrap().channel,
                ChannelSel::Rgba,
                "the texture's own alpha behind the clip"
            );
            assert_law(&lm, &images);
        }
    }

    #[test]
    fn round_and_legacy_clips() {
        let mut images = Images::default();
        let img = images.add("a", false);
        // ROUND of a texture's alpha: MASK 0.5.
        let mut m = mat("Round");
        let t = image_node(&mut m, img);
        let r = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, r, "operation", "ROUND");
        link_at(&mut m, (t, 1), (r, 0));
        link(&mut m, (r, "Value"), (BSDF, "Alpha"));
        assert_eq!(
            lower_m(&m, &images).alpha_mode,
            AlphaMode::Mask { cutoff: 0.5 }
        );
        // Legacy `alpha > 0.2`.
        let mut m = mat("Legacy");
        let t = image_node(&mut m, img);
        let g = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, g, "operation", "GREATER_THAN");
        set_in_at(&mut m, g, 1, float(0.2));
        link_at(&mut m, (t, 1), (g, 0));
        link(&mut m, (g, "Value"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert_eq!(lm.alpha_mode, AlphaMode::Mask { cutoff: 0.2 });
        // An alpha-only texture times 0.5: BLEND with factor 0.5.
        let mut m = mat("Half");
        let t = image_node(&mut m, img);
        let mul = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, mul, "operation", "MULTIPLY");
        set_in_at(&mut m, mul, 1, float(0.5));
        link_at(&mut m, (t, 1), (mul, 0));
        link(&mut m, (mul, "Value"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert_eq!((lm.alpha_mode, lm.base_color.factor[3]), (AlphaMode::Blend, 0.5));
        assert_eq!(lm.base_color.texture.as_ref().unwrap().channel, ChannelSel::A);
        assert_law(&lm, &images);
    }

    #[test]
    fn constant_math_is_folded_like_the_inline_tree() {
        // Verified on 5.2.4 (golden gltf/materials_core/alpha_modes): ROUND(0.7) is folded to
        // the constant 1 by `InlineShaderNodes`, so the material is OPAQUE.
        let images = Images::default();
        let mut m = mat("Round");
        let r = add(&mut m, "ShaderNodeMath");
        set_enum(&mut m, r, "operation", "ROUND");
        set_in_at(&mut m, r, 0, float(0.7));
        link(&mut m, (r, "Value"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert_eq!((lm.alpha_mode, lm.base_color.factor[3]), (AlphaMode::Opaque, 1.0));
        // With a glTF Material Output Occlusion input the original tree is read: no folding.
        let g = settings_node(&mut m);
        let _ = g;
        let lm = lower_m(&m, &images);
        assert_eq!(
            (lm.alpha_mode, lm.base_color.factor[3]),
            (AlphaMode::Mask { cutoff: 0.5 }, 0.7)
        );
    }
}
