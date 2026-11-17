// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeMix` socket availability. Port of `sh_node_mix_update`
//! (`source/blender/nodes/shader/nodes/node_shader_mix.cc`, Blender 5.2, GPL-2.0-or-later).
//!
//! Verified on 5.2.4: `data_type` selects the `A_*`/`B_*`/`Result_*` sockets of its type
//! (FLOAT, VECTOR, RGBA; shader trees have no ROTATION mix); `Factor_Vector` replaces
//! `Factor_Float` only for `data_type='VECTOR'` with `factor_mode='NON_UNIFORM'`.

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let data_type = node.prop_str("data_type").to_owned();
    let non_uniform = data_type == "VECTOR" && node.prop_str("factor_mode") == "NON_UNIFORM";
    node.set_input_enabled("Factor_Float", !non_uniform);
    node.set_input_enabled("Factor_Vector", non_uniform);
    for (suffix, ty) in [
        ("Float", "FLOAT"),
        ("Vector", "VECTOR"),
        ("Color", "RGBA"),
        ("Rotation", "ROTATION"),
    ] {
        let on = data_type == ty;
        node.set_input_enabled(&format!("A_{suffix}"), on);
        node.set_input_enabled(&format!("B_{suffix}"), on);
        node.set_output_enabled(&format!("Result_{suffix}"), on);
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static MIX: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeMix",
    update,
};
