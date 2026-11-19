// SPDX-License-Identifier: GPL-3.0-or-later
//! Vertex colour: a colour attribute that feeds the base colour or the alpha
//! ([`LoweredMaterial::vertex_color`], ARCHITECTURE §16.2). The exporter's `COLOR_0` writer
//! reads it (`export_vertex_color='MATERIAL'` exports the attributes a material uses; when
//! no material uses one, `export_all_vertex_colors` inserts a white `COLOR_0` so that the
//! other colours do not tint the material).
//!
//! Lowering ports the attribute half of `gather_color_info` / `gather_alpha_info`
//! (`get_color_attrib`, `detect_multiply_by_color_attrib`, `search_node_tree.py`) as
//! `__gather_base_color_factor` (`pbr_metallic_roughness.py`) and the unlit path
//! (`unlit.py`, glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors) use them: a
//! Color Attribute node (`layer_name`) or an Attribute node of type GEOMETRY
//! (`attribute_name`) feeding `Base Color` (or `Alpha`) directly, or multiplied with it
//! (a Mix RGBA MULTIPLY with factor 1, a Math MULTIPLY), after an optional multiplication by
//! a constant. An empty name is the active render colour attribute
//! ([`VertexColorUse::attribute`] `None`). Both halves are kept, as the add-on's `vc_info`
//! keeps them: [`VertexColorUse::color`] / `attribute` for Base Color,
//! [`VertexColorUse::alpha`] / `alpha_attribute` for Alpha (the `COLOR_n` writer adds the
//! alpha only for a material that is not OPAQUE).
//!
//! Raising is part of `base_color()` (`base_color.rs`): the importer builds the Color
//! Attribute node (with an empty layer name) and the "Mix Vertex Color" nodes there.

use smol_str::SmolStr;

use crate::graph::In;
use crate::{LowerCx, LoweredMaterial, MaterialPattern, VertexColorUse};

/// The vertex colour use of a colour socket and an alpha socket (the add-on's `vc_info`:
/// `color`/`color_type` from `gather_color_info`, `alpha`/`alpha_type` from
/// `gather_alpha_info`).
pub(crate) fn vertex_color_of<'a>(
    cx: &LowerCx<'a>,
    color: Option<In<'a>>,
    alpha: Option<In<'a>>,
) -> Option<VertexColorUse> {
    let name = |attrib: SmolStr| (!attrib.is_empty()).then_some(attrib);
    let color_use = color.and_then(|c| {
        let a = cx.color_info(c).attrib?;
        // Directly (`get_color_attrib`) or multiplied with the rest.
        let direct = {
            let mut nav = c;
            if let Some((_, rest)) = cx.multiply_by_constant(nav) {
                nav = rest;
            }
            cx.color_attrib(nav).is_some()
        };
        Some((name(a), !direct))
    });
    let alpha_use = alpha
        .and_then(|a| super::alpha::gather_alpha_info(cx, a).attrib)
        .map(|(a, multiply)| (name(a), multiply));
    match (color_use, alpha_use) {
        (None, None) => None,
        (Some((attribute, multiply)), alpha) => Some(VertexColorUse {
            attribute,
            multiply,
            color: true,
            alpha: alpha.is_some(),
            alpha_attribute: alpha.and_then(|(a, _)| a),
        }),
        (None, Some((attribute, multiply))) => Some(VertexColorUse {
            alpha_attribute: attribute.clone(),
            attribute,
            multiply,
            color: false,
            alpha: true,
        }),
    }
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit {
        return;
    }
    let color = cx
        .get_socket("Base Color")
        .or_else(|| cx.get_socket("BaseColor"))
        .or_else(|| cx.gltf_output_input("BaseColorFactor"));
    out.vertex_color = vertex_color_of(cx, color, cx.get_socket("Alpha"));
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "vertex_color",
    order: 17,
    lower,
    raise: None,
};

#[cfg(test)]
mod tests {
    use frap_rna::Value;

    use crate::VertexColorUse;
    use crate::graph::testkit::*;

    #[test]
    fn colour_attributes_feeding_the_base_colour() {
        let mut images = Images::default();
        let img = images.add("albedo", false);
        // Directly, the active render attribute.
        let mut m = mat("Direct");
        let vc = add(&mut m, "ShaderNodeVertexColor");
        link(&mut m, (vc, "Color"), (BSDF, "Base Color"));
        let lm = lower_m(&m, &images);
        assert_eq!(
            lm.vertex_color,
            Some(VertexColorUse::color_only(None, false))
        );
        assert_law(&lm, &images);
        // Multiplied with a texture, by name.
        let mut m = mat("Mul");
        let vc = add(&mut m, "ShaderNodeVertexColor");
        assert!(
            m.node_tree
                .node_mut(vc)
                .unwrap()
                .set_prop("layer_name", Value::Str("Paint".into()))
        );
        let t = image_node(&mut m, img);
        let mix = add(&mut m, "ShaderNodeMix");
        set_enum(&mut m, mix, "data_type", "RGBA");
        set_enum(&mut m, mix, "blend_type", "MULTIPLY");
        set_in(&mut m, mix, "Factor", float(1.0));
        link(&mut m, (t, "Color"), (mix, "A"));
        link(&mut m, (vc, "Color"), (mix, "B"));
        link(&mut m, (mix, "Result"), (BSDF, "Base Color"));
        let lm = lower_m(&m, &images);
        assert_eq!(
            lm.vertex_color,
            Some(VertexColorUse::color_only(Some("Paint".into()), true))
        );
        assert_eq!(lm.base_color.texture.as_ref().unwrap().image, img);
        // An Attribute node of type GEOMETRY on Alpha.
        let mut m = mat("Alpha");
        let at = add(&mut m, "ShaderNodeAttribute");
        assert!(
            m.node_tree
                .node_mut(at)
                .unwrap()
                .set_prop("attribute_name", Value::Str("Col".into()))
        );
        link(&mut m, (at, "Factor"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        let v = lm.vertex_color.clone().unwrap();
        assert!(!v.color && v.alpha && !v.adds_alpha(lm.alpha_mode));
        assert_eq!(v.alpha_attribute, Some("Col".into()));
        assert_law(&lm, &images);
        // Color and Alpha from the same Color Attribute node (the importer's graph): both
        // halves, and COLOR_0 carries alpha once the material blends.
        let mut m = mat("Both");
        let vc = add(&mut m, "ShaderNodeVertexColor");
        link(&mut m, (vc, "Color"), (BSDF, "Base Color"));
        link(&mut m, (vc, "Alpha"), (BSDF, "Alpha"));
        let lm = lower_m(&m, &images);
        assert_eq!(
            lm.vertex_color,
            Some(VertexColorUse::color_and_alpha(None, false))
        );
        assert!(lm.vertex_color.as_ref().unwrap().adds_alpha(lm.alpha_mode));
        assert_law(&lm, &images);
        // Color from one attribute, Alpha from another one by name.
        let mut m = mat("Split");
        let vc = add(&mut m, "ShaderNodeVertexColor");
        let at = add(&mut m, "ShaderNodeAttribute");
        assert!(
            m.node_tree
                .node_mut(at)
                .unwrap()
                .set_prop("attribute_name", Value::Str("Mask".into()))
        );
        link(&mut m, (vc, "Color"), (BSDF, "Base Color"));
        link(&mut m, (at, "Factor"), (BSDF, "Alpha"));
        let v = lower_m(&m, &images).vertex_color.unwrap();
        assert_eq!(
            (v.attribute, v.color, v.alpha, v.alpha_attribute),
            (None, true, true, Some("Mask".into()))
        );
        // None: no colour attribute.
        assert_eq!(lower_m(&mat("Plain"), &images).vertex_color, None);
    }
}
