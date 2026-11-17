// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene → glTF with io_scene_gltf2 5.2.41 semantics (ARCHITECTURE §17.2), static scenes (M1):
//! meshes with their material primitives, the object hierarchy with Blender's TRS rules,
//! Principled base materials and base-colour textures, `GLB`, `GLTF_SEPARATE` and
//! `GLTF_EMBEDDED`.
//!
//! The module layout mirrors the add-on: [`settings`] (operator props), [`vtree`]
//! (`blender/exp/tree.py`), [`nodes`] (`nodes.py`), [`mesh`] (`primitive_extract.py`,
//! `primitives.py`, `obj_data.py`, with the extension-point slices of [`mesh::ext`]),
//! [`materials`] (`material/*`), [`images`] (`sampler.py`, `material/{texture,image}.py`) and
//! [`mod@write`] (`io/exp/export.py`). [`gather`] holds the shared state; the gatherers call each
//! other in the add-on's traversal order so array order and BIN layout follow Blender's.
//! Byte identity with Blender is a non-goal (goldens compare canonically, WP 1.23).

pub mod gather;
pub mod images;
pub mod materials;
pub mod mesh;
pub mod nodes;
pub mod settings;
pub mod vtree;
pub mod write;

use frap_gltf::Document;
use frap_rna::Report;
use frap_scene::{Context, Main};

pub use settings::{
    ExportFormat, ExportImageFormat, ExportMaterials, ExportSettings, SUPPORTED_PROPS,
};

/// Exporter errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExportError {
    /// Feature not available in this milestone.
    #[error("{0}")]
    NotImplemented(String),
    /// Inconsistent data produced by an extension point (a writer or post-processor bug).
    #[error("frap glTF export: {0}")]
    Invalid(String),
}

/// What the exporter produced.
#[derive(Clone, Debug)]
pub struct ExportOutput {
    /// The main file: the `.glb`, or the `.gltf` JSON text.
    pub bytes: Vec<u8>,
    /// Further files of `GLTF_SEPARATE` (`<stem>.bin` first, then textures), as
    /// `(path relative to the main file, bytes)`; written by the caller.
    pub files: Vec<(String, Vec<u8>)>,
    /// `extensionsUsed`.
    pub extensions_used: Vec<String>,
    /// Number of glTF nodes (reported in `Response.result`).
    pub nodes: usize,
    /// Number of glTF meshes.
    pub meshes: usize,
    /// WARNINGs for features exported partially or not at all.
    pub reports: Vec<Report>,
    /// The written document (buffers referenced, not embedded).
    pub document: Document,
}

/// Exports the context scene.
pub fn export(
    main: &Main,
    ctx: &Context,
    settings: &ExportSettings,
) -> Result<ExportOutput, ExportError> {
    let tree = vtree::build(main, ctx, settings);
    let mut g = gather::Gather::new(main, ctx, settings);
    let mut roots = Vec::with_capacity(tree.roots.len());
    for &r in &tree.roots {
        roots.push(nodes::gather_node(&mut g, &tree, r)?);
    }
    write::finish(g, roots)
}
