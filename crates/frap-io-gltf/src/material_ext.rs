// SPDX-License-Identifier: GPL-3.0-or-later
//! Extension points for the `KHR_materials_*` extensions (CONTRACT, M3 W0, decision log
//! M34W0): one file per extension in `export/materials/ext/` registers a
//! [`MaterialExtExporter`] and one in `import/materials_ext/` a [`MaterialExtImporter`]
//! (WP 3.6), so the core material gatherers (`export/materials/mod.rs`,
//! `import/materials.rs`, WP 3.5) never name an extension.
//!
//! - Export: after the core members (`emissiveTexture`, `normalTexture`, `occlusionTexture`,
//!   `pbrMetallicRoughness`) every exporter runs in `order` and adds its object to
//!   `material.extensions` (and to `extensionsUsed` through [`ExtExportCx::use_extension`]),
//!   reading the [`LoweredMaterial`]'s `ext` fields.
//! - Import: after the core members are read into the [`LoweredMaterial`], every importer runs
//!   in `order` and fills its `ext` field from the glTF material (textures through
//!   [`ExtImportHost::texture`]); `frap_material::raise` then builds the node graph.

use frap_gltf::json::{Extensions, Material as GMaterial, TextureInfo};
use frap_material::{ChannelSel, LoweredMaterial, TexRef};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId, ReportType};

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{TextureSlot, gather_texture_info};
use crate::import::ImportError;

/// What an extension exporter works with.
pub struct ExtExportCx<'g, 'a> {
    /// The export state (document, buffer, settings, reports).
    pub g: &'g mut Gather<'a>,
    /// The material being written.
    pub material: IdRef<MaterialId>,
    /// The mesh the material is written for (texture coordinates resolve per mesh).
    pub mesh: &'g Mesh,
}

impl ExtExportCx<'_, '_> {
    /// `gather_texture_info` for an extension texture; `slot` names the feature
    /// (`"clearcoatTexture"`), so the same image used by two features is two glTF textures
    /// exactly when the add-on makes two.
    pub fn texture_info(
        &mut self,
        t: &TexRef,
        slot: &'static str,
    ) -> Result<Option<TextureInfo>, ExportError> {
        gather_texture_info(
            self.g,
            t,
            (self.material, TextureSlot::Ext(slot)),
            self.mesh,
        )
    }

    /// Adds `name` to `extensionsUsed` (and `extensionsRequired` when `required`).
    pub fn use_extension(&mut self, name: &str, required: bool) {
        self.g.use_extension(name, required);
    }

    /// Adds a WARNING (once per message).
    pub fn warn(&mut self, message: impl Into<String>) {
        self.g.warn(message);
    }
}

/// One `KHR_materials_*` exporter (`export/materials/ext/<name>.rs`).
#[derive(Debug)]
pub struct MaterialExtExporter {
    /// The extension name (`"KHR_materials_clearcoat"`).
    pub name: &'static str,
    /// Run order (ascending; ties by name).
    pub order: u16,
    /// Writes the extension into `material.extensions` when the lowered material uses it.
    pub run:
        fn(&mut ExtExportCx<'_, '_>, &LoweredMaterial, &mut GMaterial) -> Result<(), ExportError>,
}

/// Every material extension exporter.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static MATERIAL_EXT_EXPORTERS: [MaterialExtExporter];

/// What an extension importer may ask the importer for.
pub trait ExtImportHost {
    /// The `TexRef` of a glTF `textureInfo` (image imported on first use, sampler and
    /// `KHR_texture_transform` read), as the core members do; `None` when the texture has no
    /// usable image.
    fn texture(
        &mut self,
        index: u32,
        tex_coord: Option<u32>,
        extensions: &Extensions,
        channel: ChannelSel,
        is_data: bool,
    ) -> Result<Option<TexRef>, ImportError>;
    /// Adds a report (once per message for WARNINGs).
    fn report(&mut self, level: ReportType, message: String);
}

/// One `KHR_materials_*` importer (`import/materials_ext/<name>.rs`).
#[derive(Debug)]
pub struct MaterialExtImporter {
    /// The extension name (`"KHR_materials_clearcoat"`).
    pub name: &'static str,
    /// Run order (ascending; ties by name).
    pub order: u16,
    /// Reads the extension from the glTF material into `lm.ext`.
    pub run:
        fn(&mut dyn ExtImportHost, &GMaterial, &mut LoweredMaterial) -> Result<(), ImportError>,
}

/// Every material extension importer.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static MATERIAL_EXT_IMPORTERS: [MaterialExtImporter];

/// The exporters in run order.
pub fn exporters() -> Vec<&'static MaterialExtExporter> {
    let mut v: Vec<_> = MATERIAL_EXT_EXPORTERS.iter().collect();
    v.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(b.name)));
    v
}

/// The importers in run order.
pub fn importers() -> Vec<&'static MaterialExtImporter> {
    let mut v: Vec<_> = MATERIAL_EXT_IMPORTERS.iter().collect();
    v.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(b.name)));
    v
}
