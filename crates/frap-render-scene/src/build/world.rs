// SPDX-License-Identifier: GPL-3.0-or-later
//! `World` → [`RWorld`] (the EEVEE-like engine's background; Workbench uses
//! `RSettings::workbench_background`).
//!
//! The active world output is chosen like `ntreeShaderOutputNode(ntree, SHD_OUTPUT_EEVEE)`
//! (blenkernel `node_shader_tree.cc`): an output whose target is EEVEE beats one whose target
//! is ALL; between equals the active output (`is_active_output`) wins, else the first one.
//! Recognised graphs: Background → output (`Color { Color, Strength }`) and Environment
//! Texture (optionally behind a Mapping node, whose Z rotation is kept) → Background
//! (`Hdri`). Anything else falls back to `World.color` at strength 1. A scene without a
//! world renders black.

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
                .and_then(|m| match input_value(m, "Rotation") {
                    Some(SocketValue::Vector(v)) => Some(v[2]),
                    Some(SocketValue::Rotation(v)) => Some(v[2]),
                    _ => None,
                })
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
