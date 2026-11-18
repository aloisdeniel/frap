// SPDX-License-Identifier: GPL-3.0-or-later
//! Emission: Principled BSDF `Emission Color` × `Emission Strength` → `emissiveFactor`,
//! `emissiveTexture` and `KHR_materials_emissive_strength` (ARCHITECTURE §16.2).
//!
//! Lowering ports `export_emission_factor` (`io_scene_gltf2/blender/exp/material/extensions/
//! emission.py`) and the strength split of `gather_material` (`materials.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0): factor = colour (constant, or the constant of a Mix
//! MULTIPLY; white when only a texture is linked; black when nothing) × the constant
//! strength; when a channel exceeds 1 the largest channel becomes `emissive_strength` and the
//! factor is divided by it. Computed in f64 like Python, so lowering a raised graph gives the
//! same f32 values.
//!
//! Raising ports `emission()` (`imp/pbrMetallicRoughness.py`): a grey factor folds into the
//! strength (colour white), then `color_factor_and_texture` ("Emissive Factor" Mix, Image
//! Texture labelled "EMISSIVE").

use super::base_color::{color_info, describe};
use crate::graph::{Src, unrecognised};
use crate::{ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let src = cx.src(p, "Emission Color");
    let (factor, rest) = color_info(cx, src);
    let linked = matches!(src, Src::Linked(..));
    let mut f: [f64; 3] = match factor {
        Some(c) => c.map(f64::from),
        None if linked => [1.0; 3],
        None => [0.0; 3],
    };
    let strength_src = cx.src(p, "Emission Strength");
    match cx.const_f32(strength_src) {
        Some(s) => f = f.map(|c| c * f64::from(s)),
        None => unrecognised(
            &mut out.diagnostics,
            format!(
                "Emission Strength: unsupported node setup at {}; strength ignored",
                describe(strength_src)
            ),
        ),
    }
    let max = f.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut strength = 1.0;
    if max > 1.0 {
        strength = max;
        f = f.map(|c| c / max);
    }
    out.emissive.factor = f.map(|c| c as f32);
    out.emissive_strength = strength as f32;
    out.emissive.texture = None;
    if let Some(rest) = rest {
        match cx.image_behind(rest) {
            Some((tex, ChannelSel::Rgb)) => {
                out.emissive.texture = cx.tex_ref(tex, ChannelSel::Rgb, &mut out.diagnostics);
            }
            _ => unrecognised(
                &mut out.diagnostics,
                format!(
                    "Emission Color: unsupported node setup at {}; exported without a texture",
                    describe(rest)
                ),
            ),
        }
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
