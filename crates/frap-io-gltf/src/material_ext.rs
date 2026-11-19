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
//!
//! WP 3.6 (owner after W0) registers the nine extensions of io_scene_gltf2 5.2.41
//! (clearcoat, transmission, volume, specular, sheen, anisotropy, iridescence, dispersion, ior;
//! `ior.rs` runs last and also applies the add-on's final `__check_volume` /
//! `__check_dispersion`), and adds the helpers below: [`ext_of`], [`import_texture`],
//! [`import_normal`] for the importers and [`bind_settings_nodes`], which the importer calls
//! after `raise` to give the "glTF Material Output" settings node its group datablock.

use frap_gltf::Extension;
use frap_gltf::json::{Extensions, Material as GMaterial, NormalTextureInfo, TextureInfo};
use frap_material::{ChannelSel, LoweredMaterial, NormalMap, TexRef};
use frap_mesh::Mesh;
use frap_nodes::builtin_groups::gltf_material_output;
use frap_rna::{IdRef, MaterialId, ReportType, Value};
use frap_scene::id::Material;
use frap_scene::{ChangeLog, Main};

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

// ------------------------------------------------------------------ helpers (WP 3.6)

/// The typed extension `E` of a glTF material; a malformed one is skipped with a WARNING.
///
/// Deviation (proposed for `docs/deviations.md`): the 5.2 add-on raises a Python
/// `RuntimeError` on a malformed extension (`clearcoatFactor: "abc"`) and on an extension
/// texture index out of range, so the whole import fails; frap imports the rest of the file
/// and names the extension in a WARNING.
pub fn ext_of<E: Extension>(host: &mut dyn ExtImportHost, m: &GMaterial) -> Option<E> {
    match m.extensions.get::<E>() {
        Ok(e) => e,
        Err(err) => {
            host.report(
                ReportType::Warning,
                format!("{}: ignored, malformed ({err})", E::NAME),
            );
            None
        }
    }
}

/// [`ExtImportHost::texture`] for a `textureInfo` member (the importer's `texture()` with
/// `is_data`; `channel` is the channel the raised graph reads).
pub fn import_texture(
    host: &mut dyn ExtImportHost,
    t: Option<&TextureInfo>,
    channel: ChannelSel,
    is_data: bool,
) -> Result<Option<TexRef>, ImportError> {
    match t {
        Some(t) => host.texture(t.index, t.tex_coord, &t.extensions, channel, is_data),
        None => Ok(None),
    }
}

/// `MaterialNormalTextureInfoClass` as a [`NormalMap`] (scale 1 when absent).
pub fn import_normal(
    host: &mut dyn ExtImportHost,
    t: Option<&NormalTextureInfo>,
) -> Result<Option<NormalMap>, ImportError> {
    let Some(t) = t else { return Ok(None) };
    Ok(host
        .texture(t.index, t.tex_coord, &t.extensions, ChannelSel::Rgb, true)?
        .map(|texture| NormalMap {
            texture,
            scale: t.scale.unwrap_or(1.0) as f32,
        }))
}

/// Binds the "glTF Material Output" group nodes that `frap_material::raise` created
/// (`make_settings_node`: group nodes with the add-on's inputs and no `node_tree`, because
/// `raise` cannot reach `bpy.data`) to the `bpy.data.node_groups` datablock of that name,
/// created with the add-on's interface when missing (`get_settings_group`). The import calls
/// it right after `raise`.
pub fn bind_settings_nodes(main: &mut Main, log: &mut ChangeLog, mat: &mut Material) {
    let unbound = |n: &frap_nodes::Node| {
        n.bl_idname == "ShaderNodeGroup"
            && n.group_tree().is_none()
            && gltf_material_output::INPUTS
                .iter()
                .all(|(name, _)| n.input_index(name).is_some())
    };
    if !mat.node_tree.nodes.iter().any(unbound) {
        return;
    }
    let group = match main.node_groups.find(gltf_material_output::NAME) {
        Some(g) => g,
        None => main.node_groups.insert(
            gltf_material_output::NAME,
            gltf_material_output::new_group(),
            log,
        ),
    };
    let Some(interface) = main.node_groups.get(group).map(|g| g.interface.clone()) else {
        return;
    };
    let tree = &mut mat.node_tree;
    for i in 0..tree.nodes.len() {
        if unbound(&tree.nodes[i]) {
            tree.nodes[i]
                .props
                .values
                .insert("node_tree", Value::Id(Some(group.any())));
            tree.sync_group_node(i, Some(&interface));
        }
    }
}

#[cfg(test)]
mod tests {
    //! WP 3.6 end to end: frap imports `import_ext.gltf` (one material, every
    //! `KHR_materials_*` extension, a texture on every texture member) into the same graph as
    //! Blender 5.2.4 and exports it again as Blender does (`import_ext.blender.json`, written
    //! by `frap-material/src/patterns/transmission/import_ext/dump.py`).

    use std::path::PathBuf;

    use frap_scene::{ChangeLog, Context, Preferences, startup};
    use serde_json::Value as J;

    use super::*;
    use crate::import::{ImportSettings, import_file};
    use crate::{ExportSettings, export};

    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../frap-material/src/patterns/transmission/import_ext")
    }

    fn close(a: &J, b: &J) -> bool {
        match (a, b) {
            (J::Number(x), J::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                (x - y).abs() <= 1e-6 * x.abs().max(1.0)
            }
            (J::Array(x), J::Array(y)) => {
                x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close(p, q))
            }
            (J::Object(x), J::Object(y)) => {
                x.len() == y.len()
                    && x.iter().all(|(k, v)| {
                        // Texture indices depend on the gather order; compared below.
                        k == "index" || y.get(k).is_some_and(|w| close(v, w))
                    })
            }
            _ => a == b,
        }
    }

    fn files(dir: &str) -> Vec<String> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir);
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|f| f.ends_with(".rs") && !f.starts_with('_'))
            .map(|f| format!("KHR_materials_{}", f.trim_end_matches(".rs")))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn one_registration_per_extension_file() {
        let mut ex: Vec<String> = exporters().iter().map(|e| e.name.to_owned()).collect();
        let mut im: Vec<String> = importers().iter().map(|e| e.name.to_owned()).collect();
        // `ior.rs` runs the add-on's final checks, so it comes last.
        assert_eq!(ex.last().map(String::as_str), Some("KHR_materials_ior"));
        ex.sort();
        im.sort();
        assert_eq!(ex, files("src/export/materials/ext"));
        assert_eq!(im, files("src/import/materials_ext"));
        for orders in [
            exporters().iter().map(|e| e.order).collect::<Vec<_>>(),
            importers().iter().map(|e| e.order).collect(),
        ] {
            let set: std::collections::BTreeSet<u16> = orders.iter().copied().collect();
            assert_eq!(set.len(), orders.len(), "distinct orders");
        }
    }

    #[test]
    fn import_and_export_every_extension_like_blender() {
        let dir = fixture();
        let dump: J = serde_json::from_str(
            &std::fs::read_to_string(dir.join("import_ext.blender.json")).unwrap(),
        )
        .unwrap();
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let report = import_file(
            &mut main,
            &win,
            &dir.join("import_ext.gltf"),
            &ImportSettings::default(),
            &mut log,
        )
        .unwrap();
        assert!(report.reports.is_empty(), "{:?}", report.reports);
        let m = main.materials.find("M").unwrap();
        let mat = main.materials.get(m).unwrap();
        let t = &mat.node_tree;
        let names: Vec<(&str, &str)> = t
            .nodes
            .iter()
            .map(|n| (n.name.as_str(), n.bl_idname))
            .collect();
        let want: Vec<(&str, &str)> = dump["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n[0].as_str().unwrap(), n[1].as_str().unwrap()))
            .collect();
        assert_eq!(names, want);
        let links: Vec<J> = t
            .links
            .iter()
            .map(|l| {
                serde_json::json!([
                    t.node(l.from_node).unwrap().name.as_str(),
                    l.from_socket.as_str(),
                    t.node(l.to_node).unwrap().name.as_str(),
                    l.to_socket.as_str()
                ])
            })
            .collect();
        assert_eq!(J::Array(links), dump["links"]);
        // The settings node is bound to the add-on's group datablock.
        let group = main.node_groups.find(gltf_material_output::NAME).unwrap();
        let g = t.node_by_name("Group").unwrap();
        assert_eq!(g.group_tree(), Some(group.any()));
        assert_eq!(main.users(group.any()), 1);
        // Colour spaces: Non-Color for the data textures, as `texture(is_data=…)` sets them.
        for (node, v) in dump["images"].as_object().unwrap() {
            let n = t.node_by_name(node).unwrap();
            let Some(Value::Id(Some(id))) = n.prop("image") else {
                panic!("{node} has no image")
            };
            let im = main.images.get(IdRef::from_any(*id).unwrap()).unwrap();
            assert_eq!(im.header.name.as_str(), v[0].as_str().unwrap(), "{node}");
            assert_eq!(
                im.colorspace_settings.name.as_str(),
                v[1].as_str().unwrap(),
                "{node}"
            );
        }

        // Export again: the material Blender writes after its own import.
        let prefs = Preferences::default();
        let ctx = Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let out = export(&main, &ctx, &ExportSettings::default()).unwrap();
        let doc = serde_json::to_value(&out.document).unwrap();
        let gm = doc["materials"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["name"] == "M")
            .unwrap();
        let want = &dump["again"];
        assert_eq!(gm["doubleSided"], want["doubleSided"]);
        let (ge, we) = (
            gm["extensions"].as_object().unwrap(),
            want["extensions"].as_object().unwrap(),
        );
        let mut keys: Vec<&String> = ge.keys().collect();
        let mut wkeys: Vec<&String> = we.keys().collect();
        keys.sort();
        wkeys.sort();
        assert_eq!(keys, wkeys);
        for (k, v) in we {
            assert!(close(&ge[k], v), "{k}: {} != {v}", ge[k]);
        }
        // The packed clearcoat image: one texture for both members, as in Blender.
        let cc = &ge["KHR_materials_clearcoat"];
        assert_eq!(
            cc["clearcoatTexture"]["index"],
            cc["clearcoatRoughnessTexture"]["index"]
        );
        for name in we.keys() {
            assert!(out.extensions_used.contains(name), "{name}");
        }
    }
}
