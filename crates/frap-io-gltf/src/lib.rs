// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-io-gltf` (layer L9): scene ↔ glTF with io_scene_gltf2 5.2.41 semantics; the module
//! structure mirrors the add-on (`export/`, `import/`) (ARCHITECTURE §17.2).
//!
//! Ownership (M1): `src/export/**` WP 1.16, `src/import/**` WP 1.17, this file the integrator.
//! CONTRACT (frozen at M1 W0): `export/mesh/ext.rs` (`PRIMITIVE_*` slices).
#![deny(unsafe_code)]

pub mod export;
pub mod glb;
pub mod import;
pub mod material_ext;

pub use export::mesh::ext::{
    AttrColumn, AttrValues, KeyColumn, KeyFn, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS,
    PRIMITIVE_EXTRACTORS, PRIMITIVE_POST_PROCESS, PrimitiveAttributeWriter, PrimitiveDraft,
    PrimitiveExtractor, PrimitivePostProcess, SplitKey,
};
pub use export::{ExportError, ExportFormat, ExportOutput, ExportSettings, export};
pub use material_ext::{
    ExtExportCx, ExtImportHost, MATERIAL_EXT_EXPORTERS, MATERIAL_EXT_IMPORTERS,
    MaterialExtExporter, MaterialExtImporter,
};
