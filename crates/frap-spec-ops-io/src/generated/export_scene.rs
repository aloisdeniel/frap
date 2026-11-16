// SPDX-License-Identifier: GPL-3.0-or-later
//! HAND-WRITTEN M0 stand-in for the generated `export_scene` namespace (ARCHITECTURE §4.2,
//! §17.2), moved from `spec/m0/ops.rs` by M1 W0 (WP 1.0): the M0 subset of the 110
//! `export_scene.gltf` props. Values copied from `spec/m0/ops/export_scene.gltf.json` and
//! `spec/m0/overlay.json`; `tests/spec_m0_drift.rs` fails if they disagree.
//! `cargo xtask codegen` (WP 1.1) replaces this directory (and adds `import_scene`).

use frap_rna::*;
use smol_str::SmolStr;


/// `EXPORT_SCENE_OT_gltf` (M0 subset of the 110 props: `filepath`, `export_format`).
pub static OPSPEC_EXPORT_SCENE_OT_GLTF: OperatorSpec = OperatorSpec {
    idname: "export_scene.gltf",
    bl_idname: "EXPORT_SCENE_OT_gltf",
    label: "Export glTF 2.0",
    description: "Export scene as glTF 2.0 file",
    options: OpOptions::PRESET,
    props: &EXPORT_SCENE_GLTF_PROPS,
    macro_steps: &[],
    poll_matrix: None, // not in the M0 dump; WP 1.1 generates the real matrix
};

static EXPORT_FORMAT_ITEMS: [EnumItem; 2] = [
    EnumItem {
        identifier: "GLB",
        name: "glTF Binary (.glb)",
        description: "Exports a single file, with all data packed in binary form. Most efficient and portable, but more difficult to edit later",
        value: 0,
    },
    EnumItem {
        identifier: "GLTF_SEPARATE",
        name: "glTF Separate (.gltf + .bin + textures)",
        description: "Exports multiple files, with separate JSON, binary and texture data. Easiest to edit later",
        value: 1,
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
        deprecated: None,
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
        deprecated: None,
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
        ExportSceneGltf { filepath: a.prop(0, "filepath"), export_format: a.prop(1, "export_format") }
    }
}

/// Every operator spec of this stand-in (drift test).
pub static ALL_OPS: [&OperatorSpec; 1] = [&OPSPEC_EXPORT_SCENE_OT_GLTF];
