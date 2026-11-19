// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeTexNoise` socket availability.
//!
//! Verified on 5.2.4 for every (`noise_dimensions`, `noise_type`) pair: `Vector` is off in 1D,
//! `W` on in 1D and 4D; `Offset` is off for MULTIFRACTAL and FBM; `Gain` is on only for
//! HYBRID_MULTIFRACTAL and RIDGED_MULTIFRACTAL. `normalize` changes no socket.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_tex_noise.cc
// (`sh_node_tex_noise_declare`, `.available(…)`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let dims = node.prop_str("noise_dimensions").to_owned();
    let ty = node.prop_str("noise_type").to_owned();
    node.set_input_enabled("Vector", dims != "1D");
    node.set_input_enabled("W", matches!(dims.as_str(), "1D" | "4D"));
    node.set_input_enabled("Offset", !matches!(ty.as_str(), "MULTIFRACTAL" | "FBM"));
    node.set_input_enabled(
        "Gain",
        matches!(ty.as_str(), "HYBRID_MULTIFRACTAL" | "RIDGED_MULTIFRACTAL"),
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static TEX_NOISE: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeTexNoise",
    update,
};
