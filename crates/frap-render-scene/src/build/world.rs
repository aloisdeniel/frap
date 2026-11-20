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
//! lights with `frap_render::eevee::ibl::world_lighting` (WP 5.2). An Environment Texture
//! without an image is Blender's missing-texture magenta × strength
//! (`node_tex_environment_empty`, verified on 5.2.4). Anything else falls back to `World.color`
//! at strength 1. A scene without a world renders black.
//!
//! `RWorld::Hdri::rotation` is the Mapping node's Z rotation in the sense of the `POINT`
//! transform: a world direction `d` looks the environment up at `Rz(rotation) · d`.
//!
//! [`warnings`] lists what the EEVEE-like render of a world leaves out (the parts of the graph
//! `RWorld` cannot represent), for `render.render` to report.

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
    linked_output(tree, node, socket).map(|(n, _)| n)
}

/// [`linked_from`] with the identifier of the output socket the link leaves from.
fn linked_output<'a>(tree: &'a NodeTree, node: &Node, socket: &str) -> Option<(&'a Node, &'a str)> {
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
        return linked_output(tree, from, "Input");
    }
    Some((from, link.from_socket.as_str()))
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

/// The Background node the world's output shows, if any.
fn background_node(tree: &NodeTree) -> Option<&Node> {
    let bg = linked_from(tree, output_node(tree)?, "Surface")?;
    (bg.bl_idname == "ShaderNodeBackground").then_some(bg)
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
    let Some(bg) = background_node(tree) else {
        return fallback;
    };
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
                // `node_tex_environment_empty`: magenta.
                return RWorld::Color {
                    rgb: [1.0, 0.0, 1.0],
                    strength,
                };
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

fn enum_prop<'a>(n: &'a Node, name: &str, default: &'a str) -> &'a str {
    match prop(n, name) {
        Some(Value::Enum(s)) | Some(Value::Str(s)) => s.as_str(),
        _ => default,
    }
}

fn vector_input(node: &Node, socket: &str) -> Option<[f32; 3]> {
    match input_value(node, socket)? {
        SocketValue::Vector(v) => Some([v[0], v[1], v[2]]),
        SocketValue::Rotation(v) => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

/// What the EEVEE-like render of world `w` leaves out (one WARNING text each): the parts of
/// the world graph that `RWorld` does not represent, so the render differs from Blender's.
/// Empty for the recognised graphs ([`world`]).
pub fn warnings(w: Option<&World>) -> Vec<String> {
    let mut out = vec![];
    let Some(w) = w else { return out };
    let tree = &w.node_tree;
    let Some(bg) = background_node(tree) else {
        if output_node(tree).is_some_and(|o| linked_from(tree, o, "Surface").is_some()) {
            out.push(
                "frap: the world's surface is not a Background node; rendered with World.color"
                    .to_owned(),
            );
        }
        return out;
    };
    if linked_from(tree, bg, "Strength").is_some() {
        out.push(
            "frap: the world Background's Strength input is linked; rendered at strength 1"
                .to_owned(),
        );
    }
    let Some(env) = linked_from(tree, bg, "Color") else {
        return out;
    };
    if env.bl_idname != "ShaderNodeTexEnvironment" {
        out.push(format!(
            "frap: the world Background's Color comes from a '{}' node, which frap does not \
             render; rendered with World.color",
            env.bl_idname
        ));
        return out;
    }
    let projection = enum_prop(env, "projection", "EQUIRECTANGULAR");
    if projection != "EQUIRECTANGULAR" {
        out.push(format!(
            "frap: the world Environment Texture's projection '{projection}' is not rendered \
             yet; rendered as 'EQUIRECTANGULAR'"
        ));
    }
    let interpolation = enum_prop(env, "interpolation", "Linear");
    if interpolation != "Linear" {
        out.push(format!(
            "frap: the world Environment Texture's interpolation '{interpolation}' is not \
             rendered yet; rendered as 'Linear'"
        ));
    }
    let Some((mut vec_node, mut vec_socket)) = linked_output(tree, env, "Vector") else {
        return out;
    };
    if vec_node.bl_idname == "ShaderNodeMapping" {
        let mapping = vec_node;
        let rot = vector_input(mapping, "Rotation").unwrap_or([0.0; 3]);
        let mut left_out = vec![];
        if linked_from(tree, mapping, "Rotation").is_some() {
            left_out.push("a linked Rotation");
        } else if rot[0] != 0.0 || rot[1] != 0.0 {
            left_out.push("the X and Y rotation");
        }
        if linked_from(tree, mapping, "Location").is_some()
            || vector_input(mapping, "Location").is_some_and(|v| v != [0.0; 3])
        {
            left_out.push("the location");
        }
        if linked_from(tree, mapping, "Scale").is_some()
            || vector_input(mapping, "Scale").is_some_and(|v| v != [1.0; 3])
        {
            left_out.push("the scale");
        }
        if !left_out.is_empty() {
            out.push(format!(
                "frap: the world Mapping node's {} is not rendered yet (only its Z rotation)",
                left_out.join(", ")
            ));
        }
        match linked_output(tree, mapping, "Vector") {
            Some((n, s)) => (vec_node, vec_socket) = (n, s),
            None => {
                out.push(
                    "frap: the world Mapping node's Vector input is not linked: Blender looks \
                     up one texel of the environment (a single-colour world), frap renders the \
                     whole environment; link Texture Coordinate > Generated into it"
                        .to_owned(),
                );
                return out;
            }
        }
    }
    let generated = vec_node.bl_idname == "ShaderNodeTexCoord" && vec_socket == "Generated";
    if !generated {
        let what = if vec_node.bl_idname == "ShaderNodeTexCoord" {
            format!("Texture Coordinate > {vec_socket}")
        } else {
            format!("a '{}' node", vec_node.bl_idname)
        };
        out.push(format!(
            "frap: the world Environment Texture's vector comes from {what}, which frap does \
             not render; rendered with the view direction (Texture Coordinate > Generated)"
        ));
    }
    out
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

    fn node(uid: u32, bl_idname: &'static str, inputs: Vec<Socket>, outputs: &[&str]) -> Node {
        Node {
            uid,
            name: bl_idname.into(),
            label: String::new(),
            bl_idname,
            location: [0.0, 0.0],
            width: 140.0,
            parent: None,
            mute: false,
            hide: false,
            inputs,
            outputs: outputs
                .iter()
                .map(|o| socket(o, SocketType::Vector, SocketValue::None))
                .collect(),
            props: PropBag::default(),
        }
    }

    fn link(from: u32, from_socket: &str, to: u32, to_socket: &str) -> Link {
        Link {
            from_node: from,
            from_socket: from_socket.into(),
            to_node: to,
            to_socket: to_socket.into(),
        }
    }

    /// Output ← Background (strength 1.5) ← Environment Texture (image `img`) ← Mapping
    /// (rotation `rot`) ← Texture Coordinate > Generated.
    fn hdri_world(img: Option<frap_rna::IdAny>, rot: [f32; 4]) -> World {
        let v = |n: &str, x: [f32; 4]| socket(n, SocketType::Vector, SocketValue::Vector(x));
        let mut w = World::new("World");
        let out = node(
            1,
            "ShaderNodeOutputWorld",
            vec![socket("Surface", SocketType::Shader, SocketValue::None)],
            &[],
        );
        let bg = node(
            2,
            "ShaderNodeBackground",
            vec![
                socket("Color", SocketType::Rgba, SocketValue::Color([0.2; 4])),
                socket("Strength", SocketType::Value, SocketValue::Float(1.5)),
            ],
            &["Background"],
        );
        let mut env = node(
            3,
            "ShaderNodeTexEnvironment",
            vec![v("Vector", [0.0; 4])],
            &["Color"],
        );
        env.props.values.insert("image", Value::Id(img));
        let mut mapping = mapping(4, "POINT", rot);
        mapping.inputs.insert(0, v("Vector", [0.0; 4]));
        mapping.inputs.push(v("Location", [0.0; 4]));
        mapping.inputs.push(v("Scale", [1.0, 1.0, 1.0, 0.0]));
        let coords = node(
            5,
            "ShaderNodeTexCoord",
            vec![],
            &["Generated", "Normal", "Window"],
        );
        w.node_tree.nodes = vec![out, bg, env, mapping, coords];
        w.node_tree.links = vec![
            link(2, "Background", 1, "Surface"),
            link(3, "Color", 2, "Color"),
            link(4, "Vector", 3, "Vector"),
            link(5, "Generated", 4, "Vector"),
        ];
        w
    }

    #[test]
    fn environment_texture_worlds() {
        let img = IdRef::<ImageId>::from_key(frap_rna::IdKey::next());
        let mut seen = vec![];
        let mut key = |i: IdRef<ImageId>| {
            seen.push(i);
            ImageKey(9)
        };
        let w = hdri_world(Some(img.any()), [0.0, 0.0, 0.5, 0.0]);
        assert_eq!(
            world(Some(&w), &mut key),
            RWorld::Hdri {
                image: ImageKey(9),
                rotation: 0.5,
                strength: 1.5
            }
        );
        assert!(warnings(Some(&w)).is_empty(), "{:?}", warnings(Some(&w)));
        // No image: Blender's magenta × strength (`node_tex_environment_empty`).
        let w = hdri_world(None, [0.0; 4]);
        assert_eq!(
            world(Some(&w), &mut key),
            RWorld::Color {
                rgb: [1.0, 0.0, 1.0],
                strength: 1.5
            }
        );
        assert_eq!(seen, vec![img]);
        assert!(warnings(None).is_empty());
    }

    #[test]
    fn warnings_name_what_the_render_leaves_out() {
        let img = Some(IdRef::<ImageId>::from_key(frap_rna::IdKey::next()).any());
        let has = |w: &World, needle: &str| {
            let ws = warnings(Some(w));
            assert!(
                ws.iter().any(|m| m.contains(needle)),
                "{needle:?} in {ws:?}"
            );
            assert!(ws.iter().all(|m| m.starts_with("frap: ")), "{ws:?}");
        };
        // X/Y rotation, location and scale of the Mapping node.
        let mut w = hdri_world(img, [0.1, 0.0, 0.5, 0.0]);
        has(&w, "X and Y rotation");
        w.node_tree.nodes[3].inputs[2].default_value = SocketValue::Vector([0.0, 0.0, 1.0, 0.0]);
        w.node_tree.nodes[3].inputs[3].default_value = SocketValue::Vector([2.0, 1.0, 1.0, 0.0]);
        has(&w, "the location, the scale");
        // Mapping without a Vector link: Blender shows one texel.
        let mut w = hdri_world(img, [0.0; 4]);
        w.node_tree.links.pop();
        has(&w, "Vector input is not linked");
        // Another coordinate source.
        let mut w = hdri_world(img, [0.0; 4]);
        w.node_tree.links[3].from_socket = "Window".into();
        has(&w, "Texture Coordinate > Window");
        // Projection and interpolation.
        let mut w = hdri_world(img, [0.0; 4]);
        let env = &mut w.node_tree.nodes[2].props.values;
        env.insert("projection", Value::Enum("MIRROR_BALL".into()));
        env.insert("interpolation", Value::Enum("Closest".into()));
        has(&w, "projection 'MIRROR_BALL'");
        has(&w, "interpolation 'Closest'");
        // An unsupported colour source.
        let mut w = hdri_world(img, [0.0; 4]);
        w.node_tree.nodes[2].bl_idname = "ShaderNodeTexSky";
        has(&w, "'ShaderNodeTexSky' node");
        // A plain Background world has nothing to report.
        let mut w = hdri_world(img, [0.0; 4]);
        w.node_tree.links.retain(|l| l.to_node == 1);
        assert!(warnings(Some(&w)).is_empty());
    }
}
