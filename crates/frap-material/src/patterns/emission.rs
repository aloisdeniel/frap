// SPDX-License-Identifier: GPL-3.0-or-later
//! Emission: `emissiveFactor` and `emissiveTexture` (ARCHITECTURE §16.2), from a dedicated
//! Emission shader linked to the output if there is one (it supersedes the Principled BSDF,
//! whose `Emission Color` socket always exists), else the Principled `Emission Color` ×
//! `Emission Strength`. The split into `KHR_materials_emissive_strength` is
//! `emission_strength.rs`.
//!
//! Lowering ports `export_emission_factor` / `export_emission_texture`
//! (`io_scene_gltf2/blender/exp/material/extensions/emission.py`, glTF-Blender-IO 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors): factor = `get_factor` of the colour socket (a
//! constant, or the single constant of a Mix MULTIPLY; white when only something else is
//! linked; black when nothing) × `get_factor` of the strength socket (`Strength` of the
//! Emission node, `Emission Strength` of the Principled BSDF; ignored when it is not a
//! factor); the texture is the first Image Texture found behind the colour socket.
//! Computed in f64 like Python.
//!
//! Raising ports `emission()` (`imp/pbrMetallicRoughness.py`): a grey factor folds into the
//! strength (colour white), then `color_factor_and_texture` ("Emissive Factor" Mix, Image
//! Texture labelled "EMISSIVE").

use crate::graph::{In, Konst};
use crate::{ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// The emission colour socket (`get_socket(tree, "Emissive")`, else the glTF Material
/// Output's `EmissiveFactor`).
pub(crate) fn emissive_socket<'a>(cx: &LowerCx<'a>) -> Option<In<'a>> {
    cx.get_socket("Emissive")
        .or_else(|| cx.gltf_output_input("EmissiveFactor"))
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    if out.unlit {
        return;
    }
    out.emissive.factor = [0.0; 3];
    out.emissive_strength = 1.0;
    out.emissive.texture = None;
    let Some(socket) = emissive_socket(cx) else {
        return;
    };
    let linked = cx.peek_back(socket).is_some();
    let mut f: [f64; 3] = match cx.get_factor(socket) {
        Some(Konst::Rgb(c) | Konst::Vector(c)) => c.map(f64::from),
        Some(Konst::Value(v)) => [f64::from(v); 3],
        None if linked => [1.0; 3],
        None => [0.0; 3],
    };
    let strength_name = match socket.node.bl_idname {
        "ShaderNodeEmission" => "Strength",
        _ => "Emission Strength",
    };
    if let Some(s) = In::named(socket.node, strength_name)
        && let Some(Konst::Value(v)) = cx.get_factor(s)
    {
        f = f.map(|c| c * f64::from(v));
    }
    out.emissive.factor = f.map(|c| c as f32);
    // `export_emission_texture`: gathered from the colour socket (`gather_texture_info`).
    let emissive_tex = cx
        .get_socket("Emissive")
        .or_else(|| cx.gltf_output_input("Emissive"));
    if let Some(hit) = emissive_tex.and_then(|s| cx.texture_from(s)) {
        out.emissive.texture = cx.tex_ref(hit.node, ChannelSel::Rgb, &mut out.diagnostics);
    }
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(bsdf) = cx.bsdf else { return };
    let mut factor = lm.emissive.factor;
    let mut strength = f64::from(lm.emissive_strength);
    if factor[0] == factor[1] && factor[1] == factor[2] {
        strength *= f64::from(factor[0]);
        factor = [1.0; 3];
    }
    let socket = cx.input(bsdf, "Emission Color");
    let loc = cx.location("emission");
    cx.color_factor_and_texture(
        loc,
        "Emissive",
        socket,
        factor,
        lm.emissive.texture.as_ref(),
    );
    cx.set_input(
        bsdf,
        "Emission Strength",
        frap_nodes::SocketValue::Float(strength as f32),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "emission",
    order: 10,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 130,
    key: "emission",
    takes_space: |lm| lm.emissive.texture.is_some(),
};
