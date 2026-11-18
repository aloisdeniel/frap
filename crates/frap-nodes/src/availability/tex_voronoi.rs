// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeTexVoronoi` socket availability.
//!
//! Verified on 5.2.4 for every (`voronoi_dimensions`, `distance`, `feature`) triple (80
//! cases): `Vector` is off in 1D and `W` on in 1D/4D; N_SPHERE_RADIUS turns off `Detail`,
//! `Roughness`, `Lacunarity` and every output but `Radius`; `Smoothness` is SMOOTH_F1's;
//! `Exponent` needs MINKOWSKI, more than one dimension and a feature other than
//! DISTANCE_TO_EDGE / N_SPHERE_RADIUS; DISTANCE_TO_EDGE keeps only the `Distance` output.
//! `normalize` changes no socket.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_tex_voronoi.cc
// (`sh_node_tex_voronoi_declare`, `.available(…)`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let dims = node.prop_str("voronoi_dimensions").to_owned();
    let feature = node.prop_str("feature").to_owned();
    let minkowski = node.prop_str("distance") == "MINKOWSKI";
    let one_d = dims == "1D";
    let w = matches!(dims.as_str(), "1D" | "4D");
    let radius = feature == "N_SPHERE_RADIUS";
    let edge = feature == "DISTANCE_TO_EDGE";
    node.set_input_enabled("Vector", !one_d);
    node.set_input_enabled("W", w);
    for s in ["Detail", "Roughness", "Lacunarity"] {
        node.set_input_enabled(s, !radius);
    }
    node.set_input_enabled("Smoothness", feature == "SMOOTH_F1");
    node.set_input_enabled("Exponent", minkowski && !one_d && !edge && !radius);
    node.set_output_enabled("Distance", !radius);
    node.set_output_enabled("Color", !edge && !radius);
    node.set_output_enabled("Position", !edge && !radius && !one_d);
    node.set_output_enabled("W", !edge && !radius && w);
    node.set_output_enabled("Radius", radius);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static TEX_VORONOI: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeTexVoronoi",
    update,
};
