// SPDX-License-Identifier: GPL-3.0-or-later
//! `GoldenState` (ARCHITECTURE §20.3, CONTRACT frozen at M1 W0): identical JSON from both sides.
//! `tools/blender/dump_state.py` writes it from Blender; the frap runner writes it through
//! frap's own RNA with the same property list (`tests/golden/props.toml`). Open-ended parts
//! (attributes, node graphs, actions, canonical glTF) are JSON values so both dumpers can grow
//! without a contract change; their shape is pinned by WP 1.8's comparator self-tests.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

/// Blender version and build hash (fixtures only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlenderStamp {
    /// `bpy.app.version_string`
    pub version: String,
    /// `bpy.app.build_hash`
    pub build_hash: String,
}

/// Outcome of one statement: ok, or exception class + message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineOutcome {
    /// Source line of the statement.
    pub line: u32,
    /// True when it ran without an exception (and was not CANCELLED).
    pub ok: bool,
    /// Python exception class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exception: Option<String>,
    /// Exception message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// One object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GObject {
    /// `Object.type`
    pub r#type: String,
    /// Parent name.
    pub parent: Option<String>,
    /// `matrix_world`, row-major.
    pub matrix_world: [[f32; 4]; 4],
    /// `location`
    pub location: [f32; 3],
    /// `rotation_mode`
    pub rotation_mode: String,
    /// Rotation in its mode (Euler XYZ, quaternion w-first or axis-angle).
    pub rotation: Vec<f32>,
    /// `scale`
    pub scale: [f32; 3],
    /// Modifier names and types, in stack order.
    pub modifiers: Vec<(String, String)>,
    /// Material per slot.
    pub materials: Vec<Option<String>>,
    /// `Object.mode`
    pub mode: String,
}

/// One mesh (original, or `evaluated_get` + `to_mesh`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GMesh {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Edges.
    pub edges: Vec<[u32; 2]>,
    /// Faces as corner vertex indices.
    pub faces: Vec<Vec<u32>>,
    /// Corner normals.
    pub corner_normals: Vec<[f32; 3]>,
    /// `MeshPolygon.normal` per face (`face_normal_calc`, not `polygon_normals`). Fixtures
    /// blessed before the M5+M6 W0 lack it; the comparators skip it unless both sides have it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_normals: Option<Vec<[f32; 3]>>,
    /// UV layers by name (per corner).
    pub uv_layers: BTreeMap<String, Vec<[f32; 2]>>,
    /// Other attributes by name: `{"domain", "data_type", "data"}`.
    pub attributes: BTreeMap<String, Json>,
    /// Selected vertices / edges / faces (edit-mode data, M2).
    pub selection: Json,
    /// Active element `{"type", "index"}` (M2).
    pub active: Json,
    /// Select history (M2).
    pub select_history: Json,
}

/// One material: nodes, links, socket defaults (normalised JSON).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GMaterial {
    /// Nodes by name.
    pub nodes: Json,
    /// Links.
    pub links: Json,
}

/// One action: slots, F-curves, keys and sampled values per frame (M6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GAction {
    /// The dump.
    pub data: Json,
}

/// The context at the end of the case.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GContext {
    /// Active object name.
    pub active: Option<String>,
    /// Selected object names, base order.
    pub selected: Vec<String>,
    /// `context.mode`
    pub mode: String,
    /// `tool_settings.mesh_select_mode` in edit mode.
    pub select_mode: Option<[bool; 3]>,
    /// `scene.cursor.location`
    pub cursor: [f32; 3],
}

/// An output file, canonicalised.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum GFile {
    /// A glTF/GLB, as canonical glTF (`gltf_canonical`).
    Gltf {
        /// Canonical form.
        canonical: Json,
    },
    /// An image: size and linear RGBA pixels (or a fixture path for large images).
    Image {
        /// Width.
        width: u32,
        /// Height.
        height: u32,
        /// Pixels or a reference to the fixture PNG.
        pixels: Json,
    },
}

/// Identical JSON from both sides (§20.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GoldenState {
    /// Version + build hash (fixtures only).
    pub blender: Option<BlenderStamp>,
    /// ok | exception class + message, per statement.
    pub lines: Vec<LineOutcome>,
    /// Objects by name.
    pub objects: BTreeMap<String, GObject>,
    /// Meshes by name.
    pub meshes: BTreeMap<String, GMesh>,
    /// Evaluated meshes by object name.
    pub evaluated: BTreeMap<String, GMesh>,
    /// Materials by name.
    pub materials: BTreeMap<String, GMaterial>,
    /// Actions by name.
    pub actions: BTreeMap<String, GAction>,
    /// Context.
    pub context: GContext,
    /// Output files by case-relative path.
    pub files: BTreeMap<String, GFile>,
}

/// `$BLENDER_BIN`, then `blender` on `PATH`, then `~/Applications/Blender-5.2.app` (macOS).
/// The search order is implemented (and tested) by [`crate::blender::locate_from`].
pub fn locate_blender() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "blender.exe"
    } else {
        "blender"
    };
    crate::blender::locate_from(
        std::env::var_os("BLENDER_BIN"),
        std::env::var_os("PATH"),
        std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")),
        exe,
    )
}
