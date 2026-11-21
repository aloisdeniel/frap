// SPDX-License-Identifier: GPL-3.0-or-later
//! The order of `extensionsUsed` / `extensionsRequired` (M3+M4 stabilization round 4).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/exporter.py` (`GlTF2Exporter.__traverse`,
//! `__traverse_property`), `blender/exp/material/materials.py` (`__gather_extensions`) and
//! `blender/exp/export.py` (`__manage_extension_declaration`), Apache-2.0, The glTF-Blender-IO
//! authors. The add-on declares an extension when its traversal of the gathered tree first
//! meets it:
//!
//! - scenes in order, then depth first; every object's members in `dir()` order, which is
//!   alphabetical (a node's `children` before its `extensions` and its `mesh`; a material's
//!   `emissive_texture`, then `extensions`, then `normal_texture`, `occlusion_texture`,
//!   `pbr_metallic_roughness`); a child-of-root object (node, mesh, material, texture) only at
//!   its first reference;
//! - an extension's own content (the `KHR_texture_transform` of its textures) before its name;
//! - a material that is not unlit carries an `Extension` object for clearcoat, transmission,
//!   emissive strength, specular, anisotropy and IOR **even when it is empty**, and one for
//!   dispersion whenever it has a volume (`export_dispersion` only checks for the volume
//!   extension and the glTF Material Output node's Dispersion socket), in
//!   `__gather_extensions`' order; empty ones are dropped from the JSON at the end, and so are
//!   their declarations when no material keeps them, but the declarations that stay keep the
//!   traversal order. So two imported files whose first material is transmission and second
//!   clearcoat declare `[clearcoat, transmission]` (verified on 5.2.4 with every import order), and
//!   volume, sheen, then dispersion materials declare `[…, dispersion, sheen]` (the volume
//!   material's empty dispersion object comes first; M34S5-2).
//!
//! - `export_iridescence` also returns a (possibly empty) object for every material with a
//!   Principled BSDF and a glTF Material Output group with the iridescence sockets (the group
//!   the importer creates for any glTF material that needed one), so such a material declares
//!   iridescence at its place in the traversal even when its own object is dropped;
//!   `export_dispersion` returns one whenever the material has a volume and the group's
//!   Dispersion socket.
//!
//! frap records the extensions while gathering (first-use order, [`Gather::use_extension`]),
//! and per glTF material the empty objects the add-on would create beyond the always-created
//! ones ([`CreatedEmpty`], filled by `materials::gather` from the lowered material);
//! [`reorder`] then sorts the declared names by this traversal of the finished document.
//! Approximation: every non-unlit material counts as having a Principled BSDF with unlinked
//! Anisotropic and IOR sockets (the add-on skips specular, anisotropy and IOR otherwise), which
//! only matters for the relative order of extensions that a later material keeps.
//!
//! [`Gather::use_extension`]: crate::export::gather::Gather::use_extension

use std::collections::{HashMap, HashSet};

use frap_gltf::json::{Document, Extensions, Index};
use serde_json::Value;

/// `__gather_extensions`' order for a material that is not unlit.
const MATERIAL_EXTENSIONS: [&str; 10] = [
    "KHR_materials_clearcoat",
    "KHR_materials_transmission",
    "KHR_materials_emissive_strength",
    "KHR_materials_volume",
    "KHR_materials_specular",
    "KHR_materials_sheen",
    "KHR_materials_anisotropy",
    "KHR_materials_iridescence",
    "KHR_materials_dispersion",
    "KHR_materials_ior",
];

/// The ones the add-on creates (possibly empty) for every Principled material.
const ALWAYS_CREATED: [&str; 6] = [
    "KHR_materials_clearcoat",
    "KHR_materials_transmission",
    "KHR_materials_emissive_strength",
    "KHR_materials_specular",
    "KHR_materials_anisotropy",
    "KHR_materials_ior",
];

const UNLIT: &str = "KHR_materials_unlit";
const VOLUME: &str = "KHR_materials_volume";
const DISPERSION: &str = "KHR_materials_dispersion";

/// Per glTF material index: the extensions whose (possibly empty) object the add-on's
/// `__gather_extensions` creates for it besides [`ALWAYS_CREATED`] (iridescence for a
/// material with a Principled BSDF and the glTF Material Output group's iridescence sockets,
/// dispersion for one with a volume and the group's Dispersion socket). A `Gather` state.
#[derive(Clone, Debug, Default)]
pub struct CreatedEmpty(pub HashMap<Index, Vec<&'static str>>);

struct Walk<'d> {
    doc: &'d Document,
    created: Option<&'d CreatedEmpty>,
    order: Vec<String>,
    nodes: HashSet<Index>,
    meshes: HashSet<Index>,
    materials: HashSet<Index>,
    textures: HashSet<Index>,
}

impl Walk<'_> {
    fn declare(&mut self, name: &str) {
        if !self.order.iter().any(|n| n == name) {
            self.order.push(name.to_owned());
        }
    }

    /// An `extensions` dict: each entry's content, then its name.
    fn extensions(&mut self, e: &Extensions) {
        for (name, v) in &e.0 {
            self.json(v);
            self.declare(name);
        }
    }

    /// Extension content: nested `extensions` objects and texture references (a texture
    /// info's `extensions` sort before its `index`).
    fn json(&mut self, v: &Value) {
        match v {
            Value::Object(o) => {
                let mut keys: Vec<&String> = o.keys().collect();
                keys.sort();
                for k in keys {
                    match (k.as_str(), &o[k]) {
                        ("extensions", Value::Object(ext)) => {
                            for (name, inner) in ext {
                                self.json(inner);
                                self.declare(name);
                            }
                        }
                        ("index", Value::Number(n)) => {
                            if let Some(i) = n.as_u64() {
                                self.texture(i as Index);
                            }
                        }
                        (_, inner) => self.json(inner),
                    }
                }
            }
            Value::Array(a) => a.iter().for_each(|x| self.json(x)),
            _ => {}
        }
    }

    fn texture(&mut self, i: Index) {
        if !self.textures.insert(i) {
            return;
        }
        if let Some(t) = self.doc.textures.get(i as usize) {
            self.extensions(&t.extensions);
        }
    }

    fn texture_info(&mut self, extensions: &Extensions, index: Index) {
        self.extensions(extensions);
        self.texture(index);
    }

    fn node(&mut self, i: Index) {
        if !self.nodes.insert(i) {
            return;
        }
        let Some(n) = self.doc.nodes.get(i as usize) else {
            return;
        };
        for &c in &n.children {
            self.node(c);
        }
        self.extensions(&n.extensions);
        if let Some(m) = n.mesh {
            self.mesh(m);
        }
    }

    fn mesh(&mut self, i: Index) {
        if !self.meshes.insert(i) {
            return;
        }
        let Some(m) = self.doc.meshes.get(i as usize) else {
            return;
        };
        self.extensions(&m.extensions);
        for p in &m.primitives {
            self.extensions(&p.extensions);
            if let Some(mat) = p.material {
                self.material(mat);
            }
        }
    }

    fn material(&mut self, i: Index) {
        if !self.materials.insert(i) {
            return;
        }
        let Some(m) = self.doc.materials.get(i as usize) else {
            return;
        };
        if let Some(t) = &m.emissive_texture {
            self.texture_info(&t.extensions, t.index);
        }
        if m.extensions.0.contains_key(UNLIT) {
            self.extensions(&m.extensions);
        } else {
            for name in MATERIAL_EXTENSIONS {
                match m.extensions.0.get(name) {
                    Some(v) => {
                        self.json(v);
                        self.declare(name);
                    }
                    None if ALWAYS_CREATED.contains(&name) => self.declare(name),
                    // Recorded by the material gatherer (iridescence, dispersion).
                    None if self
                        .created
                        .and_then(|c| c.0.get(&i))
                        .is_some_and(|c| c.contains(&name)) =>
                    {
                        self.declare(name)
                    }
                    // Without a record (a document built by hand): `export_dispersion`
                    // creates a (possibly empty) object whenever the material has a volume
                    // (and a glTF Material Output node with a Dispersion socket, which every
                    // material a volume comes from has).
                    None if self.created.is_none()
                        && name == DISPERSION
                        && m.extensions.0.contains_key(VOLUME) =>
                    {
                        self.declare(name)
                    }
                    None => {}
                }
            }
            for (name, v) in &m.extensions.0 {
                if !MATERIAL_EXTENSIONS.contains(&name.as_str()) {
                    self.json(v);
                    self.declare(name);
                }
            }
        }
        if let Some(t) = &m.normal_texture {
            self.texture_info(&t.extensions, t.index);
        }
        if let Some(t) = &m.occlusion_texture {
            self.texture_info(&t.extensions, t.index);
        }
        if let Some(pbr) = &m.pbr_metallic_roughness {
            if let Some(t) = &pbr.base_color_texture {
                self.texture_info(&t.extensions, t.index);
            }
            self.extensions(&pbr.extensions);
            if let Some(t) = &pbr.metallic_roughness_texture {
                self.texture_info(&t.extensions, t.index);
            }
        }
    }
}

/// The add-on's traversal order of the extensions met in `doc` (see the module doc).
fn traversal_order(doc: &Document, created: Option<&CreatedEmpty>) -> Vec<String> {
    let mut w = Walk {
        doc,
        created,
        order: Vec::new(),
        nodes: HashSet::new(),
        meshes: HashSet::new(),
        materials: HashSet::new(),
        textures: HashSet::new(),
    };
    for s in &doc.scenes {
        w.extensions(&s.extensions);
        for &n in &s.nodes {
            w.node(n);
        }
    }
    // Objects no scene reaches (unused textures, other root members), then the root.
    for i in 0..doc.materials.len() {
        w.material(i as Index);
    }
    for i in 0..doc.textures.len() {
        w.texture(i as Index);
    }
    w.extensions(&doc.extensions);
    // `manage_gpu_instancing_nodes` traverses the instance holders' extensions after every
    // scene and animation (M7 WP 7.5), so `EXT_mesh_gpu_instancing` is declared after them.
    let mut order = w.order;
    if let Some(i) = order.iter().position(|n| n == "EXT_mesh_gpu_instancing") {
        let n = order.remove(i);
        order.push(n);
    }
    order
}

fn sort_by(names: &mut [String], order: &[String]) {
    // Stable: names the traversal does not meet keep their place after the others.
    names.sort_by_key(|n| order.iter().position(|o| o == n).unwrap_or(usize::MAX));
}

/// Puts `extensionsUsed` and `extensionsRequired` in the add-on's declaration order.
pub fn reorder(doc: &mut Document) {
    reorder_with(doc, None);
}

/// [`reorder`] with the material gatherer's record of empty extension objects.
pub fn reorder_with(doc: &mut Document, created: Option<&CreatedEmpty>) {
    let order = traversal_order(doc, created);
    sort_by(&mut doc.extensions_used, &order);
    sort_by(&mut doc.extensions_required, &order);
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // documents built field by field
mod tests {
    use super::*;
    use frap_gltf::json::{Material, Mesh, Node, Primitive, Scene};

    fn material(name: &str, ext: &[&str]) -> Material {
        let mut m = Material {
            name: Some(name.into()),
            ..Material::default()
        };
        for e in ext {
            m.extensions.0.insert((*e).into(), serde_json::json!({}));
        }
        m
    }

    #[test]
    fn principled_materials_declare_in_gather_order() {
        // Transmission first in the file, clearcoat second: the first material's empty
        // clearcoat object already declared clearcoat.
        let mut doc = Document::default();
        doc.materials = vec![
            material("Frosted", &["KHR_materials_transmission"]),
            material("Lacquer", &["KHR_materials_clearcoat"]),
            material("Velvet", &["KHR_materials_sheen"]),
        ];
        doc.meshes = (0..3)
            .map(|i| Mesh {
                primitives: vec![Primitive {
                    material: Some(i),
                    ..Primitive::default()
                }],
                ..Mesh::default()
            })
            .collect();
        doc.nodes = (0..3)
            .map(|i| Node {
                mesh: Some(i),
                ..Node::default()
            })
            .collect();
        doc.scenes = vec![Scene {
            nodes: vec![2, 0, 1],
            ..Scene::default()
        }];
        doc.extensions_used = vec![
            "KHR_materials_sheen".into(),
            "KHR_materials_transmission".into(),
            "KHR_materials_clearcoat".into(),
        ];
        reorder(&mut doc);
        assert_eq!(
            doc.extensions_used,
            [
                "KHR_materials_clearcoat",
                "KHR_materials_transmission",
                "KHR_materials_sheen"
            ]
        );
    }

    #[test]
    fn a_volume_declares_dispersion_before_a_later_sheen() {
        // Volume, sheen and dispersion materials in that order (M34S5-2).
        let mut doc = Document::default();
        doc.materials = vec![
            material("Glass", &["KHR_materials_transmission", VOLUME]),
            material("Velvet", &["KHR_materials_sheen"]),
            material("Prism", &["KHR_materials_transmission", VOLUME, DISPERSION]),
        ];
        doc.meshes = (0..3)
            .map(|i| Mesh {
                primitives: vec![Primitive {
                    material: Some(i),
                    ..Primitive::default()
                }],
                ..Mesh::default()
            })
            .collect();
        doc.nodes = (0..3)
            .map(|i| Node {
                mesh: Some(i),
                ..Node::default()
            })
            .collect();
        doc.scenes = vec![Scene {
            nodes: vec![0, 1, 2],
            ..Scene::default()
        }];
        doc.extensions_used = vec![
            "KHR_materials_transmission".into(),
            VOLUME.into(),
            "KHR_materials_sheen".into(),
            DISPERSION.into(),
        ];
        reorder(&mut doc);
        assert_eq!(
            doc.extensions_used,
            [
                "KHR_materials_transmission",
                VOLUME,
                DISPERSION,
                "KHR_materials_sheen"
            ]
        );
    }

    #[test]
    fn unlit_and_children_first() {
        let mut doc = Document::default();
        doc.materials = vec![
            material("Flat", &[UNLIT]),
            material("Lacquer", &["KHR_materials_clearcoat"]),
        ];
        doc.meshes = (0..2)
            .map(|i| Mesh {
                primitives: vec![Primitive {
                    material: Some(i),
                    ..Primitive::default()
                }],
                ..Mesh::default()
            })
            .collect();
        // Node 0 (clearcoat) is the parent of node 1 (unlit): the child is traversed first.
        doc.nodes = vec![
            Node {
                mesh: Some(1),
                children: vec![1],
                ..Node::default()
            },
            Node {
                mesh: Some(0),
                ..Node::default()
            },
        ];
        doc.scenes = vec![Scene {
            nodes: vec![0],
            ..Scene::default()
        }];
        doc.extensions_used = vec!["KHR_materials_clearcoat".into(), UNLIT.into()];
        reorder(&mut doc);
        assert_eq!(doc.extensions_used, [UNLIT, "KHR_materials_clearcoat"]);
    }
}
