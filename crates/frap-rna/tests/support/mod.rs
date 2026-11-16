// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand-written RNA definitions for frap-rna's own tests. frap-rna cannot dev-depend on the
//! generated `frap-spec-rna` (it depends on frap-rna: dev-dependency cycle, rule 2.2.11), so
//! the properties used here copy the Blender 5.2.4 metadata they stand for (types, ranges,
//! flags, enum items) from `docs/research/blender-rna-datamodel.md` Appendix A and probes.
#![allow(dead_code)]

use frap_rna::*;

pub const FLT_MAX: f64 = 3.402_823_466_385_288_6e38;

pub const fn float(
    identifier: &'static str,
    array: ArrayShape,
    hard: (f64, f64),
    subtype: Subtype,
    flags: PropFlags,
) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Float {
            default: Defaults::Scalar(0.0),
            hard,
            soft: hard,
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype,
        unit: Unit::None,
        array,
        flags,
        deprecated: None,
    }
}

pub const fn int(
    identifier: &'static str,
    array: ArrayShape,
    hard: (i64, i64),
    flags: PropFlags,
) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Int {
            default: Defaults::Scalar(0),
            hard,
            soft: hard,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array,
        flags,
        deprecated: None,
    }
}

pub const fn boolean(identifier: &'static str, array: ArrayShape, flags: PropFlags) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Bool {
            default: Defaults::Scalar(false),
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array,
        flags,
        deprecated: None,
    }
}

pub const fn string(identifier: &'static str, max_bytes: u32, flags: PropFlags) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::String {
            default: "",
            max_bytes,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags,
        deprecated: None,
    }
}

pub const fn item(identifier: &'static str, value: i32) -> EnumItem {
    EnumItem {
        identifier,
        name: identifier,
        description: "",
        value,
    }
}

pub const fn enumeration(
    identifier: &'static str,
    items: &'static [EnumItem],
    flags: PropFlags,
) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Enum {
            items,
            default: EnumDefault::Item(""),
            dynamic: None,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags,
        deprecated: None,
    }
}

pub const fn pointer(
    identifier: &'static str,
    fixed_type: &'static str,
    is_id: bool,
    flags: PropFlags,
) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Pointer { fixed_type, is_id },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags,
        deprecated: None,
    }
}

pub const fn collection(
    identifier: &'static str,
    item_type: &'static str,
    flags: PropFlags,
) -> PropDef {
    PropDef {
        identifier,
        name: identifier,
        description: "",
        kind: PropKind::Collection {
            item_type,
            api: None,
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags,
        deprecated: None,
    }
}

pub const NONE: PropFlags = PropFlags::empty();
pub const RO: PropFlags = PropFlags::READONLY;
pub const NN: PropFlags = PropFlags::NEVER_NULL;

pub const fn st(
    identifier: &'static str,
    base: Option<&'static StructDef>,
    props: &'static [PropDef],
    name_property: Option<&'static str>,
    is_id: bool,
) -> StructDef {
    StructDef {
        identifier,
        base,
        name: identifier,
        description: "",
        props,
        funcs: &[],
        name_property,
        is_id,
    }
}

// --- ID, Object, Scene, Mesh, Material, NodeTree, DecimateModifier (subset) -----------------

pub static ID_PROPS: [PropDef; 1] = [string("name", 256, NN)];
pub static ID: StructDef = st("ID", None, &ID_PROPS, Some("name"), true);

pub static ROTATION_MODES: [EnumItem; 8] = [
    item("QUATERNION", 0),
    item("XYZ", 1),
    item("XZY", 2),
    item("YXZ", 3),
    item("YZX", 4),
    item("ZXY", 5),
    item("ZYX", 6),
    item("AXIS_ANGLE", -1),
];
pub static OBJECT_TYPES: [EnumItem; 3] = [item("MESH", 0), item("EMPTY", 25), item("CAMERA", 11)];

pub static OBJECT_PROPS: [PropDef; 12] = [
    float(
        "location",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Translation,
        PropFlags::ANIMATABLE,
    ),
    float(
        "empty_display_size",
        ArrayShape::Scalar,
        (0.0001, 1000.0),
        Subtype::Distance,
        NONE,
    ),
    enumeration("rotation_mode", &ROTATION_MODES, NONE),
    enumeration("type", &OBJECT_TYPES, RO),
    boolean("hide_render", ArrayShape::Scalar, NONE),
    boolean("lock_location", ArrayShape::Fixed(3), NONE),
    float(
        "matrix_world",
        ArrayShape::Matrix(4, 4),
        (-FLT_MAX, FLT_MAX),
        Subtype::Matrix,
        NONE,
    ),
    pointer("parent", "Object", true, NONE),
    pointer("data", "ID", true, NONE),
    int(
        "parent_vertices",
        ArrayShape::Fixed(3),
        (0, i32::MAX as i64),
        NONE,
    ),
    string("parent_bone", 64, NN),
    float(
        "rotation_euler",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Euler,
        NONE,
    ),
];
pub static OBJECT: StructDef = st("Object", Some(&ID), &OBJECT_PROPS, Some("name"), true);

/// `Scene.frame_current`: INT, hard range [MINAFRAME, MAXFRAME].
pub static SCENE_PROPS: [PropDef; 1] = [int(
    "frame_current",
    ArrayShape::Scalar,
    (-1_048_574, 1_048_574),
    NONE,
)];
pub static SCENE: StructDef = st("Scene", Some(&ID), &SCENE_PROPS, Some("name"), true);

pub static MESH: StructDef = st("Mesh", Some(&ID), &[], Some("name"), true);
pub static MATERIAL_PROPS: [PropDef; 1] = [float(
    "diffuse_color",
    ArrayShape::Fixed(4),
    (0.0, FLT_MAX),
    Subtype::Color,
    NONE,
)];
pub static MATERIAL: StructDef = st("Material", Some(&ID), &MATERIAL_PROPS, Some("name"), true);
pub static NODE_TREE_PROPS: [PropDef; 1] = [collection("nodes", "Node", NONE)];
pub static NODE_TREE: StructDef = st("NodeTree", Some(&ID), &NODE_TREE_PROPS, Some("name"), true);
pub static NODE_PROPS: [PropDef; 2] = [
    string("name", 64, NN),
    collection("inputs", "NodeSocket", NONE),
];
pub static NODE: StructDef = st("Node", None, &NODE_PROPS, Some("name"), false);
pub static SOCKET_PROPS: [PropDef; 2] = [
    string("name", 64, NN),
    float(
        "default_value",
        ArrayShape::Fixed(4),
        (0.0, FLT_MAX),
        Subtype::Color,
        NONE,
    ),
];
pub static SOCKET: StructDef = st("NodeSocketColor", None, &SOCKET_PROPS, Some("name"), false);

pub static DELIMIT_ITEMS: [EnumItem; 5] = [
    item("NORMAL", 1),
    item("MATERIAL", 2),
    item("SEAM", 4),
    item("SHARP", 8),
    item("UV", 16),
];
pub static DECIMATE_PROPS: [PropDef; 2] = [
    string("name", 64, NN),
    enumeration("delimit", &DELIMIT_ITEMS, PropFlags::ENUM_FLAG),
];
pub static DECIMATE: StructDef = st(
    "DecimateModifier",
    None,
    &DECIMATE_PROPS,
    Some("name"),
    false,
);

// --- Operator properties (MESH_OT_primitive_cube_add, MESH_OT_primitive_cylinder_add) -------

pub static ALIGN_ITEMS: [EnumItem; 3] = [item("WORLD", 0), item("VIEW", 1), item("CURSOR", 2)];
pub static CUBE_ADD: [PropDef; 4] = [
    float(
        "size",
        ArrayShape::Scalar,
        (0.0, FLT_MAX),
        Subtype::Distance,
        NONE,
    ),
    boolean("calc_uvs", ArrayShape::Scalar, NONE),
    enumeration("align", &ALIGN_ITEMS, NONE),
    float(
        "location",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Translation,
        NONE,
    ),
];
pub static CYLINDER_VERTICES: PropDef = int("vertices", ArrayShape::Scalar, (3, 10_000_000), NONE);
pub static SAVE_FILEPATH: PropDef = string("filepath", 1024, NONE);

pub fn prop(s: &'static StructDef, id: &str) -> &'static PropDef {
    s.find_prop(id)
        .unwrap_or_else(|| panic!("{}.{id} missing from the test definitions", s.identifier))
}

pub fn op_prop(id: &str) -> &'static PropDef {
    CUBE_ADD
        .iter()
        .find(|p| p.identifier == id)
        .expect("cube_add prop")
}
