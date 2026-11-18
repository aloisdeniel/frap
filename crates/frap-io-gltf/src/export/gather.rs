// SPDX-License-Identifier: GPL-3.0-or-later
//! Gather state shared by the node, mesh, material and image gatherers.
//!
//! The add-on builds Python objects first and assigns array indices while its exporter
//! traverses them (blender/exp/exporter.py, `__traverse`: members in **alphabetical** order,
//! children before their parent, a child-of-root object appended when first reached, equal
//! objects shared). The gatherers here call each other in that same order and append to the
//! document and the buffer as they go, so array order and BIN layout follow Blender's: node
//! `children` before `mesh`, a primitive's `attributes` before `indices` before `material`,
//! a texture's `sampler` before `source`, an image's bytes where its material is first used.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use frap_gltf::json::Index;
use frap_gltf::{BufferBuilder, Document};
use frap_rna::{IdRef, MaterialId, MeshId, Report, ReportType};
use frap_scene::{Context, Main};

use crate::export::ExportSettings;
use crate::export::images::ImageCache;
use crate::export::materials::MaterialCache;
use crate::export::mesh::gather::WrittenPrimitive;

/// A glTF mesh's identity: the mesh datablock and the object's material per slot.
pub type MeshKey = (IdRef<MeshId>, Vec<Option<IdRef<MaterialId>>>);

/// Mutable state of one export.
pub struct Gather<'a> {
    /// `bpy.data`
    pub main: &'a Main,
    /// The context (scene, view layer).
    pub ctx: &'a Context<'a>,
    /// Settings.
    pub settings: &'a ExportSettings,
    /// The document being built (views and accessors live in `buf` until the end).
    pub doc: Document,
    /// The single buffer (GLB BIN chunk, `.bin` file or data URI).
    pub buf: BufferBuilder,
    /// Reports (WARNING for features exported partially or not at all).
    pub reports: Vec<Report>,
    /// Extra files of `GLTF_SEPARATE` (texture images), by path relative to the `.gltf`.
    pub files: Vec<(String, Vec<u8>)>,
    /// glTF mesh per (mesh, material slots) (the add-on's `gather_mesh` cache key).
    pub(crate) meshes: HashMap<MeshKey, Option<Index>>,
    /// Written primitives per mesh (the add-on's `__gather_cache_primitives`): objects sharing a
    /// mesh with different materials share the accessors.
    pub(crate) primitives: HashMap<IdRef<MeshId>, Vec<WrittenPrimitive>>,
    /// Index accessors by content (the add-on's `@cached gather_accessor`).
    pub(crate) index_accessors: HashMap<Vec<u32>, Index>,
    /// Materials.
    pub(crate) materials: MaterialCache,
    /// Textures, samplers and images.
    pub(crate) images: ImageCache,
    /// Per-feature state of later milestones' files (camera/light caches, extras, …), one
    /// value per type ([`Gather::state`]; M3+M4 W0), so they never add fields here.
    states: HashMap<TypeId, Box<dyn Any>>,
}

impl<'a> Gather<'a> {
    /// Fresh state.
    pub fn new(main: &'a Main, ctx: &'a Context<'a>, settings: &'a ExportSettings) -> Self {
        Gather {
            main,
            ctx,
            settings,
            doc: Document::default(),
            buf: BufferBuilder::new(),
            reports: Vec::new(),
            files: Vec::new(),
            meshes: HashMap::new(),
            primitives: HashMap::new(),
            index_accessors: HashMap::new(),
            materials: MaterialCache::default(),
            images: ImageCache::default(),
            states: HashMap::new(),
        }
    }

    /// The export-wide state of type `T` of a feature file (created on first use with
    /// `T::default()`).
    pub fn state<T: Any + Default>(&mut self) -> &mut T {
        self.states
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut::<T>()
            .expect("invariant: one state per TypeId")
    }

    /// Adds a WARNING report (once per distinct message).
    pub fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        if !self.reports.iter().any(|r| r.message == message) {
            self.reports.push(Report::new(ReportType::Warning, message));
        }
    }

    /// Records an extension in `extensionsUsed` (and `extensionsRequired`), in first-use order.
    pub fn use_extension(&mut self, name: &str, required: bool) {
        if !self.doc.extensions_used.iter().any(|e| e == name) {
            self.doc.extensions_used.push(name.to_owned());
        }
        if required && !self.doc.extensions_required.iter().any(|e| e == name) {
            self.doc.extensions_required.push(name.to_owned());
        }
    }
}
