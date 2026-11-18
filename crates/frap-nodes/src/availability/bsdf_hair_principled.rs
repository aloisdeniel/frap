// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeBsdfHairPrincipled` socket availability.
//!
//! Verified on 5.2.4 for every (`model`, `parametrization`) pair: the parametrization picks
//! `Color` (COLOR), `Melanin`/`Melanin Redness`/`Tint`/`Random Color` (MELANIN) or
//! `Absorption Coefficient` (ABSORPTION); CHIANG enables `Radial Roughness` and `Coat`, HUANG
//! `Aspect Ratio` and the three lobe factors (identifiers `R lobe`, `TT lobe`, `TRT lobe`,
//! named "Reflection", "Transmission", "Secondary Reflection"). The internal `Weight` stays off.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_bsdf_hair_principled.cc
// (`node_shader_update_hair_principled`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let param = node.prop_str("parametrization").to_owned();
    let huang = node.prop_str("model") == "HUANG";
    let melanin = param == "MELANIN";
    node.set_input_enabled("Color", param == "COLOR");
    node.set_input_enabled("Melanin", melanin);
    node.set_input_enabled("Melanin Redness", melanin);
    node.set_input_enabled("Tint", melanin);
    node.set_input_enabled("Random Color", melanin);
    node.set_input_enabled("Absorption Coefficient", param == "ABSORPTION");
    node.set_input_enabled("Radial Roughness", !huang);
    node.set_input_enabled("Coat", !huang);
    node.set_input_enabled("Aspect Ratio", huang);
    for lobe in ["R lobe", "TT lobe", "TRT lobe"] {
        node.set_input_enabled(lobe, huang);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static BSDF_HAIR_PRINCIPLED: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeBsdfHairPrincipled",
    update,
};
