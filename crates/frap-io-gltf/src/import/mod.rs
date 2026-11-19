// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF → scene (inverse port of io_scene_gltf2 5.2.41 `blender/imp`, §17.2): `settings`,
//! `scenes`, `nodes`, `mesh`, `materials` (raise), `images`, `cameras`, `lights`, `skins`,
//! `animation`, `decompress`, `variants`. WP 1.17 adds the submodules here.
//!
//! [`import_file`] / [`import_bytes`] do what `bpy.ops.import_scene.gltf(filepath=…)` does to
//! `bpy.data` and the context scene, in-process and without an operator (the operator, its
//! reports and `//` paths are WP 1.23's). The pipeline mirrors `BlenderGlTF._create`:
//! read and check the file, compute the virtual node tree (`vnode.rs`), create the Blender
//! scenes and collections the glTF scenes need (`scenes.rs`), create every object depth-first
//! (`nodes.rs`: meshes, cameras, lights, empties), then select the imported objects, make the
//! default scene's first root active and exclude the other scenes' collections.
//!
//! M1 scope (static content): skins, morph targets and animations (M6), EXT_mesh_gpu_instancing,
//! KHR_materials_variants and compressed meshes (M7) are not imported yet; a file using them imports its static part and the report lists one
//! WARNING per missing feature.

mod cameras;
mod convert;
mod extras;
mod images;
mod lights;
mod materials;
/// `KHR_materials_*` importers, one file per extension (`materials_ext/<name>.rs`, WP 3.6),
/// each registering a [`crate::material_ext::MaterialExtImporter`] (M3 W0 extension point).
pub mod materials_ext {
    automod::dir!(pub "src/import/materials_ext");
}
mod mesh;
mod nodes;
mod scenes;
pub mod settings;
mod vnode;

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use frap_gltf::accessor::{AccessorData, load_buffers, read_accessor, uri_to_path};
use frap_gltf::{Document, GltfError};
use frap_rna::{IdRef, ImageId, MaterialId, ObjectId, Report, ReportType, SceneId};
use frap_scene::{ChangeLog, Main, WindowState};

pub use settings::{BoneHeuristic, ExportImportConvertLightingMode, ImportSettings, ImportShading};

use convert::Units;
use scenes::CollTarget;
use vnode::VTree;

/// Extensions the add-on imports (`glTFImporter.extensions_managed`): a file that *requires*
/// any other extension is refused with Blender's message.
pub const EXTENSIONS_MANAGED: [&str; 22] = [
    "KHR_materials_pbrSpecularGlossiness",
    "KHR_lights_punctual",
    "KHR_materials_unlit",
    "KHR_texture_transform",
    "KHR_materials_clearcoat",
    "KHR_mesh_quantization",
    "EXT_meshopt_compression",
    "KHR_meshopt_compression",
    "EXT_mesh_gpu_instancing",
    "KHR_draco_mesh_compression",
    "KHR_materials_variants",
    "KHR_materials_emissive_strength",
    "KHR_materials_transmission",
    "KHR_materials_specular",
    "KHR_materials_sheen",
    "KHR_materials_ior",
    "KHR_animation_pointer",
    "KHR_materials_volume",
    "EXT_texture_webp",
    "KHR_materials_anisotropy",
    "KHR_materials_dispersion",
    "KHR_materials_iridescence",
];

/// Why an import failed. The messages are the add-on's `ImportError` texts where it has one
/// (the operator reports them as `{'ERROR'}` and returns `{'CANCELLED'}`).
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The file could not be read.
    #[error("Couldn't read {path}: {message}")]
    Io {
        /// File path.
        path: PathBuf,
        /// OS message.
        message: String,
    },
    /// Not a glTF/GLB file, or invalid JSON / container.
    #[error("Couldn't parse glTF. Check that the file is valid ({0})")]
    Parse(GltfError),
    /// `asset.version` is not `"2.0"`.
    #[error("glTF version must be 2.0; got {0}")]
    Version(String),
    /// `extensionsRequired` names an extension missing from `extensionsUsed`.
    #[error("Extension required must be in Extension Used too")]
    RequiredNotUsed(String),
    /// `extensionsRequired` names an extension the importer does not handle.
    #[error("Extension {0} is not available on this addon version")]
    UnsupportedExtension(String),
    /// A buffer URI could not be loaded (`Missing resource, '<uri>'.`).
    #[error("Missing resource, '{0}'.")]
    MissingResource(String),
    /// Accessor or index data is invalid.
    #[error("Bad glTF: {0}")]
    Data(String),
    /// Content Blender imports but frap does not yet (named milestone).
    #[error("{0}")]
    NotImplemented(String),
}

impl From<GltfError> for ImportError {
    fn from(e: GltfError) -> Self {
        match e {
            GltfError::Load { uri, .. } => ImportError::MissingResource(uri),
            GltfError::Compressed { extension, .. } => ImportError::NotImplemented(format!(
                "{extension}: compressed buffer views are imported from M7"
            )),
            e => ImportError::Data(e.to_string()),
        }
    }
}

/// What an import produced.
#[derive(Clone, Debug, Default)]
pub struct ImportReport {
    /// Created objects, in creation order.
    pub objects: Vec<IdRef<ObjectId>>,
    /// Messages for the operator's reports (WARNING per feature not imported yet, ERROR for a
    /// missing image file, …), in emission order.
    pub reports: Vec<Report>,
}

/// Imports a `.glb`/`.gltf` file into `main`, in the context scene of `win`
/// (`bpy.ops.import_scene.gltf(filepath=path)`). Relative URIs resolve against the file's
/// directory.
pub fn import_file(
    main: &mut Main,
    win: &WindowState,
    path: &Path,
    settings: &ImportSettings,
    log: &mut ChangeLog,
) -> Result<ImportReport, ImportError> {
    let bytes = std::fs::read(path).map_err(|e| ImportError::Io {
        path: path.to_owned(),
        message: e.to_string(),
    })?;
    import_bytes(main, win, &bytes, path, settings, log)
}

/// [`import_file`] on bytes already read; `path` is the file's path (for relative URIs and
/// image file paths).
pub fn import_bytes(
    main: &mut Main,
    win: &WindowState,
    bytes: &[u8],
    path: &Path,
    settings: &ImportSettings,
    log: &mut ChangeLog,
) -> Result<ImportReport, ImportError> {
    let glb = frap_gltf::read(bytes).map_err(ImportError::Parse)?;
    let doc = glb.document;
    check_version(&doc)?;
    checks(&doc)?;
    let dir = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let buffers = load_buffers(&doc, glb.bin.as_deref(), |uri| {
        std::fs::read(dir.join(uri_to_path(uri))).map_err(|e| e.to_string())
    })?;
    let scene = win.scene;
    let scale_length = main
        .scenes
        .get(scene)
        .map(|s| s.unit_settings.scale_length)
        .unwrap_or(1.0);
    let units = Units::new(scale_length);
    let mut cx = Gltf {
        vtree: VTree::compute(&doc, units),
        doc,
        buffers,
        dir,
        settings,
        units,
        reports: Vec::new(),
        warned: BTreeSet::new(),
        materials: HashMap::new(),
        images: HashMap::new(),
        roughness_images: HashMap::new(),
        mesh_cache: HashMap::new(),
        scene,
        view_layer: win.view_layer.clone(),
        collections: HashMap::new(),
        blender_scenes: Vec::new(),
        active_collection: CollTarget::Master(scene),
        objects: Vec::new(),
    };
    cx.warn_unsupported();
    scenes::create(&mut cx, main, log)?;
    if settings.import_unused_materials {
        materials::create_unused(&mut cx, main, log)?;
    }
    Ok(ImportReport {
        objects: cx.objects,
        reports: cx.reports,
    })
}

/// A glTF `name` as the add-on's falsy fallbacks see it (`name or default`, `if not name`):
/// an empty string counts as no name. Lights (`'name' not in pylight`) and materials
/// (`name is None`) test presence instead and keep `Some("")`.
pub(crate) fn truthy_name(name: &Option<String>) -> Option<String> {
    name.clone().filter(|n| !n.is_empty())
}

/// `glTFImporter.check_version`.
fn check_version(doc: &Document) -> Result<(), ImportError> {
    let v = doc.asset.version.as_str();
    if v != "2.0" {
        return Err(ImportError::Version(v.to_owned()));
    }
    Ok(())
}

/// `glTFImporter.checks`.
fn checks(doc: &Document) -> Result<(), ImportError> {
    for ext in &doc.extensions_required {
        if !doc.extensions_used.contains(ext) {
            return Err(ImportError::RequiredNotUsed(ext.clone()));
        }
        if !EXTENSIONS_MANAGED.contains(&ext.as_str()) {
            return Err(ImportError::UnsupportedExtension(ext.clone()));
        }
    }
    Ok(())
}

/// Cache key of a Blender mesh created from a glTF mesh (`BlenderNode.create_mesh_object`):
/// instances with the same skin (and, with morph targets, the same weights) share one mesh.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct MeshKey {
    pub mesh: usize,
    pub skin: Option<u32>,
    /// Bit patterns of the node's morph weights (only when the mesh has shape keys).
    pub weights: Vec<u64>,
}

/// The importer state (`gltf` in the add-on).
pub(crate) struct Gltf<'a> {
    pub doc: Document,
    pub buffers: Vec<Vec<u8>>,
    /// Directory of the imported file.
    pub dir: PathBuf,
    pub settings: &'a ImportSettings,
    pub units: Units,
    pub reports: Vec<Report>,
    warned: BTreeSet<String>,
    pub vtree: VTree,
    /// `pymaterial.blender_material[vertex_color]`: (material index, uses COLOR_0).
    pub materials: HashMap<(usize, bool), IdRef<MaterialId>>,
    /// `pyimage.blender_image_name`.
    pub images: HashMap<usize, Option<IdRef<ImageId>>>,
    /// `pyimage.blender_roughness_image_name`: the roughness image baked from a glTF image
    /// for `KHR_materials_pbrSpecularGlossiness` (one per image, whatever the factor).
    pub roughness_images: HashMap<usize, IdRef<ImageId>>,
    /// `pymesh.blender_name[cache_key]`: the Blender mesh, or point cloud with
    /// `import_point_as_pointcloud` (WP 3.7).
    pub mesh_cache: HashMap<MeshKey, frap_rna::IdAny>,
    /// The context scene (`gltf.blender_scene`).
    pub scene: IdRef<SceneId>,
    /// The context view layer name.
    pub view_layer: smol_str::SmolStr,
    /// `gltf.blender_collections`: glTF scene index (`None`: orphan nodes) → collection.
    pub collections: HashMap<Option<usize>, CollTarget>,
    /// `gltf.blender_scenes`, per glTF scene index.
    pub blender_scenes: Vec<IdRef<SceneId>>,
    /// `bpy.context.collection` at import time.
    pub active_collection: CollTarget,
    /// Objects created, in order.
    pub objects: Vec<IdRef<ObjectId>>,
}

impl Gltf<'_> {
    /// Decodes accessor `i`.
    pub fn accessor(&self, i: u32) -> Result<AccessorData, ImportError> {
        Ok(read_accessor(&self.doc, &self.buffers, i)?)
    }

    /// Decodes accessor `i` as `N`-component f32 elements (normalized integers dequantized).
    pub fn accessor_f32<const N: usize>(&self, i: u32) -> Result<Vec<[f32; N]>, ImportError> {
        let a = self.accessor(i)?;
        let n = a.type_.components();
        if n == N {
            return Ok(a.to_f32_elements::<N>()?);
        }
        // The add-on's numpy code keeps the accessor's own width; frap pads/truncates to the
        // width the caller stores (e.g. VEC3 colours → RGBA, done by the caller).
        let flat = a.to_f32();
        Ok(flat
            .chunks_exact(n.max(1))
            .map(|c| std::array::from_fn(|k| c.get(k).copied().unwrap_or(0.0)))
            .collect())
    }

    /// Adds a report.
    pub fn report(&mut self, level: ReportType, message: impl Into<String>) {
        self.reports.push(Report::new(level, message));
    }

    /// One WARNING per feature that is not imported yet.
    pub fn warn_once(&mut self, key: &str, message: impl Into<String>) {
        if self.warned.insert(key.to_owned()) {
            self.report(ReportType::Warning, message);
        }
    }

    /// Warns about every feature of the file that M1 does not import.
    fn warn_unsupported(&mut self) {
        let doc = &self.doc;
        let mut w: Vec<(&str, String)> = Vec::new();
        if !doc.skins.is_empty() {
            w.push((
                "skins",
                "glTF import: skins are imported from M6; joints become empties and meshes are not skinned".into(),
            ));
        }
        if !doc.animations.is_empty() {
            w.push((
                "animations",
                "glTF import: animations are imported from M6; they were skipped".into(),
            ));
        }
        if doc
            .meshes
            .iter()
            .any(|m| m.primitives.iter().any(|p| !p.targets.is_empty()))
        {
            w.push((
                "morph",
                "glTF import: morph targets (shape keys) are imported from M6; they were skipped"
                    .into(),
            ));
        }
        for (ext, milestone) in [
            ("EXT_mesh_gpu_instancing", "M7"),
            ("KHR_materials_variants", "M7"),
        ] {
            if doc.extensions_used.iter().any(|e| e == ext) {
                w.push((
                    ext,
                    format!("glTF import: {ext} is imported from {milestone}; it was ignored"),
                ));
            }
        }
        for (k, m) in w {
            self.warn_once(k, m);
        }
    }
}
