// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeVectorMath` socket availability. Port of `node_shader_update_vector_math`
//! (`source/blender/nodes/shader/nodes/node_shader_vector_math.cc`, Blender 5.2,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4 for every `operation` (inputs `Vector`, `Vector_001`, `Vector_002`,
//! `Scale`; outputs `Vector`, `Value`): the second vector is off for the unary operations and
//! SCALE; the third is on only for MULTIPLY_ADD, FACEFORWARD and WRAP; `Scale` only for SCALE
//! and REFRACT; the `Value` output replaces `Vector` for DOT_PRODUCT, DISTANCE and LENGTH.
//! Socket names do not change (REFRACT's `Scale` is labelled "IOR" in the UI only).

use crate::{Node, NodeAvailability};

/// Operations that read only the first vector.
const UNARY: [&str; 11] = [
    "LENGTH",
    "NORMALIZE",
    "ABSOLUTE",
    "SIGN",
    "ROUND",
    "FLOOR",
    "CEIL",
    "FRACTION",
    "SINE",
    "COSINE",
    "TANGENT",
];

/// Operations that read the third vector.
const TERNARY: [&str; 3] = ["MULTIPLY_ADD", "FACEFORWARD", "WRAP"];

/// Operations whose result is the `Value` output.
const SCALAR_OUT: [&str; 3] = ["DOT_PRODUCT", "DISTANCE", "LENGTH"];

fn update(node: &mut Node) {
    let op = node.prop_str("operation").to_owned();
    let op = op.as_str();
    let scale = matches!(op, "SCALE" | "REFRACT");
    node.set_input_enabled("Vector", true);
    node.set_input_enabled("Vector_001", !UNARY.contains(&op) && op != "SCALE");
    node.set_input_enabled("Vector_002", TERNARY.contains(&op));
    node.set_input_enabled("Scale", scale);
    let scalar = SCALAR_OUT.contains(&op);
    node.set_output_enabled("Vector", !scalar);
    node.set_output_enabled("Value", scalar);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static VECTOR_MATH: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeVectorMath",
    update,
};
