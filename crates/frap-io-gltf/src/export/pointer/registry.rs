// SPDX-License-Identifier: GPL-3.0-or-later
//! The add-on's `export_settings['KHR_animation_pointer']`: for every exported material, light
//! and camera, the glTF object it became and the Blender properties its gatherer read, each
//! with the JSON pointer it fills (`/materials/XXX/…`, `XXX` standing for the index).
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/gather.py` (the three dicts), `blender/exp/nodes.py` (`__gather_camera` and
//! `__gather_extensions` register every exported camera and light with the paths
//! `cameras.py` / `lights.py` / `light_spots.py` stored in `current_paths`),
//! `blender/exp/material/materials.py` (`gather_material` registers a material only when it
//! is read from its original node tree, `bmat.used == "ORIGINAL"`, which with
//! `export_pointer_animation` means its node tree has animation data; `get_final_material`
//! sets `glTF_material`).
//!
//! The dicts are keyed by `id(datablock)` and filled in the order the gatherers run: a node's
//! mesh (its materials) and camera before its children, its light extension after them, nodes
//! depth first from each scene's roots. frap gathers first and registers afterwards, walking
//! the finished document in that same order ([`build`]): the gathered values do not depend on
//! the registration, only the paths and the order do.

use std::collections::HashMap;

use frap_gltf::ext::khr_lights_punctual::KhrLightsPunctualNode;
use frap_gltf::json::Index;
use frap_rna::{CameraId, IdRef, LightId, MaterialId, ObjectId};
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::export::ExportMaterials;
use crate::export::gather::Gather;
use crate::export::node_map::NodeMap;

/// The kind of datablock a registered asset is (`blender_type_data`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DataKind {
    /// `'materials'` (the properties live in the material's embedded node tree).
    Materials,
    /// `'lights'`
    Lights,
    /// `'cameras'`
    Cameras,
}

/// One property a gatherer read (`export_settings['current_paths'][key]`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PathInfo {
    /// `path`: the JSON pointer with `XXX` for the object's index.
    pub pointer: String,
    /// `length`: the number of components exported.
    pub length: usize,
    /// `additional_path`: the alpha of a base colour (and the colour of an alpha), the
    /// `spot_size` an inner cone angle needs.
    pub additional_path: Option<String>,
    /// `strength_channel` of `emissiveFactor`: the emission strength to multiply by.
    pub strength_channel: Option<String>,
    /// `factor_channel` of `emissiveStrength`: the emission colour it scales.
    pub factor_channel: Option<String>,
    /// `lamp_type` of a light intensity (`POINT`, `SPOT`, `SUN`).
    pub lamp_type: Option<SmolStr>,
    /// `reverse` of `occlusionTexture/strength`: the Mix's white is its `B` input, so the
    /// strength is `1 - Factor` (WP 7.7).
    pub reverse: bool,
    /// `vector_type` of the Mapping node a `KHR_texture_transform` path belongs to (`POINT`,
    /// `TEXTURE`, `VECTOR`; WP 7.7).
    pub vector_type: Option<SmolStr>,
    /// `additional`: further pointers the same property fills (one Mapping node or Normal Map
    /// feeding several textures). Each becomes a copy of the channel with its own accessors
    /// (`gather_data_sampled_channels`' deep copies, WP 7.7).
    pub additional: Vec<String>,
}

impl PathInfo {
    /// A plain path of `length` components.
    pub fn new(pointer: &str, length: usize) -> PathInfo {
        PathInfo {
            pointer: pointer.to_owned(),
            length,
            ..PathInfo::default()
        }
    }

    /// The pointer with the object's index (`path.replace("XXX", str(index))`).
    pub fn resolved(&self, index: Index) -> String {
        self.pointer.replace("XXX", &index.to_string())
    }

    /// The [`Self::additional`] pointers with the object's index.
    pub fn resolved_additional(&self, index: Index) -> Vec<String> {
        self.additional
            .iter()
            .map(|p| p.replace("XXX", &index.to_string()))
            .collect()
    }
}

/// A registered datablock.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// The material, light or camera.
    pub id: frap_rna::IdAny,
    /// `glTF_material` / `glTF_light` / `glTF_camera`: its index in the document.
    pub index: Index,
    /// `paths`, keyed by the Blender data path relative to the datablock
    /// (`node_tree.nodes["Principled BSDF"].inputs[0].default_value`, `energy`, `angle`, …),
    /// in registration order.
    pub paths: IndexMap<String, PathInfo>,
}

/// `export_settings['KHR_animation_pointer']`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Registry {
    /// Materials whose node tree is animated, in gather order.
    pub materials: Vec<Asset>,
    /// Exported lights, in gather order.
    pub lights: Vec<Asset>,
    /// Exported cameras, in gather order.
    pub cameras: Vec<Asset>,
}

impl Registry {
    /// The assets of `kind`.
    pub fn of(&self, kind: DataKind) -> &[Asset] {
        match kind {
            DataKind::Materials => &self.materials,
            DataKind::Lights => &self.lights,
            DataKind::Cameras => &self.cameras,
        }
    }
}

/// Registers `id` (first registration keeps its place; later ones replace index and paths, as
/// re-assigning a dict entry does).
fn register(
    list: &mut Vec<Asset>,
    id: frap_rna::IdAny,
    index: Index,
    paths: IndexMap<String, PathInfo>,
) {
    match list.iter_mut().find(|a| a.id == id) {
        Some(a) => {
            a.index = index;
            a.paths = paths;
        }
        None => list.push(Asset { id, index, paths }),
    }
}

/// `cameras.py`: the paths of a camera (`clip_end` / `clip_start` / `angle` for a perspective
/// one, the two magnifications and the clip distances for an orthographic one).
pub fn camera_paths(cam_type: &str) -> IndexMap<String, PathInfo> {
    let mut p = IndexMap::new();
    match cam_type {
        "ORTHO" => {
            p.insert(
                "ortho_scale_x".into(),
                PathInfo::new("/cameras/XXX/orthographic/xmag", 1),
            );
            p.insert(
                "ortho_scale_y".into(),
                PathInfo::new("/cameras/XXX/orthographic/ymag", 1),
            );
            p.insert(
                "clip_end".into(),
                PathInfo::new("/cameras/XXX/orthographic/zfar", 1),
            );
            p.insert(
                "clip_start".into(),
                PathInfo::new("/cameras/XXX/orthographic/znear", 1),
            );
        }
        _ => {
            p.insert(
                "clip_end".into(),
                PathInfo::new("/cameras/XXX/perspective/zfar", 1),
            );
            p.insert(
                "clip_start".into(),
                PathInfo::new("/cameras/XXX/perspective/znear", 1),
            );
            p.insert(
                "angle".into(),
                PathInfo::new("/cameras/XXX/perspective/yfov", 1),
            );
        }
    }
    p
}

/// `lights.py` / `light_spots.py` without an emission node (frap models no light node trees,
/// and the add-on ignores them under `BLENDER_EEVEE` anyway): `color`, `energy`, the spot's
/// `spot_blend` (inner cone, with `spot_size`) and `spot_size` (outer cone), and
/// `cutoff_distance` when `use_custom_distance` is on.
pub fn light_paths(light_type: &str, use_custom_distance: bool) -> IndexMap<String, PathInfo> {
    let mut p = IndexMap::new();
    p.insert(
        "color".into(),
        PathInfo::new("/extensions/KHR_lights_punctual/lights/XXX/color", 3),
    );
    p.insert(
        "energy".into(),
        PathInfo {
            lamp_type: Some(light_type.into()),
            ..PathInfo::new("/extensions/KHR_lights_punctual/lights/XXX/intensity", 1)
        },
    );
    if light_type == "SPOT" {
        p.insert(
            "spot_blend".into(),
            PathInfo {
                additional_path: Some("spot_size".into()),
                ..PathInfo::new(
                    "/extensions/KHR_lights_punctual/lights/XXX/spot.innerConeAngle",
                    1,
                )
            },
        );
        p.insert(
            "spot_size".into(),
            PathInfo::new(
                "/extensions/KHR_lights_punctual/lights/XXX/spot.outerConeAngle",
                1,
            ),
        );
    }
    if use_custom_distance {
        p.insert(
            "cutoff_distance".into(),
            PathInfo::new("/extensions/KHR_lights_punctual/lights/XXX/range", 1),
        );
    }
    p
}

/// Builds the registry from the gathered document (see the module documentation).
pub fn build(g: &mut Gather) -> Registry {
    // Node index → object (an object linked into several scenes has one node per scene).
    let node_objects: HashMap<Index, IdRef<ObjectId>> = g
        .state::<NodeMap>()
        .objects
        .iter()
        .map(|(&(_, ob), &n)| (n, ob))
        .collect();
    let main = g.main;
    let doc = &g.doc;
    // Material name → datablock (names are unique).
    let materials_by_name: HashMap<&str, IdRef<MaterialId>> = main
        .materials
        .iter()
        .map(|(r, m)| (m.header.name.as_str(), r))
        .collect();
    let mut reg = Registry::default();
    // Materials the add-on's sampler fails on (a registered property without a path).
    let mut failing: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Index, bool)> = Vec::new();
    for s in &doc.scenes {
        for &root in s.nodes.iter().rev() {
            stack.push((root, false));
        }
    }
    // Depth first: (node, false) = enter (mesh, camera), (node, true) = leave (light).
    while let Some((n, leaving)) = stack.pop() {
        let Some(node) = doc.nodes.get(n as usize) else {
            continue;
        };
        let ob = node_objects.get(&n).and_then(|&r| main.objects.get(r));
        if leaving {
            if let (Some(ob), Ok(Some(ext))) = (ob, node.extensions.get::<KhrLightsPunctualNode>())
                && let Some(r) = ob.data.and_then(|d| d.typed::<LightId>())
                && let Some(la) = main.lights.get(r)
            {
                let paths = light_paths(la.r#type.as_str(), la.fields.use_custom_distance);
                register(&mut reg.lights, r.any(), ext.light, paths);
            }
            continue;
        }
        if !seen.insert(n) {
            continue;
        }
        if let Some(m) = node.mesh.and_then(|m| doc.meshes.get(m as usize))
            && g.settings.export_materials == ExportMaterials::Export
        {
            for p in &m.primitives {
                let Some(mi) = p.material else { continue };
                let Some(name) = doc
                    .materials
                    .get(mi as usize)
                    .and_then(|gm| gm.name.as_deref())
                else {
                    continue;
                };
                let Some(&r) = materials_by_name.get(name) else {
                    continue;
                };
                let Some(mat) = main.materials.get(r) else {
                    continue;
                };
                // `bmat.used == "ORIGINAL"`: read from the original tree because it is animated.
                if mat.node_tree.animation_data.is_none() {
                    continue;
                }
                let image_none = g.settings.export_image_format
                    == crate::export::settings::ExportImageFormat::None;
                let gm = &doc.materials[mi as usize];
                let lm = crate::export::materials::lower::lowered_for(main, mat, g.settings);
                let orm = crate::export::materials::core_occlusion::orm_inputs(g, &lm).is_some();
                let found = super::material::paths(main, mat, gm, image_none, orm);
                if found.paths.is_empty() {
                    continue;
                }
                if !found.none_keys.is_empty() && !failing.iter().any(|(n, _)| *n == name) {
                    failing.push((name.to_owned(), found.none_keys.join(", ")));
                }
                register(&mut reg.materials, r.any(), mi, found.paths);
            }
        }
        if let (Some(ob), Some(ci)) = (ob, node.camera)
            && let Some(r) = ob.data.and_then(|d| d.typed::<CameraId>())
            && let Some(cam) = main.cameras.get(r)
        {
            register(
                &mut reg.cameras,
                r.any(),
                ci,
                camera_paths(cam.fields.r#type.as_str()),
            );
        }
        stack.push((n, true));
        for &c in node.children.iter().rev() {
            stack.push((c, false));
        }
    }
    for (name, members) in failing {
        g.warn(format!(
            "frap: export_scene.gltf: material '{name}': {members} has no animatable source \
             (Blender's KHR_animation_pointer export fails here: 'NoneType' object has no \
             attribute 'startswith'); its other channels are exported"
        ));
    }
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_paths_follow_cameras_py() {
        let p = camera_paths("PERSP");
        let keys: Vec<&str> = p.keys().map(String::as_str).collect();
        assert_eq!(keys, ["clip_end", "clip_start", "angle"]);
        assert_eq!(p["angle"].resolved(2), "/cameras/2/perspective/yfov");
        let p = camera_paths("ORTHO");
        let keys: Vec<&str> = p.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            ["ortho_scale_x", "ortho_scale_y", "clip_end", "clip_start"]
        );
    }

    #[test]
    fn light_paths_follow_lights_py() {
        let p = light_paths("SPOT", true);
        let keys: Vec<&str> = p.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "color",
                "energy",
                "spot_blend",
                "spot_size",
                "cutoff_distance"
            ]
        );
        assert_eq!(
            p["spot_blend"].additional_path.as_deref(),
            Some("spot_size")
        );
        assert_eq!(p["energy"].lamp_type.as_deref(), Some("SPOT"));
        let p = light_paths("SUN", false);
        assert_eq!(p.len(), 2);
    }
}
