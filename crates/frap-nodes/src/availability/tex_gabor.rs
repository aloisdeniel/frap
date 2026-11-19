// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeTexGabor` socket availability.
//!
//! Verified on 5.2.4: `gabor_type='2D'` enables the float `Orientation 2D`, `3D` the vector
//! `Orientation 3D` (both named "Orientation").

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_tex_gabor.cc
// (`sh_node_tex_gabor_declare`, `.available(…)`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let three_d = node.prop_str("gabor_type") == "3D";
    node.set_input_enabled("Orientation 2D", !three_d);
    node.set_input_enabled("Orientation 3D", three_d);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static TEX_GABOR: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeTexGabor",
    update,
};
