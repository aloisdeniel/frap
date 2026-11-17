// SPDX-License-Identifier: GPL-3.0-or-later
//! `scene.json` ↔ `Main` mapping: RNA-identifier keys, `{"$id", "name"}` references, linked IDs, orphans dropped (WP 1.23).
//!
//! Layout (ARCHITECTURE §15; keys are RNA identifiers, every implemented value is written):
//!
//! ```jsonc
//! {"window": {"scene": "Scene", "view_layer": "ViewLayer",
//!             "view3d": {"view_matrix": [[…]], "shading": {…}}},
//!  "libraries": {"Essentials": {"filepath": "…", "parent": null, "is_archive": false, …}},
//!  "scenes": {"Scene": {"camera": {"$id": "OB", "name": "Camera"}, "render": {…}, …,
//!             "collection": {"name": "Scene Collection", "objects": […], "children": […], …},
//!             "view_layers": [{"name": "ViewLayer",
//!                              "bases": [{"object": {"$id": "OB", "name": "Cube"}, "select": true, "hide": false}],
//!                              "active": {"$id": "OB", "name": "Cube"},
//!                              "layer_collection": {"collection": null, "exclude": false, …, "children": […]},
//!                              "active_layer_collection": []}]}},
//!  "objects": {"Cube": {"type": "MESH", "data": {"$id": "ME", "name": "Cube"}, "location": [0.0, 0.0, 0.0], …}},
//!  "meshes": {"Cube": {"face_offsets": {"$buf": "5e1c…", "dtype": "i32", "len": 7},
//!                      "attributes": [{"name": "position", "domain": "POINT", "data_type": "FLOAT_VECTOR",
//!                                      "data": [[1.0, 1.0, 1.0], …]}, …], …}},
//!  "node_groups": {"$linked": {"Essentials": {"Smooth by Angle": {…}}}},
//!  …,
//!  "preferences": {"addons": {"io_scene_gltf2": {…}}}}
//! ```
//!
//! - **Collections** are objects keyed by name in Blender's `ListBase` order (§5.4). Linked
//!   IDs are written under `"$linked": {"<library name>": {…}}` inside their collection, so a
//!   local and a linked ID of the same name both round-trip (`$`-prefixed like the other
//!   reserved keys, so a local ID named `linked` cannot collide with it).
//! - **References** are `{"$id": code, "name": name}`, plus `"library": "<library name>"` for
//!   linked IDs; `IdKey`s (and `session_uid`) are regenerated on load. A reference to an ID
//!   that is not written (an orphan reached through a non-user pointer such as `Scene.camera`)
//!   is written as `null`.
//! - **Embedded IDs** are inline: `materials.X.node_tree`, `worlds.X.node_tree`,
//!   `scenes.X.collection`.
//! - **Arrays** of mesh data go through [`crate::buffers::BufferStore::encode`]: inline below
//!   64 elements, else a content-addressed buffer. Matrices are row-major (as RNA reads them).
//!   Floats keep their shortest `f32` spelling; non-finite values are `{"$float": "inf"}`.
//! - **Generated plain structs** (`render`, `tool_settings`, `cursor`, camera and light fields,
//!   …) are written with their serde derives.
//! - **Orphans** (no users, no fake user, `Main::orphans`) are not written (§5.5, Blender
//!   `[OBJ §1.11]`); [`Saved::dropped`] names them.
//! - **Unknown fields** fail the load unless it is lenient (`--lenient`).

mod json;
mod read;
mod write;

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdAny, IdRef, MeshId};
use frap_scene::{Main, Preferences, WindowState};
use serde_json::Value as J;

use crate::container::{self, ProjectError, ReadOptions, to_json_pretty};

/// Key of the linked-ID sub-map inside a collection.
pub const LINKED: &str = "$linked";

/// Errors of the `scene.json` mapping.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum MapError {
    /// A value of `scene.json` has the wrong shape or refers to a missing ID.
    #[error("invalid scene.json at {path}: {message}")]
    Invalid {
        /// Where (`objects["Cube"].location`).
        path: String,
        /// What is wrong.
        message: String,
    },
    /// A field this build does not know (load is not lenient).
    #[error("unknown field `{0}` in scene.json (open with --lenient to ignore it)")]
    Unknown(String),
    /// The container could not be read or written.
    #[error(transparent)]
    Project(#[from] ProjectError),
}

impl MapError {
    pub(crate) fn invalid(path: &str, message: impl Into<String>) -> MapError {
        MapError::Invalid {
            path: path.to_owned(),
            message: message.into(),
        }
    }
    /// Whether the file was written by a newer frap.
    pub fn is_newer_schema(&self) -> bool {
        matches!(self, MapError::Project(p) if p.is_newer_schema())
    }
}

/// What [`save`] produced.
#[derive(Clone, Debug)]
pub struct Saved {
    /// The `.frap` archive.
    pub bytes: Vec<u8>,
    /// The orphans that were not written, as `bpy.data` keys (`meshes["Cube.001"]`), in
    /// `Main::all_ids` order.
    pub dropped: Vec<String>,
}

/// The edit-mode view of a mesh, when it differs from `Main`'s (§6.3); `None` writes the
/// mesh of `Main`.
pub type MeshViewFn<'a> = &'a dyn Fn(IdRef<MeshId>) -> Option<Arc<Mesh>>;

fn no_view(_: IdRef<MeshId>) -> Option<Arc<Mesh>> {
    None
}

/// The `scene.json` tree and its buffers and images (no container).
pub fn to_json(
    main: &Main,
    win: &WindowState,
    prefs: &Preferences,
    mesh_view: Option<MeshViewFn<'_>>,
) -> Result<
    (
        J,
        crate::buffers::BufferStore,
        crate::buffers::ImageStore,
        Vec<String>,
    ),
    MapError,
> {
    let orphans: Vec<IdAny> = main.orphans();
    let skip: BTreeSet<IdAny> = orphans.iter().copied().collect();
    let view = mesh_view.unwrap_or(&no_view);
    let mut w = write::Writer::new(main, &skip, view);
    let tree = w.scene_json(win, prefs)?;
    let dropped = orphans
        .iter()
        .filter_map(|a| {
            main.header(*a)
                .map(|h| format!("{}[{:?}]", a.code.collection(), h.name.as_str()))
        })
        .collect();
    Ok((tree, w.buffers, w.images, dropped))
}

/// Serializes the state into a `.frap` archive (deterministic: saving the same state twice
/// gives the same bytes, §15).
pub fn save(
    main: &Main,
    win: &WindowState,
    prefs: &Preferences,
    mesh_view: Option<MeshViewFn<'_>>,
) -> Result<Saved, MapError> {
    let (tree, buffers, images, dropped) = to_json(main, win, prefs, mesh_view)?;
    let scene =
        to_json_pretty(&tree).map_err(|e| MapError::invalid("scene.json", e.to_string()))?;
    let bytes = container::write(&scene, &buffers, &images)?;
    Ok(Saved { bytes, dropped })
}

/// A loaded `.frap`.
#[derive(Clone, Debug)]
pub struct Opened {
    /// `bpy.data` (`filepath` unset: the caller sets it).
    pub main: Main,
    /// Window state (context scene and view layer, the virtual 3D View).
    pub win: WindowState,
    /// glTF add-on preferences saved with the file.
    pub prefs: Preferences,
    /// WARNING reports (e.g. an object saved in Edit Mode).
    pub warnings: Vec<String>,
    /// The `schema_version` the file was migrated from, if older.
    pub migrated_from: Option<u32>,
}

/// Reads a `.frap` archive. `lenient` ignores unknown entries and fields.
pub fn open(bytes: &[u8], lenient: bool) -> Result<Opened, MapError> {
    let loaded = container::read(bytes, ReadOptions { lenient })?;
    let r = read::read(&loaded.scene, &loaded.buffers, &loaded.images, !lenient)?;
    Ok(Opened {
        main: r.main,
        win: r.win,
        prefs: r.prefs,
        warnings: r.warnings,
        migrated_from: loaded.migration.map(|m| m.from),
    })
}

/// [`open`] on a file.
pub fn open_file(path: &Path, lenient: bool) -> Result<Opened, MapError> {
    let bytes = std::fs::read(path).map_err(|e| MapError::Project(ProjectError::Io(e)))?;
    open(&bytes, lenient)
}

/// Reads a `scene.json` tree directly (tests, migrations).
pub fn from_json(
    tree: &J,
    buffers: &crate::buffers::BufferStore,
    images: &crate::buffers::ImageStore,
    lenient: bool,
) -> Result<Opened, MapError> {
    let r = read::read(tree, buffers, images, !lenient)?;
    Ok(Opened {
        main: r.main,
        win: r.win,
        prefs: r.prefs,
        warnings: r.warnings,
        migrated_from: None,
    })
}

#[cfg(test)]
mod tests;
