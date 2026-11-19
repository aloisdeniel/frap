// SPDX-License-Identifier: GPL-3.0-or-later
//! The "Smooth by Angle" geometry node group of Blender's Essentials asset library
//! (`datafiles/assets/nodes/geometry_nodes_essentials.blend`), which `object.shade_auto_smooth`
//! links into the file (WP 4.1).
//!
//! frap has no geometry-nodes evaluator: the group exists as data, identical to the linked
//! one (dumped from Blender 5.2.4: 14 nodes, 13 links, interface `Output_2` Mesh, `Input_0`
//! Mesh, `Input_1` Angle 30°, `Socket_1` Ignore Sharpness), and the NODES modifier evaluates it
//! natively, recognising it by **(library file name, group name)**
//! ([`is_smooth_by_angle`], `frap-modifiers/src/types/nodes.rs`), never by the name alone: a
//! local group named "Smooth by Angle" is not the built-in one.
//!
//! Linking, verified on 5.2.4 (`-b --factory-startup`): the first `shade_auto_smooth` adds two
//! `bpy.data.libraries` entries with the same file path, `geometry_nodes_essentials.blend`
//! and the packed archive `geometry_nodes_essentials.blend.001` (`is_archive`), and links the
//! group from the second one (`is_linked_packed`, read-only); later calls reuse it. frap's
//! library path is [`ESSENTIALS_FILEPATH`] (Blender's is inside its installation).

use frap_rna::{IdRef, LibraryId, Value};
use smol_str::SmolStr;

use crate::{
    InOut, InterfaceSocket, Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketType,
    SocketValue,
};

/// The group's name.
pub const NAME: &str = "Smooth by Angle";
/// The file name of the library the group is linked from.
pub const LIBRARY_BASENAME: &str = "geometry_nodes_essentials.blend";
/// Name of the packed archive library the group's `library` points at.
pub const ARCHIVE_LIBRARY_NAME: &str = "geometry_nodes_essentials.blend.001";
/// `Library.filepath` of both libraries (Blender: `<install>/datafiles/assets/nodes/…`; the
/// path suffix is what `is_smooth_by_angle_modifier` checks).
pub const ESSENTIALS_FILEPATH: &str =
    "<frap>/datafiles/assets/nodes/geometry_nodes_essentials.blend";
/// Identifier of the geometry input.
pub const INPUT_MESH: &str = "Input_0";
/// Identifier of the "Angle" input (`interface_inputs()[1]`, what `shade_auto_smooth` sets).
pub const INPUT_ANGLE: &str = "Input_1";
/// Identifier of the "Ignore Sharpness" input.
pub const INPUT_IGNORE_SHARPNESS: &str = "Socket_1";
/// Default of the "Angle" input (30°, as an `f32`).
pub const DEFAULT_ANGLE: f32 = std::f32::consts::FRAC_PI_6;
/// Description of the "Angle" input.
pub const ANGLE_DESCRIPTION: &str = "Maximum face angle for smooth edges";

/// The node types of the group that are not in the generated (shader) node table: a `.frap`
/// reader rebuilds them from their stored sockets and [`NODE_PROPS`].
pub const NODE_TYPES: &[&str] = &[
    "GeometryNodeSetShadeSmooth",
    "GeometryNodeInputMeshEdgeAngle",
    "GeometryNodeInputEdgeSmooth",
    "GeometryNodeInputShadeSmooth",
    "FunctionNodeCompare",
    "FunctionNodeBooleanMath",
];

/// The node properties those types store (`domain`, `operation`, `data_type`, `mode`).
pub const NODE_PROPS: &[&str] = &["domain", "operation", "data_type", "mode"];

/// Whether a group with this name, linked from a library with this file path, is the built-in
/// "Smooth by Angle" (`is_smooth_by_angle_modifier`: the library path ends in
/// `geometry_nodes_essentials.blend`, the group is named "Smooth by Angle").
pub fn is_smooth_by_angle(name: &str, library_filepath: Option<&str>) -> bool {
    let Some(path) = library_filepath else {
        return false;
    };
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name == NAME && base == LIBRARY_BASENAME
}

fn socket(name: &str, identifier: &str, ty: SocketType, v: SocketValue) -> Socket {
    Socket {
        name: name.into(),
        identifier: identifier.into(),
        ty,
        enabled: true,
        hide_value: false,
        default_value: v,
    }
}

fn geometry(name: &str, identifier: &str) -> Socket {
    socket(name, identifier, SocketType::Geometry, SocketValue::None)
}

fn boolean(name: &str, identifier: &str, v: bool) -> Socket {
    socket(name, identifier, SocketType::Boolean, SocketValue::Bool(v))
}

fn float(name: &str, identifier: &str, v: f32) -> Socket {
    socket(name, identifier, SocketType::Value, SocketValue::Float(v))
}

fn extend() -> Socket {
    socket("", "__extend__", SocketType::Custom, SocketValue::None)
}

fn props(values: &[(&'static str, &'static str)]) -> PropBag {
    PropBag {
        values: values
            .iter()
            .map(|(k, v)| (*k, Value::Enum(SmolStr::new_static(v))))
            .collect(),
    }
}

/// The group's interface (`interface.items_tree` order: the output first).
pub fn interface() -> Vec<InterfaceSocket> {
    let s = |name: &str, identifier: &str, in_out, socket_type, default_value| InterfaceSocket {
        name: name.into(),
        identifier: identifier.into(),
        in_out,
        socket_type,
        default_value,
    };
    vec![
        s(
            "Mesh",
            "Output_2",
            InOut::Output,
            SocketType::Geometry,
            SocketValue::None,
        ),
        s(
            "Mesh",
            INPUT_MESH,
            InOut::Input,
            SocketType::Geometry,
            SocketValue::None,
        ),
        s(
            "Angle",
            INPUT_ANGLE,
            InOut::Input,
            SocketType::Value,
            SocketValue::Float(DEFAULT_ANGLE),
        ),
        s(
            "Ignore Sharpness",
            INPUT_IGNORE_SHARPNESS,
            InOut::Input,
            SocketType::Boolean,
            SocketValue::Bool(false),
        ),
    ]
}

/// The linked group as Blender 5.2.4 links it, from `library` (the archive library).
pub fn build(library: IdRef<LibraryId>) -> NodeTree {
    let mut t = NodeTree::new(NAME, NodeTreeType::Geometry, false);
    t.header.library = Some(library);
    t.header.is_linked_packed = true;
    t.interface = interface();
    // Group Input sockets carry no values of their own (Blender hides the unused ones in the
    // editor, `socket.hide`, which frap does not store).
    let group_input = || {
        let mut outs = vec![
            geometry("Mesh", INPUT_MESH),
            float("Angle", INPUT_ANGLE, DEFAULT_ANGLE),
            boolean("Ignore Sharpness", INPUT_IGNORE_SHARPNESS, false),
            extend(),
        ];
        for s in &mut outs {
            s.default_value = SocketValue::None;
        }
        outs
    };
    let set_smooth = || {
        let mut sel = boolean("Selection", "Selection", true);
        sel.hide_value = true;
        vec![
            geometry("Mesh", "Geometry"),
            sel,
            boolean("Shade Smooth", "Shade Smooth", true),
        ]
    };
    let bool_math = || {
        vec![
            boolean("Boolean", "Boolean", false),
            boolean("Boolean", "Boolean_001", false),
        ]
    };
    let mut add = |name: &str,
                   bl_idname: &'static str,
                   location: [f32; 2],
                   inputs: Vec<Socket>,
                   outputs: Vec<Socket>,
                   props: PropBag| {
        let uid = t.next_node_uid;
        t.next_node_uid += 1;
        t.nodes.push(Node {
            uid,
            name: name.into(),
            label: String::new(),
            bl_idname,
            location,
            width: 140.0,
            parent: None,
            mute: false,
            hide: false,
            inputs,
            outputs,
            props,
        });
        uid
    };
    let set1 = add(
        "Set Shade Smooth",
        "GeometryNodeSetShadeSmooth",
        [120.0, -80.0],
        set_smooth(),
        vec![geometry("Mesh", "Geometry")],
        props(&[("domain", "EDGE")]),
    );
    let set2 = add(
        "Set Shade Smooth.001",
        "GeometryNodeSetShadeSmooth",
        [300.0, -80.0],
        set_smooth(),
        vec![geometry("Mesh", "Geometry")],
        props(&[("domain", "FACE")]),
    );
    let mut output_props = PropBag::default();
    output_props
        .values
        .insert("is_active_output", Value::Bool(true));
    let out = add(
        "Group Output",
        "NodeGroupOutput",
        [480.0, -80.0],
        vec![geometry("Mesh", "Output_2"), extend()],
        vec![],
        output_props,
    );
    let edge_angle = add(
        "Edge Angle",
        "GeometryNodeInputMeshEdgeAngle",
        [-440.0, -240.0],
        vec![],
        vec![
            float("Unsigned Angle", "Unsigned Angle", 0.0),
            float("Signed Angle", "Signed Angle", 0.0),
        ],
        PropBag::default(),
    );
    let gi = add(
        "Group Input",
        "NodeGroupInput",
        [-80.0, -80.0],
        vec![],
        group_input(),
        PropBag::default(),
    );
    let edge_smooth = add(
        "Is Edge Smooth",
        "GeometryNodeInputEdgeSmooth",
        [-260.0, -120.0],
        vec![],
        vec![boolean("Smooth", "Smooth", false)],
        PropBag::default(),
    );
    let shade_smooth = add(
        "Is Shade Smooth",
        "GeometryNodeInputShadeSmooth",
        [-439.519_17, -396.632_8],
        vec![],
        vec![boolean("Smooth", "Smooth", false)],
        PropBag::default(),
    );
    let compare = add(
        "Compare",
        "FunctionNodeCompare",
        [-260.0, -260.0],
        vec![float("A", "A", 0.0), float("B", "B", 0.0)],
        vec![boolean("Result", "Result", false)],
        props(&[
            ("operation", "LESS_EQUAL"),
            ("data_type", "FLOAT"),
            ("mode", "ELEMENT"),
        ]),
    );
    let and = add(
        "Boolean Math.001",
        "FunctionNodeBooleanMath",
        [-80.0, -260.0],
        bool_math(),
        vec![boolean("Boolean", "Boolean", false)],
        props(&[("operation", "AND")]),
    );
    let gi1 = add(
        "Group Input.001",
        "NodeGroupInput",
        [-439.519_17, -329.139_6],
        vec![],
        group_input(),
        PropBag::default(),
    );
    let gi2 = add(
        "Group Input.002",
        "NodeGroupInput",
        [-259.038_36, -188.658_57],
        vec![],
        group_input(),
        PropBag::default(),
    );
    let or = add(
        "Boolean Math",
        "FunctionNodeBooleanMath",
        [-80.0, -149.139_6],
        bool_math(),
        vec![boolean("Boolean", "Boolean", false)],
        props(&[("operation", "OR")]),
    );
    let or2 = add(
        "Boolean Math.002",
        "FunctionNodeBooleanMath",
        [-260.0, -380.0],
        bool_math(),
        vec![boolean("Boolean", "Boolean", false)],
        props(&[("operation", "OR")]),
    );
    let gi3 = add(
        "Group Input.003",
        "NodeGroupInput",
        [-440.0, -460.0],
        vec![],
        group_input(),
        PropBag::default(),
    );
    let link = |from_node, from: &str, to_node, to: &str| Link {
        from_node,
        from_socket: from.into(),
        to_node,
        to_socket: to.into(),
    };
    t.links = vec![
        link(edge_angle, "Unsigned Angle", compare, "A"),
        link(set2, "Geometry", out, "Output_2"),
        link(gi1, INPUT_ANGLE, compare, "B"),
        link(compare, "Result", and, "Boolean"),
        link(gi, INPUT_MESH, set1, "Geometry"),
        link(set1, "Geometry", set2, "Geometry"),
        link(and, "Boolean", set1, "Shade Smooth"),
        link(gi2, INPUT_IGNORE_SHARPNESS, or, "Boolean_001"),
        link(edge_smooth, "Smooth", or, "Boolean"),
        link(or, "Boolean", set1, "Selection"),
        link(gi3, INPUT_IGNORE_SHARPNESS, or2, "Boolean_001"),
        link(shade_smooth, "Smooth", or2, "Boolean"),
        link(or2, "Boolean", and, "Boolean_001"),
    ];
    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::IdKey;

    #[test]
    fn group_matches_the_blender_dump() {
        let t = build(IdRef::from_key(IdKey::next()));
        assert_eq!(t.nodes.len(), 14);
        assert_eq!(t.links.len(), 13);
        assert!(t.header.is_linked_packed && t.header.library.is_some());
        let ids: Vec<&str> = t.interface.iter().map(|s| s.identifier.as_str()).collect();
        assert_eq!(ids, ["Output_2", "Input_0", "Input_1", "Socket_1"]);
        let names: Vec<&str> = t.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names[0..3],
            ["Set Shade Smooth", "Set Shade Smooth.001", "Group Output"]
        );
        // Every link joins existing sockets.
        for l in &t.links {
            let from = t.nodes.iter().find(|n| n.uid == l.from_node).unwrap();
            let to = t.nodes.iter().find(|n| n.uid == l.to_node).unwrap();
            assert!(from.outputs.iter().any(|s| s.identifier == l.from_socket));
            assert!(to.inputs.iter().any(|s| s.identifier == l.to_socket));
        }
    }

    #[test]
    fn recognised_by_library_file_name_and_group_name() {
        assert!(is_smooth_by_angle(NAME, Some(ESSENTIALS_FILEPATH)));
        assert!(is_smooth_by_angle(
            NAME,
            Some(
                "/Applications/Blender.app/Contents/Resources/5.2/datafiles/assets/nodes/geometry_nodes_essentials.blend"
            )
        ));
        assert!(!is_smooth_by_angle(NAME, None), "a local group");
        assert!(!is_smooth_by_angle(
            "Smooth by Angle.001",
            Some(ESSENTIALS_FILEPATH)
        ));
        assert!(!is_smooth_by_angle(NAME, Some("/tmp/other.blend")));
    }
}
