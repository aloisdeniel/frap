// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeMath` socket availability. Port of `node_shader_update_math`
//! (`source/blender/nodes/shader/nodes/node_shader_math.cc`, Blender 5.2, GPL-2.0-or-later).
//!
//! Verified on 5.2.4 for every `operation`: unary operations enable only the first input,
//! ternary ones all three, the rest the first two.

use crate::{Node, NodeAvailability};

const UNARY: [&str; 21] = [
    "SQRT",
    "INVERSE_SQRT",
    "ABSOLUTE",
    "EXPONENT",
    "SIGN",
    "ROUND",
    "FLOOR",
    "CEIL",
    "TRUNC",
    "FRACT",
    "SINE",
    "COSINE",
    "TANGENT",
    "ARCSINE",
    "ARCCOSINE",
    "ARCTANGENT",
    "SINH",
    "COSH",
    "TANH",
    "RADIANS",
    "DEGREES",
];

const TERNARY: [&str; 5] = ["MULTIPLY_ADD", "COMPARE", "SMOOTH_MIN", "SMOOTH_MAX", "WRAP"];

fn update(node: &mut Node) {
    let op = node.prop_str("operation").to_owned();
    let unary = UNARY.contains(&op.as_str());
    let ternary = TERNARY.contains(&op.as_str());
    node.set_input_enabled("Value", true);
    node.set_input_enabled("Value_001", !unary);
    node.set_input_enabled("Value_002", ternary);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static MATH: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeMath",
    update,
};
