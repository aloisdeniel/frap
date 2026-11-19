// SPDX-License-Identifier: GPL-3.0-or-later
//! Normal map: Principled BSDF `Normal` → `normalTexture` with `scale` (ARCHITECTURE §16.2).
//!
//! Lowering ports `__gather_normal_texture` (`io_scene_gltf2/blender/exp/material/
//! materials.py`) and `__gather_normal_scale` (`texture_info.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors): the texture is the first Image Texture found
//! behind `Normal` (through the Normal Map node), used as is; the scale is the `Strength` of
//! the first Normal Map node found behind `Normal` when it is unlinked in the tree the
//! exporter reads (a link the inliner constant-folds, such as a Value node, gives its value);
//! 1 otherwise, which the exporter omits.
//!
//! Raising ports `normal()` / `normal_map()` (`imp/pbrMetallicRoughness.py`,
//! `imp/material_utils.py`): a Normal Map node (`uv_map` named after the `TEXCOORD_n`,
//! `Strength` = scale) and the Image Texture labelled "NORMAL MAP" (data colour space, set by
//! the importer on the image).

use crate::graph::{FoldVal, In};
use crate::{ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, NormalMap, RaiseCx};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    out.normal = None;
    if out.unlit {
        return;
    }
    let Some(socket) = cx.get_socket("Normal") else {
        return;
    };
    let Some(hit) = cx.texture_from(socket) else {
        return;
    };
    let Some(texture) = cx.tex_ref(hit.node, ChannelSel::Rgb, &mut out.diagnostics) else {
        return;
    };
    let scale = cx
        .node_from(socket, "ShaderNodeNormalMap")
        .and_then(|n| cx.inline_value(In::named(n, "Strength")?))
        .map(FoldVal::f)
        .unwrap_or(1.0);
    out.normal = Some(NormalMap { texture, scale });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(bsdf), Some(n)) = (cx.bsdf, &lm.normal) else {
        return;
    };
    let socket = cx.input(bsdf, "Normal");
    let loc = cx.location("normal");
    cx.normal_map(loc, "Normal Map", socket, n);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "normal_map",
    order: 42,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 40,
    key: "normal",
    takes_space: |lm| lm.normal.is_some(),
};

#[cfg(test)]
mod tests {
    use crate::graph::testkit::*;

    #[test]
    fn normal_map_texture_and_strength() {
        let mut images = Images::default();
        let img = images.add("bumps", true);
        let mut m = mat("N");
        let t = image_node(&mut m, img);
        let nm = add(&mut m, "ShaderNodeNormalMap");
        set_in(&mut m, nm, "Strength", float(0.7));
        link(&mut m, (t, "Color"), (nm, "Color"));
        link(&mut m, (nm, "Normal"), (BSDF, "Normal"));
        let lm = lower_m(&m, &images);
        let n = lm.normal.as_ref().unwrap();
        assert_eq!((n.texture.image, n.scale), (img, 0.7));
        assert!(n.texture.colorspace_is_data);
        assert_law(&lm, &images);
        // The importer's names: Normal Map then the Image Texture labelled "NORMAL MAP".
        let (raised, _) = raise_lower(&lm, &images);
        let tex = raised
            .node_tree
            .first_of_type("ShaderNodeTexImage")
            .unwrap();
        assert_eq!(tex.label, "NORMAL MAP");
        let nmap = raised
            .node_tree
            .first_of_type("ShaderNodeNormalMap")
            .unwrap();
        assert_eq!(nmap.prop_str("uv_map"), "UVMap");
        // A Strength linked from a Value node is folded by the inliner (verified on 5.2.4,
        // probe uv2.py: Value 3 → scale 3).
        let v = add(&mut m, "ShaderNodeValue");
        m.node_tree.node_mut(v).unwrap().outputs[0].default_value = float(3.0);
        link_at(&mut m, (v, 0), (nm, 0));
        assert_eq!(lower_m(&m, &images).normal.unwrap().scale, 3.0);
        // A Strength linked to something that is not constant is not a scale (1, omitted).
        let geo = add(&mut m, "ShaderNodeNewGeometry");
        let sep = add(&mut m, "ShaderNodeSeparateXYZ");
        link(&mut m, (geo, "Position"), (sep, "Vector"));
        link_at(&mut m, (sep, 0), (nm, 0));
        assert_eq!(lower_m(&m, &images).normal.unwrap().scale, 1.0);
        // No texture: no normal map.
        let mut m = mat("Flat");
        let nm = add(&mut m, "ShaderNodeNormalMap");
        link(&mut m, (nm, "Normal"), (BSDF, "Normal"));
        assert!(lower_m(&m, &images).normal.is_none());
    }
}
