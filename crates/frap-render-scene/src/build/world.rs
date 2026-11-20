// SPDX-License-Identifier: GPL-3.0-or-later
//! `World` → [`RWorld`] (the EEVEE-like engine's background; Workbench uses
//! `RSettings::workbench_background`).
//!
//! The active world output is chosen like `ntreeShaderOutputNode(ntree, SHD_OUTPUT_EEVEE)`
//! (blenkernel `node_shader_tree.cc`): an output whose target is EEVEE beats one whose target
//! is ALL; between equals the active output (`is_active_output`) wins, else the first one.
//! Recognised graphs: Background → output (`Color { Color, Strength }`) and Environment
//! Texture (optionally behind a Mapping node, whose Z rotation is kept, mirrored for the
//! `TEXTURE` type: [`mapping_z_rotation`]) → Background (`Hdri`), which the EEVEE-like engine
//! lights with `frap_render::eevee::ibl::world_lighting` (WP 5.2). Anything else falls back to
//! `World.color` at strength 1. A scene without a world renders black.

use frap_nodes::{Node, NodeTree, SocketValue};
use frap_rna::{IdRef, ImageId, Value};
use frap_scene::id::World;

use crate::types::{ImageKey, RWorld};

fn prop<'a>(n: &'a Node, name: &str) -> Option<&'a Value> {
    n.props.values.get(name)
}

fn output_target(n: &Node) -> &str {
    match prop(n, "target") {
        Some(Value::Enum(s)) | Some(Value::Str(s)) => s.as_str(),
        _ => "ALL",
    }
}

fn is_active_output(n: &Node) -> bool {
    matches!(prop(n, "is_active_output"), Some(Value::Bool(true)))
}

/// `ntreeShaderOutputNode` for world outputs and the EEVEE target.
pub fn output_node(tree: &NodeTree) -> Option<&Node> {
    let mut out: Option<&Node> = None;
    for n in &tree.nodes {
        if n.bl_idname != "ShaderNodeOutputWorld" {
            continue;
        }
        let t = output_target(n);
        out = match (out, t) {
            (None, "ALL" | "EEVEE") => Some(n),
            (Some(o), "ALL") if output_target(o) == "ALL" => {
                if is_active_output(n) && !is_active_output(o) {
                    Some(n)
                } else {
                    Some(o)
                }
            }
            (Some(o), "EEVEE") => {
                if output_target(o) == "ALL" || (is_active_output(n) && !is_active_output(o)) {
                    Some(n)
                } else {
                    Some(o)
                }
            }
            (o, _) => o,
        };
    }
    out
}

/// The node linked into input `socket` (first enabled input of that name) of `node`, through
/// reroutes; muted nodes count as unlinked.
pub fn linked_from<'a>(tree: &'a NodeTree, node: &Node, socket: &str) -> Option<&'a Node> {
    let input = node.inputs.iter().find(|s| s.enabled && s.name == socket)?;
    let link = tree
        .links
        .iter()
        .find(|l| l.to_node == node.uid && l.to_socket == input.identifier)?;
    let from = tree.nodes.iter().find(|n| n.uid == link.from_node)?;
    if from.mute {
        return None;
    }
    if from.bl_idname == "NodeReroute" {
        return linked_from(tree, from, "Input");
    }
    Some(from)
}

fn input_value<'a>(node: &'a Node, socket: &str) -> Option<&'a SocketValue> {
    node.inputs
        .iter()
        .find(|s| s.enabled && s.name == socket)
        .map(|s| &s.default_value)
}

fn float_input(node: &Node, socket: &str, default: f32) -> f32 {
    match input_value(node, socket) {
        Some(SocketValue::Float(v)) => *v,
        _ => default,
    }
}

/// The Z rotation a Mapping node applies to the environment lookup (`RWorld::Hdri::rotation`,
/// `lookup = Rz(rotation) · direction`). `POINT`, `VECTOR` and `NORMAL` rotate the vector by the
/// Euler angles (`mapping_point`/`mapping_vector`/`mapping_normal` in
/// blender/source/blender/gpu/shaders/material/gpu_shader_material_mapping.glsl); `TEXTURE` is
/// the inverse transform, so it turns the other way. A linked Rotation counts as 0; the X and
/// Y angles, the location and the scale are not represented in `RWorld` (WP 5.2).
pub fn mapping_z_rotation(tree: &NodeTree, mapping: &Node) -> f32 {
    if linked_from(tree, mapping, "Rotation").is_some() {
        return 0.0;
    }
    let z = match input_value(mapping, "Rotation") {
        Some(SocketValue::Vector(v)) => v[2],
        Some(SocketValue::Rotation(v)) => v[2],
        _ => 0.0,
    };
    let inverse = matches!(
        prop(mapping, "vector_type"),
        Some(Value::Enum(s)) | Some(Value::Str(s)) if s == "TEXTURE"
    );
    if inverse { -z } else { z }
}

/// The world of the render; `image` resolves an Environment Texture's image to its key.
pub fn world(w: Option<&World>, image: &mut dyn FnMut(IdRef<ImageId>) -> ImageKey) -> RWorld {
    let Some(w) = w else {
        return RWorld::Color {
            rgb: [0.0; 3],
            strength: 1.0,
        };
    };
    let fallback = RWorld::Color {
        rgb: w.color,
        strength: 1.0,
    };
    let tree = &w.node_tree;
    let Some(out) = output_node(tree) else {
        return fallback;
    };
    let Some(bg) = linked_from(tree, out, "Surface") else {
        return fallback;
    };
    if bg.bl_idname != "ShaderNodeBackground" {
        return fallback;
    }
    let strength = if linked_from(tree, bg, "Strength").is_some() {
        1.0
    } else {
        float_input(bg, "Strength", 1.0)
    };
    match linked_from(tree, bg, "Color") {
        None => match input_value(bg, "Color") {
            Some(SocketValue::Color(c)) => RWorld::Color {
                rgb: [c[0], c[1], c[2]],
                strength,
            },
            _ => fallback,
        },
        Some(env) if env.bl_idname == "ShaderNodeTexEnvironment" => {
            let img = match prop(env, "image") {
                Some(Value::Id(Some(id))) => IdRef::<ImageId>::from_any(*id),
                _ => None,
            };
            let Some(img) = img else {
                return fallback;
            };
            let rotation = linked_from(tree, env, "Vector")
                .filter(|m| m.bl_idname == "ShaderNodeMapping")
                .map(|m| mapping_z_rotation(tree, m))
                .unwrap_or(0.0);
            RWorld::Hdri {
                image: image(img),
                rotation,
                strength,
            }
        }
        Some(_) => fallback,
    }
}

#[cfg(test)]
mod tests {
    use frap_nodes::{Link, PropBag, Socket, SocketType};

    use super::*;

    fn socket(name: &str, ty: SocketType, v: SocketValue) -> Socket {
        Socket {
            name: name.into(),
            identifier: name.into(),
            ty,
            enabled: true,
            hide_value: false,
            default_value: v,
        }
    }

    fn mapping(uid: u32, vector_type: &str, rotation: [f32; 4]) -> Node {
        let mut props = PropBag::default();
        props
            .values
            .insert("vector_type", Value::Enum(vector_type.into()));
        Node {
            uid,
            name: "Mapping".into(),
            label: String::new(),
            bl_idname: "ShaderNodeMapping",
            location: [0.0, 0.0],
            width: 140.0,
            parent: None,
            mute: false,
            hide: false,
            inputs: vec![socket(
                "Rotation",
                SocketType::Vector,
                SocketValue::Vector(rotation),
            )],
            outputs: vec![socket("Vector", SocketType::Vector, SocketValue::None)],
            props,
        }
    }

    #[test]
    fn mapping_types_turn_the_lookup() {
        let mut tree = NodeTree::new("Shader Nodetree", frap_nodes::NodeTreeType::Shader, true);
        for (ty, want) in [
            ("POINT", 0.5),
            ("VECTOR", 0.5),
            ("NORMAL", 0.5),
            ("TEXTURE", -0.5),
        ] {
            let m = mapping(1, ty, [0.3, 0.2, 0.5, 0.0]);
            assert_eq!(mapping_z_rotation(&tree, &m), want, "{ty}");
        }
        // A linked Rotation is not a constant: no rotation.
        let m = mapping(1, "POINT", [0.0, 0.0, 0.5, 0.0]);
        tree.nodes = vec![m.clone(), mapping(2, "POINT", [0.0; 4])];
        tree.links = vec![Link {
            from_node: 2,
            from_socket: "Vector".into(),
            to_node: 1,
            to_socket: "Rotation".into(),
        }];
        assert_eq!(mapping_z_rotation(&tree, &m), 0.0);
    }
}
