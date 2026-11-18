// SPDX-License-Identifier: GPL-3.0-or-later
//! Exporter settings (ARCHITECTURE §17.2): the M1 subset of `export_scene.gltf`'s 110 props that
//! the library honours, under their Blender names and with Blender's defaults, plus the frap-only
//! inputs a library call needs (`generator`, `file_stem`, `blend_dir`).
//!
//! [`ExportSettings::from_props`] maps the generated operator props (`ExportSceneGltf`) onto
//! this struct; [`SUPPORTED_PROPS`] lists the props it honours, so the operator (WP 1.23) can
//! warn about every other prop set to a non-default value.
//!
//! Effective coupling rules of the add-on's `execute()` that apply to these props are applied
//! by [`ExportSettings::from_props`] (e.g. `use_active_collection_with_nested` only matters with
//! `use_active_collection`), so the exporter reads plain values.
//!
//! M3+M4 W0 (decision log M34W0): [`ExportSettings::props`] carries every typed prop, so a
//! feature file reads the props it implements (`settings.props.export_cameras.value`) without
//! editing this file, and declares them honoured by registering into
//! [`SUPPORTED_EXPORT_PROPS`]; [`is_supported`] is what the operator's WARNING checks.
//! [`ExportSettings::evaluated_meshes`] carries the meshes of `export_apply=True` (filled by the
//! operator from the session's evaluator, read by `apply.rs`, WP 4.7).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdRef, ObjectId, OpArgs, OpProps, Value};
use frap_spec_ops_io::export_scene::{ExportSceneGltf, gltf};

/// `export_format`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ExportFormat {
    /// `GLB`: one binary file.
    #[default]
    Glb,
    /// `GLTF_SEPARATE`: `.gltf` + `.bin` + texture files.
    GltfSeparate,
    /// `GLTF_EMBEDDED`: one `.gltf` with a base64 buffer (offered by the add-on only with its
    /// `allow_embedded_format` preference).
    GltfEmbedded,
}

impl ExportFormat {
    /// Parses Blender's identifier.
    pub fn from_identifier(s: &str) -> Option<Self> {
        match s {
            "GLB" => Some(ExportFormat::Glb),
            "GLTF_SEPARATE" => Some(ExportFormat::GltfSeparate),
            "GLTF_EMBEDDED" => Some(ExportFormat::GltfEmbedded),
            _ => None,
        }
    }
    /// Blender's identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            ExportFormat::Glb => "GLB",
            ExportFormat::GltfSeparate => "GLTF_SEPARATE",
            ExportFormat::GltfEmbedded => "GLTF_EMBEDDED",
        }
    }
}

/// `export_materials`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ExportMaterials {
    /// `EXPORT`: every material used by exported objects.
    #[default]
    Export,
    /// `PLACEHOLDER`: no materials, but one primitive per used slot.
    Placeholder,
    /// `VIEWPORT`: `diffuse_color`, `metallic`, `roughness` of the viewport display.
    Viewport,
    /// `NONE`: no materials, one primitive per mesh.
    None,
}

/// `export_image_format`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ExportImageFormat {
    /// `AUTO`: JPEG sources stay JPEG, everything else becomes PNG.
    #[default]
    Auto,
    /// `JPEG`: every image without alpha as JPEG.
    Jpeg,
    /// `WEBP` (EXT_texture_webp; M3; frap M1 writes PNG and warns).
    Webp,
    /// `NONE`: no images, textures omitted.
    None,
}

/// Exporter settings. Every field not listed here behaves as Blender's default
/// (`export_apply=False`, `export_cameras=False`, `export_lights=False`, `export_tangents=False`,
/// `export_vertex_color='MATERIAL'`, `export_extras=False`, …).
#[derive(Clone, Debug)]
pub struct ExportSettings {
    /// `export_format`
    pub export_format: ExportFormat,
    /// `asset.generator` (Blender writes `Khronos glTF Blender I/O v5.2.41`; ignored by goldens).
    pub generator: String,
    /// `export_copyright` → `asset.copyright` when not empty.
    pub export_copyright: String,
    /// `export_yup`: Blender Z-up → glTF Y-up.
    pub export_yup: bool,
    /// `export_normals`
    pub export_normals: bool,
    /// `export_texcoords`
    pub export_texcoords: bool,
    /// `export_materials`
    pub export_materials: ExportMaterials,
    /// `export_image_format`
    pub export_image_format: ExportImageFormat,
    /// `export_image_quality` (JPEG quality).
    pub export_image_quality: u8,
    /// `export_texture_dir`: texture folder of `GLTF_SEPARATE`, relative to the `.gltf` file.
    pub export_texture_dir: String,
    /// `use_selection`
    pub use_selection: bool,
    /// `use_visible`
    pub use_visible: bool,
    /// `use_renderable`
    pub use_renderable: bool,
    /// `use_active_collection`
    pub use_active_collection: bool,
    /// `use_active_collection_with_nested` (only with `use_active_collection`).
    pub use_active_collection_with_nested: bool,
    /// `collection`: export only this collection's objects (and its children); `""` = scene.
    pub collection: String,
    /// `export_hierarchy_flatten_objs`
    pub export_hierarchy_flatten_objs: bool,
    /// frap: stem of the output file name (`part` for `part.gltf`); names the `.bin` of
    /// `GLTF_SEPARATE` (`part.bin`), as the add-on's `gltf_binaryfilename`.
    pub file_stem: String,
    /// frap: directory of the saved project (`bpy.data.filepath`'s folder), used to resolve
    /// `//`-relative image paths; relative paths without `//` resolve against the cwd.
    pub blend_dir: Option<PathBuf>,
    /// Every operator prop as given (M3+M4 W0); defaults when built with `Default`.
    pub props: Arc<ExportSceneGltf>,
    /// frap: `export_apply=True`: the viewport-evaluated mesh of every MESH object of the scene
    /// (`evaluated_get(depsgraph).to_mesh()`), filled by the operator (M4 W0, read by
    /// `apply.rs`, WP 4.7); `None` otherwise.
    pub evaluated_meshes: Option<Arc<HashMap<IdRef<ObjectId>, Arc<Mesh>>>>,
}

/// The operator props with nothing set (every prop at its RNA default).
pub fn default_props() -> ExportSceneGltf {
    let mut a = OpArgs::default();
    for def in ExportSceneGltf::spec().props {
        a.values.insert(def.identifier, Value::default_for(def));
    }
    ExportSceneGltf::from_args(&a)
}

impl Default for ExportSettings {
    fn default() -> Self {
        ExportSettings {
            export_format: ExportFormat::Glb,
            generator: concat!("frap v", env!("CARGO_PKG_VERSION")).to_owned(),
            export_copyright: String::new(),
            export_yup: true,
            export_normals: true,
            export_texcoords: true,
            export_materials: ExportMaterials::Export,
            export_image_format: ExportImageFormat::Auto,
            export_image_quality: 75,
            export_texture_dir: String::new(),
            use_selection: false,
            use_visible: false,
            use_renderable: false,
            use_active_collection: false,
            use_active_collection_with_nested: true,
            collection: String::new(),
            export_hierarchy_flatten_objs: false,
            file_stem: "scene".to_owned(),
            blend_dir: None,
            props: Arc::new(default_props()),
            evaluated_meshes: None,
        }
    }
}

/// The `export_scene.gltf` props that [`ExportSettings::from_props`] honours (M1); feature files
/// add theirs through [`SUPPORTED_EXPORT_PROPS`]. Every other prop is accepted and ignored by
/// the library (bookkeeping props such as `filepath`,
/// `check_existing`, `ui_tab`, `filter_glob`, `will_save_settings`, `export_loglevel` have no
/// effect in Blender either).
pub const SUPPORTED_PROPS: &[&str] = &[
    "filepath",
    "check_existing",
    "export_format",
    "ui_tab",
    "export_copyright",
    "export_image_format",
    "export_texture_dir",
    "export_jpeg_quality",
    "export_image_quality",
    "export_texcoords",
    "export_normals",
    "export_materials",
    "use_selection",
    "use_visible",
    "use_renderable",
    "use_active_collection_with_nested",
    "use_active_collection",
    "collection",
    "export_yup",
    "export_hierarchy_flatten_objs",
    "will_save_settings",
    "export_loglevel",
    "filter_glob",
];

/// Props honoured by a feature file of a later milestone: each file registers the props it
/// implements (`static P: &[&str] = &["export_cameras"];`), so the operator stops warning about
/// them without anyone editing [`SUPPORTED_PROPS`] (M3+M4 W0 extension point).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static SUPPORTED_EXPORT_PROPS: [&'static [&'static str]];

/// Whether the exporter honours `prop` ([`SUPPORTED_PROPS`] or a [`SUPPORTED_EXPORT_PROPS`]
/// entry).
pub fn is_supported(prop: &str) -> bool {
    SUPPORTED_PROPS.contains(&prop) || SUPPORTED_EXPORT_PROPS.iter().any(|l| l.contains(&prop))
}

impl ExportSettings {
    /// Settings from the operator's typed props (unknown `export_format` items fall back to
    /// `GLB`, the add-on's index-0 item). `file_stem`, `blend_dir` and `generator` keep their
    /// defaults: the operator fills them from the resolved path and `bpy.data.filepath`.
    pub fn from_props(p: &ExportSceneGltf) -> ExportSettings {
        let d = ExportSettings::default();
        ExportSettings {
            export_format: ExportFormat::from_identifier(&p.export_format.value)
                .unwrap_or_default(),
            export_copyright: p.export_copyright.value.to_string(),
            export_yup: p.export_yup.value,
            export_normals: p.export_normals.value,
            export_texcoords: p.export_texcoords.value,
            export_materials: match p.export_materials.value {
                gltf::ExportMaterials::Export => ExportMaterials::Export,
                gltf::ExportMaterials::Placeholder => ExportMaterials::Placeholder,
                gltf::ExportMaterials::Viewport => ExportMaterials::Viewport,
                gltf::ExportMaterials::None => ExportMaterials::None,
            },
            export_image_format: match p.export_image_format.value {
                gltf::ExportImageFormat::Auto => ExportImageFormat::Auto,
                gltf::ExportImageFormat::Jpeg => ExportImageFormat::Jpeg,
                gltf::ExportImageFormat::Webp => ExportImageFormat::Webp,
                gltf::ExportImageFormat::None => ExportImageFormat::None,
            },
            // `export_jpeg_quality` is the add-on's legacy name; `export_image_quality` wins
            // unless only the legacy one was set.
            export_image_quality: {
                let q = if p.export_jpeg_quality.is_set && !p.export_image_quality.is_set {
                    p.export_jpeg_quality.value
                } else {
                    p.export_image_quality.value
                };
                q.clamp(0, 100) as u8
            },
            export_texture_dir: p.export_texture_dir.value.to_string(),
            use_selection: p.use_selection.value,
            use_visible: p.use_visible.value,
            use_renderable: p.use_renderable.value,
            use_active_collection: p.use_active_collection.value,
            // Coupling rule: only meaningful with use_active_collection.
            use_active_collection_with_nested: p.use_active_collection.value
                && p.use_active_collection_with_nested.value,
            collection: p.collection.value.to_string(),
            export_hierarchy_flatten_objs: p.export_hierarchy_flatten_objs.value,
            props: Arc::new(p.clone()),
            ..d
        }
    }

    /// The image format a texture is written as: `image/png` or `image/jpeg` (M1; WebP in M3).
    pub(crate) fn wants_jpeg(&self) -> bool {
        self.export_image_format == ExportImageFormat::Jpeg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator's arguments with nothing set (every prop at its RNA default).
    fn default_args() -> OpArgs {
        let mut a = OpArgs::default();
        for def in ExportSceneGltf::spec().props {
            a.values.insert(def.identifier, Value::default_for(def));
        }
        a
    }

    #[test]
    fn jpeg_quality_legacy_name() {
        let mut a = default_args();
        let i = a.values.get_index_of("export_jpeg_quality").unwrap();
        a.values.insert("export_jpeg_quality", Value::Int(40));
        a.is_set.insert(i);
        let s = ExportSettings::from_props(&ExportSceneGltf::from_args(&a));
        assert_eq!(s.export_image_quality, 40);
        assert_eq!(s.export_format, ExportFormat::Glb);
    }

    #[test]
    fn defaults_are_blenders() {
        let s = ExportSettings::default();
        assert_eq!(s.export_format, ExportFormat::Glb);
        assert!(s.export_yup && s.export_normals && s.export_texcoords);
        assert_eq!(s.export_materials, ExportMaterials::Export);
        assert_eq!(s.export_image_format, ExportImageFormat::Auto);
        assert_eq!(s.export_image_quality, 75);
        assert!(!s.use_selection && !s.use_visible && !s.use_renderable);
        assert!(!s.use_active_collection && s.use_active_collection_with_nested);
    }

    #[test]
    fn from_default_props_equals_default() {
        let p = ExportSceneGltf::from_args(&default_args());
        let s = ExportSettings::from_props(&p);
        let d = ExportSettings::default();
        assert_eq!(s.export_format, d.export_format);
        assert_eq!(s.export_yup, d.export_yup);
        assert_eq!(s.export_materials, d.export_materials);
        assert_eq!(s.export_image_quality, d.export_image_quality);
        // Coupling: with_nested is off when use_active_collection is off.
        assert!(!s.use_active_collection_with_nested);
    }

    #[test]
    fn supported_props_exist() {
        let spec = ExportSceneGltf::spec();
        for name in SUPPORTED_PROPS {
            assert!(
                spec.props.iter().any(|p| p.identifier == *name),
                "{name} is not an export_scene.gltf prop"
            );
        }
    }
}
