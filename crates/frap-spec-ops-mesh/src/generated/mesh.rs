// SPDX-License-Identifier: GPL-3.0-or-later
//! HAND-WRITTEN M0 stand-in for the generated `mesh` namespace (ARCHITECTURE §4.2, §10.4),
//! moved from `spec/m0/ops.rs` by M1 W0 (WP 1.0). Values copied from
//! `spec/m0/ops/mesh.primitive_cube_add.json` (Blender 5.2.4); `tests/spec_m0_drift.rs` fails
//! if they disagree. `cargo xtask codegen` (WP 1.1) replaces this directory.

use frap_rna::*;

const BIG: f64 = 999999995904.0;

const POLL_ALL_Y: PollMatrix = PollMatrix { plain: [PollCell::Y; 5], view3d: [PollCell::Y; 5] };

/// `MESH_OT_primitive_cube_add`
pub static OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD: OperatorSpec = OperatorSpec {
    idname: "mesh.primitive_cube_add",
    bl_idname: "MESH_OT_primitive_cube_add",
    label: "Add Cube",
    description: "Construct a cube mesh that consists of six square faces",
    options: OpOptions::REGISTER.union(OpOptions::UNDO),
    props: &PRIMITIVE_CUBE_ADD_PROPS,
    macro_steps: &[],
    poll_matrix: Some(POLL_ALL_Y),
};

static ALIGN_ITEMS: [EnumItem; 3] = [
    EnumItem { identifier: "WORLD", name: "World", description: "Align the new object to the world", value: 0 },
    EnumItem { identifier: "VIEW", name: "View", description: "Align the new object to the view", value: 1 },
    EnumItem { identifier: "CURSOR", name: "3D Cursor", description: "Use the 3D cursor orientation for the new object", value: 2 },
];

/// Props of `mesh.primitive_cube_add`, RNA order.
pub static PRIMITIVE_CUBE_ADD_PROPS: [PropDef; 7] = [
    PropDef {
        identifier: "size",
        name: "Size",
        description: "",
        kind: PropKind::Float {
            default: Defaults::Scalar(2.0),
            hard: (0.0, BIG),
            soft: (0.0010000000474974513, 100.0),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::Distance,
        unit: Unit::Length,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
        deprecated: None,
    },
    PropDef {
        identifier: "calc_uvs",
        name: "Generate UVs",
        description: "Generate a default UV map",
        kind: PropKind::Bool { default: Defaults::Scalar(true) },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
        deprecated: None,
    },
    PropDef {
        identifier: "enter_editmode",
        name: "Enter Edit Mode",
        description: "Enter edit mode when adding this object",
        kind: PropKind::Bool { default: Defaults::Scalar(false) },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::HIDDEN.union(PropFlags::SKIP_SAVE).union(PropFlags::ANIMATABLE),
        deprecated: None,
    },
    PropDef {
        identifier: "align",
        name: "Align",
        description: "The alignment of the new object",
        kind: PropKind::Enum { items: &ALIGN_ITEMS, default: EnumDefault::Item("WORLD"), dynamic: None },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
        deprecated: None,
    },
    PropDef {
        identifier: "location",
        name: "Location",
        description: "Location for the newly added object",
        kind: PropKind::Float {
            default: Defaults::Array(&[0.0, 0.0, 0.0]),
            hard: (-BIG, BIG),
            soft: (-1000.0, 1000.0),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::XyzLength,
        unit: Unit::Length,
        array: ArrayShape::Fixed(3),
        flags: PropFlags::SKIP_SAVE.union(PropFlags::ANIMATABLE),
        deprecated: None,
    },
    PropDef {
        identifier: "rotation",
        name: "Rotation",
        description: "Rotation for the newly added object",
        kind: PropKind::Float {
            default: Defaults::Array(&[0.0, 0.0, 0.0]),
            hard: (-BIG, BIG),
            soft: (-6.2831854820251465, 6.2831854820251465),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::Euler,
        unit: Unit::Rotation,
        array: ArrayShape::Fixed(3),
        flags: PropFlags::SKIP_SAVE.union(PropFlags::ANIMATABLE),
        deprecated: None,
    },
    PropDef {
        identifier: "scale",
        name: "Scale",
        description: "Scale for the newly added object",
        kind: PropKind::Float {
            default: Defaults::Array(&[0.0, 0.0, 0.0]),
            hard: (-BIG, BIG),
            soft: (-1000.0, 1000.0),
            precision: 3,
            storage: FloatStorage::F32,
        },
        subtype: Subtype::XyzLength,
        unit: Unit::Length,
        array: ArrayShape::Fixed(3),
        flags: PropFlags::HIDDEN.union(PropFlags::SKIP_SAVE).union(PropFlags::ANIMATABLE),
        deprecated: None,
    },
];

/// Enums of `mesh.primitive_cube_add`.
pub mod primitive_cube_add {
    /// `align`
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Align {
        /// "WORLD"
        World,
        /// "VIEW"
        View,
        /// "CURSOR"
        Cursor,
    }
    impl Align {
        /// Blender identifier.
        pub const fn identifier(self) -> &'static str {
            match self {
                Align::World => "WORLD",
                Align::View => "VIEW",
                Align::Cursor => "CURSOR",
            }
        }
    }
    impl frap_rna::FromValue for Align {
        fn from_value(v: &frap_rna::Value) -> Self {
            match v.as_str() {
                Some("VIEW") => Align::View,
                Some("CURSOR") => Align::Cursor,
                _ => Align::World,
            }
        }
    }
}

/// Typed props of `mesh.primitive_cube_add`.
#[derive(Debug, Clone)]
pub struct PrimitiveCubeAdd {
    /// default 2.0, hard [0, 1e12], DISTANCE
    pub size: Prop<f32>,
    /// default true
    pub calc_uvs: Prop<bool>,
    /// HIDDEN, SKIP_SAVE
    pub enter_editmode: Prop<bool>,
    /// WORLD | VIEW | CURSOR
    pub align: Prop<primitive_cube_add::Align>,
    /// unset => 3D cursor
    pub location: Prop<[f32; 3]>,
    /// unset => from align
    pub rotation: Prop<[f32; 3]>,
    /// HIDDEN; (0,0,0) => unset
    pub scale: Prop<[f32; 3]>,
}

impl OpProps for PrimitiveCubeAdd {
    const IDNAME: &'static str = "mesh.primitive_cube_add";
    const BL_IDNAME: &'static str = "MESH_OT_primitive_cube_add";
    fn spec() -> &'static OperatorSpec {
        &OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD
    }
    fn from_args(a: &OpArgs) -> Self {
        PrimitiveCubeAdd {
            size: a.prop(0, "size"),
            calc_uvs: a.prop(1, "calc_uvs"),
            enter_editmode: a.prop(2, "enter_editmode"),
            align: a.prop(3, "align"),
            location: a.prop(4, "location"),
            rotation: a.prop(5, "rotation"),
            scale: a.prop(6, "scale"),
        }
    }
}

/// Every operator spec of this stand-in (drift test).
pub static ALL_OPS: [&OperatorSpec; 1] = [&OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD];
