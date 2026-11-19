// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeTexWhiteNoise` socket availability.
//!
//! Verified on 5.2.4: `Vector` is off in 1D, `W` on in 1D and 4D; both outputs always on.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_tex_white_noise.cc
// (`sh_node_tex_white_noise_declare`, `.available(…)`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let dims = node.prop_str("noise_dimensions").to_owned();
    node.set_input_enabled("Vector", dims != "1D");
    node.set_input_enabled("W", matches!(dims.as_str(), "1D" | "4D"));
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static TEX_WHITE_NOISE: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeTexWhiteNoise",
    update,
};
