// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_pbrSpecularGlossiness`, import only: the importer's spec-gloss graph.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/KHR_materials_pbrSpecularGlossiness.py`
//! (`pbr_specular_glossiness`, `glossiness`, `calc_locations`; Apache-2.0, The glTF-Blender-IO
//! authors). The Principled BSDF gets, in the importer's creation order: the diffuse colour
//! through `base_color(is_diffuse=True)` (an Image Texture labelled "DIFFUSE"), emission,
//! the normal map, the occlusion texture into the "glTF Material Output" group node (placed at
//! (10, 425), width 240), `IOR` 1000 (so that F0 ≈ the specular tint), the specular colour
//! into `Specular Tint` ("Specular Color" factor and texture) and `Roughness` = 1 −
//! `glossinessFactor`, or the baked roughness image's `Alpha` (label "ROUGHNESS") when the
//! material has a `specularGlossinessTexture` and a non-zero glossiness factor. Metallic and
//! the other Principled inputs keep their defaults; no other extension is raised.
//!
//! The importer (`frap-io-gltf`) fills [`SpecularGlossiness`] (and no other extension) and
//! bakes the roughness image. The pattern raises first (order 0) and then clears
//! [`RaiseCx::bsdf`], so the patterns after it, which all link into the Principled BSDF,
//! add nothing. Nothing is lowered: the exporter never writes the extension.

use std::collections::BTreeMap;

use frap_nodes::SocketValue;

use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, SpecularGlossiness};

/// Height of one block of the importer's layout.
const BLOCK_HEIGHT: f32 = 460.0;

/// `calc_locations` of the spec-gloss importer: blocks top to bottom (occlusion, diffuse,
/// glossiness, normal, specular, emission), centred vertically. The diffuse block is stored
/// under `diffuse`; the emission, normal and occlusion raisers read their usual keys.
fn calc_locations(lm: &LoweredMaterial, sg: &SpecularGlossiness) -> BTreeMap<&'static str, [f32; 2]> {
    let x = -200.0;
    let mut y = 0.0;
    let blocks: [(&'static str, bool); 6] = [
        ("occlusion", lm.occlusion.is_some()),
        (
            "diffuse",
            lm.base_color.texture.is_some() || lm.vertex_color.is_some(),
        ),
        ("glossiness", sg.has_texture && sg.glossiness_factor != 0.0),
        ("normal", lm.normal.is_some()),
        ("specular", sg.has_texture),
        ("emission", lm.emissive.texture.is_some()),
    ];
    let mut locs = BTreeMap::new();
    for (key, takes_space) in blocks {
        locs.insert(key, [x, y]);
        if takes_space {
            y -= BLOCK_HEIGHT;
        }
    }
    let y_offset = -y / 2.0 - 20.0;
    for v in locs.values_mut() {
        v[1] += y_offset;
    }
    locs
}

/// `pbr_specular_glossiness(mh)` on the graph `reset_graph` made (Principled BSDF at (10, 300)
/// → Material Output at (300, 300)).
fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(sg) = &lm.ext.pbr_specular_glossiness else {
        return;
    };
    raise_graph(lm, sg, cx);
    // The graph is complete: the patterns after this one find no Principled BSDF.
    cx.bsdf = None;
}

fn raise_graph(lm: &LoweredMaterial, sg: &SpecularGlossiness, cx: &mut RaiseCx) {
    cx.locations = calc_locations(lm, sg);
    // `set_double_sided` (the importer sets it before building any graph).
    cx.into.use_backface_culling = !lm.double_sided;
    let Some(bsdf) = cx.bsdf else { return };
    let color = cx.input(bsdf, "Base Color");
    let alpha = cx.input(bsdf, "Alpha");
    let loc = cx.location("diffuse");
    super::base_color::raise_base_color(lm, cx, loc, color, Some(alpha), "DIFFUSE");
    super::emission::raise(lm, cx);
    super::normal_map::raise(lm, cx);
    if lm.occlusion.is_some() {
        let settings = cx.settings_node();
        let n = cx.node_mut(settings);
        n.location = [10.0, 425.0];
        n.width = 240.0;
        super::occlusion::raise(lm, cx);
    }
    // F0 = specular tint × ((1 − IOR) / (1 + IOR))²: IOR 1000 makes the factor ≈ 1.
    cx.set_input(bsdf, "IOR", SocketValue::Float(1000.0));
    let socket = cx.input(bsdf, "Specular Tint");
    let loc = cx.location("specular");
    cx.color_factor_and_texture(
        loc,
        "Specular Color",
        socket,
        sg.specular.factor,
        sg.specular.texture.as_ref(),
    );
    // `glossiness`: Roughness = 1 − glossinessFactor × A.
    let roughness = cx.input(bsdf, "Roughness");
    if !sg.has_texture || sg.glossiness_factor == 0.0 {
        let v = (1.0 - f64::from(sg.glossiness_factor)) as f32;
        cx.set_input_at(roughness.node, roughness.index, SocketValue::Float(v));
    } else if let Some(t) = &sg.roughness {
        let loc = cx.location("glossiness");
        cx.texture(t, loc, "ROUGHNESS", None, Some(roughness));
    }
}

/// Nothing to lower: the exporter does not write `KHR_materials_pbrSpecularGlossiness`.
fn lower(_cx: &LowerCx, _out: &mut LoweredMaterial) {}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "pbr_specular_glossiness",
    order: 0,
    lower,
    raise: Some(raise),
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, NameAlloc};
    use frap_scene::id::Material;

    fn spec_gloss(lm: &mut LoweredMaterial, sg: SpecularGlossiness) -> Material {
        lm.ext.pbr_specular_glossiness = Some(sg);
        let mut mat = Material::new("SG");
        crate::raise(lm, &mut mat, &mut NameAlloc::default());
        mat
    }

    fn input(mat: &Material, name: &str) -> SocketValue {
        let n = mat
            .node_tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap();
        n.inputs[n.input_index(name).unwrap()].default_value.clone()
    }

    /// Verified on 5.2.4 (`diffuseFactor` (0.2, 0.6, 0.9, 1), `glossinessFactor` 0.3, no
    /// texture): Base Color (0.2, 0.6, 0.9, 1), Roughness 0.7, Metallic 0 (the default), IOR
    /// 1000, Specular Tint white; only the BSDF and the output.
    #[test]
    fn factors_only() {
        let mut lm = LoweredMaterial::gltf_default();
        lm.base_color.factor = [0.2, 0.6, 0.9, 1.0];
        let mat = spec_gloss(
            &mut lm,
            SpecularGlossiness {
                specular: Channel {
                    factor: [1.0; 3],
                    texture: None,
                },
                glossiness_factor: 0.3,
                has_texture: false,
                roughness: None,
            },
        );
        assert_eq!(mat.node_tree.nodes.len(), 2);
        assert_eq!(
            input(&mat, "Base Color"),
            SocketValue::Color([0.2, 0.6, 0.9, 1.0])
        );
        assert_eq!(input(&mat, "Roughness"), SocketValue::Float(0.7));
        assert_eq!(input(&mat, "Metallic"), SocketValue::Float(0.0));
        assert_eq!(input(&mat, "IOR"), SocketValue::Float(1000.0));
        assert_eq!(input(&mat, "Specular Tint"), SocketValue::Color([1.0; 4]));
        // `set_viewport_color` is skipped for spec-gloss materials.
        assert_eq!(mat.diffuse_color, Material::new("x").diffuse_color);
    }

    #[test]
    fn locations_match_the_importer() {
        let lm = LoweredMaterial::gltf_default();
        let sg = SpecularGlossiness {
            specular: Channel {
                factor: [1.0; 3],
                texture: None,
            },
            glossiness_factor: 0.5,
            has_texture: true,
            roughness: None,
        };
        let l = calc_locations(&lm, &sg);
        // Two blocks take space (glossiness, specular): offset 460 − 20.
        assert_eq!(l["diffuse"], [-200.0, 440.0]);
        assert_eq!(l["glossiness"], [-200.0, 440.0]);
        assert_eq!(l["specular"], [-200.0, -20.0]);
        assert_eq!(l["emission"], [-200.0, -480.0]);
    }
}
