// SPDX-License-Identifier: GPL-3.0-or-later
//
// HAND-WRITTEN M0 stand-in for `crates/frap-spec-ops-{mesh,io}/src/gen/*.rs` (ARCHITECTURE
// §4.2, §10.4). Values copied from spec/m0/ops/*.json (Blender 5.2.4 dumps) and
// spec/m0/overlay.json (dynamic enum items); `crates/frap-rna/tests/spec_m0_drift.rs` fails
// if they disagree. Included by `frap_rna::spec_m0`. Deleted when M1 W0 generates the specs.

use smol_str::SmolStr;

use crate::def::*;
use crate::op::*;
use crate::value::Value;

const BIG: f64 = 999999995904.0;

/// `MESH_OT_primitive_cube_add`
pub static OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD: OperatorSpec = OperatorSpec {
    idname: "mesh.primitive_cube_add",
    bl_idname: "MESH_OT_primitive_cube_add",
    label: "Add Cube",
    description: "Construct a cube mesh that consists of six square faces",
    options: OpOptions::REGISTER.union(OpOptions::UNDO),
    props: &PRIMITIVE_CUBE_ADD_PROPS,
    macro_steps: &[],
};

static ALIGN_ITEMS: [EnumItem; 3] = [
    EnumItem { identifier: "WORLD", name: "World", description: "Align the new object to the world" },
    EnumItem { identifier: "VIEW", name: "View", description: "Align the new object to the view" },
    EnumItem { identifier: "CURSOR", name: "3D Cursor", description: "Use the 3D cursor orientation for the new object" },
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

fn p_f32(a: &OpArgs, n: &'static str) -> Prop<f32> {
    Prop { value: a.get(n).as_f32().expect("invariant: coerced float"), is_set: a.is_set(n) }
}
fn p_bool(a: &OpArgs, n: &'static str) -> Prop<bool> {
    Prop { value: a.get(n).as_bool().expect("invariant: coerced bool"), is_set: a.is_set(n) }
}
fn p_vec3(a: &OpArgs, n: &'static str) -> Prop<[f32; 3]> {
    Prop { value: a.get(n).as_vec3().expect("invariant: coerced float[3]"), is_set: a.is_set(n) }
}
fn p_str(a: &OpArgs, n: &'static str) -> Prop<SmolStr> {
    let v = match a.get(n) {
        Value::Str(s) | Value::Enum(s) => s.clone(),
        _ => panic!("invariant: coerced string"),
    };
    Prop { value: v, is_set: a.is_set(n) }
}

impl OpProps for PrimitiveCubeAdd {
    const IDNAME: &'static str = "mesh.primitive_cube_add";
    const BL_IDNAME: &'static str = "MESH_OT_primitive_cube_add";
    fn spec() -> &'static OperatorSpec {
        &OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD
    }
    fn from_args(a: &OpArgs) -> Self {
        let align = p_str(a, "align");
        PrimitiveCubeAdd {
            size: p_f32(a, "size"),
            calc_uvs: p_bool(a, "calc_uvs"),
            enter_editmode: p_bool(a, "enter_editmode"),
            align: Prop {
                value: match align.value.as_str() {
                    "VIEW" => primitive_cube_add::Align::View,
                    "CURSOR" => primitive_cube_add::Align::Cursor,
                    _ => primitive_cube_add::Align::World,
                },
                is_set: align.is_set,
            },
            location: p_vec3(a, "location"),
            rotation: p_vec3(a, "rotation"),
            scale: p_vec3(a, "scale"),
        }
    }
}

/// `EXPORT_SCENE_OT_gltf` (M0 subset of the 110 props: `filepath`, `export_format`).
pub static OPSPEC_EXPORT_SCENE_OT_GLTF: OperatorSpec = OperatorSpec {
    idname: "export_scene.gltf",
    bl_idname: "EXPORT_SCENE_OT_gltf",
    label: "Export glTF 2.0",
    description: "Export scene as glTF 2.0 file",
    options: OpOptions::PRESET,
    props: &EXPORT_SCENE_GLTF_PROPS,
    macro_steps: &[],
};

static EXPORT_FORMAT_ITEMS: [EnumItem; 2] = [
    EnumItem {
        identifier: "GLB",
        name: "glTF Binary (.glb)",
        description: "Exports a single file, with all data packed in binary form. Most efficient and portable, but more difficult to edit later",
    },
    EnumItem {
        identifier: "GLTF_SEPARATE",
        name: "glTF Separate (.gltf + .bin + textures)",
        description: "Exports multiple files, with separate JSON, binary and texture data. Easiest to edit later",
    },
];

/// Props of `export_scene.gltf` implemented in M0.
pub static EXPORT_SCENE_GLTF_PROPS: [PropDef; 2] = [
    PropDef {
        identifier: "filepath",
        name: "File Path",
        description: "Filepath used for exporting the file",
        kind: PropKind::String { default: "", max_bytes: 1025 },
        subtype: Subtype::FilePath,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::NEVER_NULL,
    },
    PropDef {
        identifier: "export_format",
        name: "Format",
        description: "Output format. Binary is most efficient, but JSON may be easier to edit later",
        kind: PropKind::Enum {
            items: &EXPORT_FORMAT_ITEMS,
            default: EnumDefault::Item("GLB"),
            dynamic: Some("EXPORT_SCENE_OT_gltf.export_format"),
        },
        subtype: Subtype::None,
        unit: Unit::None,
        array: ArrayShape::Scalar,
        flags: PropFlags::ANIMATABLE,
    },
];

/// Typed props of `export_scene.gltf` (M0 subset).
#[derive(Debug, Clone)]
pub struct ExportSceneGltf {
    /// Output path.
    pub filepath: Prop<SmolStr>,
    /// "GLB" | "GLTF_SEPARATE"
    pub export_format: Prop<SmolStr>,
}

impl OpProps for ExportSceneGltf {
    const IDNAME: &'static str = "export_scene.gltf";
    const BL_IDNAME: &'static str = "EXPORT_SCENE_OT_gltf";
    fn spec() -> &'static OperatorSpec {
        &OPSPEC_EXPORT_SCENE_OT_GLTF
    }
    fn from_args(a: &OpArgs) -> Self {
        ExportSceneGltf { filepath: p_str(a, "filepath"), export_format: p_str(a, "export_format") }
    }
}

/// Every M0 operator spec.
pub static ALL_OPS: [&OperatorSpec; 2] = [&OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD, &OPSPEC_EXPORT_SCENE_OT_GLTF];
