// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenes built in Rust for the exporter tests (WP 1.16): the factory startup, emptied or not,
//! plus objects, meshes, materials (node trees built from the generated node types) and images,
//! and the validation every exported file must pass (`frap_gltf::validate_file` and the
//! independent `gltf =1.4.1` reader).
#![allow(dead_code)]

use std::sync::Arc;

use frap_gltf::json::Document;
use frap_io_gltf::{ExportFormat, ExportOutput, ExportSettings, export};
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use frap_nodes::generated::{NodePropDefault, SocketDefault, node_type};
use frap_nodes::{Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketValue};
use frap_rna::{IdRef, ImageId, MaterialId, ObjectId, Value};
use frap_scene::id::{Collection, EncodedImage, Image, Material, Object, SHADER_NODETREE_NAME};
use frap_scene::{Base, BaseFlag, ChangeLog, Context, Main, Preferences, WindowState, startup};

/// A scene under construction.
pub struct Scene {
    pub main: Main,
    pub win: WindowState,
    pub prefs: Preferences,
    pub log: ChangeLog,
}

/// Stable test settings (`generator` does not carry the crate version).
pub fn settings() -> ExportSettings {
    ExportSettings {
        generator: "frap test".to_owned(),
        ..ExportSettings::default()
    }
}

impl Scene {
    /// The factory startup scene (Cube, Light, Camera in "Collection").
    pub fn factory() -> Scene {
        let (main, win) = startup::factory();
        Scene {
            main,
            win,
            prefs: Preferences::default(),
            log: ChangeLog::default(),
        }
    }

    /// The factory scene with every object unlinked (datablocks stay as orphans).
    pub fn empty() -> Scene {
        let mut s = Scene::factory();
        let coll = s.main.collections.find("Collection").unwrap();
        s.main
            .collections
            .get_mut(coll, &mut s.log)
            .unwrap()
            .objects
            .clear();
        let sc = s.win.scene;
        let scene = s.main.scenes.get_mut(sc, &mut s.log).unwrap();
        scene.view_layers[0].bases.clear();
        scene.view_layers[0].active = None;
        s
    }

    pub fn ctx(&self) -> Context<'_> {
        Context {
            main: &self.main,
            win: &self.win,
            prefs: &self.prefs,
        }
    }

    /// Links an object into "Collection" and gives it a base (selected or not).
    pub fn link(&mut self, ob: IdRef<ObjectId>, selected: bool) {
        let coll = self.main.collections.find("Collection").unwrap();
        self.main
            .collections
            .get_mut(coll, &mut self.log)
            .unwrap()
            .objects
            .push(ob);
        let sc = self.win.scene;
        self.main
            .scenes
            .get_mut(sc, &mut self.log)
            .unwrap()
            .view_layers[0]
            .bases
            .push(Base {
                object: ob,
                flag: if selected {
                    BaseFlag::SELECTED
                } else {
                    BaseFlag::empty()
                },
            });
    }

    /// A mesh object linked into the scene.
    pub fn mesh_object(&mut self, name: &str, mesh: Mesh) -> IdRef<ObjectId> {
        let me = self.main.meshes.insert(name, mesh, &mut self.log);
        let ob = self
            .main
            .objects
            .insert(name, Object::new("", Some(me.any())), &mut self.log);
        self.link(ob, false);
        ob
    }

    /// An object sharing another object's data.
    pub fn linked_duplicate(&mut self, name: &str, of: IdRef<ObjectId>) -> IdRef<ObjectId> {
        let data = self.main.objects.get(of).unwrap().data;
        let ob = self
            .main
            .objects
            .insert(name, Object::new("", data), &mut self.log);
        self.link(ob, false);
        ob
    }

    /// An empty linked into the scene.
    pub fn empty_object(&mut self, name: &str) -> IdRef<ObjectId> {
        let ob = self
            .main
            .objects
            .insert(name, Object::new("", None), &mut self.log);
        self.link(ob, false);
        ob
    }

    pub fn ob(&mut self, r: IdRef<ObjectId>) -> &mut Object {
        self.main.objects.get_mut(r, &mut self.log).unwrap()
    }

    pub fn mesh_of(&mut self, r: IdRef<ObjectId>) -> &mut Mesh {
        let me = self
            .main
            .objects
            .get(r)
            .unwrap()
            .data
            .unwrap()
            .typed()
            .unwrap();
        self.main.meshes.get_mut(me, &mut self.log).unwrap()
    }

    /// A material with a Principled BSDF → Material Output graph.
    pub fn material(
        &mut self,
        name: &str,
        base: [f32; 4],
        metallic: f32,
        roughness: f32,
    ) -> IdRef<MaterialId> {
        let mut m = Material::new("");
        m.node_tree = principled_tree(base, metallic, roughness);
        self.main.materials.insert(name, m, &mut self.log)
    }

    /// Adds an Image Texture node feeding Base Color (and Alpha when `alpha`).
    pub fn base_color_texture(
        &mut self,
        mat: IdRef<MaterialId>,
        image: IdRef<ImageId>,
        alpha: bool,
    ) {
        let m = self.main.materials.get_mut(mat, &mut self.log).unwrap();
        let tree = &mut m.node_tree;
        let uid = tree.next_node_uid;
        tree.next_node_uid += 1;
        let mut tex = node(uid, "ShaderNodeTexImage");
        tex.props
            .values
            .insert("image", Value::Id(Some(image.any())));
        tree.nodes.push(tex);
        let bsdf = tree
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap()
            .uid;
        tree.links.push(Link {
            from_node: uid,
            from_socket: "Color".into(),
            to_node: bsdf,
            to_socket: "Base Color".into(),
        });
        if alpha {
            tree.links.push(Link {
                from_node: uid,
                from_socket: "Alpha".into(),
                to_node: bsdf,
                to_socket: "Alpha".into(),
            });
        }
    }

    /// Links the material's last Image Texture `Color` into the Principled `Emission Color`
    /// and sets `Emission Strength`.
    pub fn link_emission_from_texture(&mut self, mat: IdRef<MaterialId>, strength: f32) {
        let m = self.main.materials.get_mut(mat, &mut self.log).unwrap();
        let tree = &mut m.node_tree;
        let tex = tree
            .nodes
            .iter()
            .rev()
            .find(|n| n.bl_idname == "ShaderNodeTexImage")
            .unwrap()
            .uid;
        let bsdf = tree
            .nodes
            .iter_mut()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap();
        for s in &mut bsdf.inputs {
            if s.name == "Emission Strength" {
                s.default_value = SocketValue::Float(strength);
            }
        }
        let bsdf = bsdf.uid;
        tree.links.push(Link {
            from_node: tex,
            from_socket: "Color".into(),
            to_node: bsdf,
            to_socket: "Emission Color".into(),
        });
    }

    /// A packed image holding `bytes`.
    pub fn packed_image(&mut self, name: &str, bytes: Vec<u8>, mime: &str) -> IdRef<ImageId> {
        let mut im = Image::new("", 2, 2);
        im.source = "FILE".into();
        im.packed_file = Some(Arc::new(EncodedImage {
            bytes,
            mime: mime.into(),
        }));
        self.main.images.insert(name, im, &mut self.log)
    }

    /// A generated BLANK image.
    pub fn generated_image(&mut self, name: &str, color: [f32; 4]) -> IdRef<ImageId> {
        let mut im = Image::new("", 4, 2);
        im.generated_color = color;
        self.main.images.insert(name, im, &mut self.log)
    }

    /// A new collection linked under the scene's master collection.
    pub fn collection(&mut self, name: &str) -> frap_rna::IdRef<frap_rna::CollectionId> {
        let c = self
            .main
            .collections
            .insert(name, Collection::new(""), &mut self.log);
        let sc = self.win.scene;
        self.main
            .scenes
            .get_mut(sc, &mut self.log)
            .unwrap()
            .collection
            .children
            .push(c);
        c
    }

    pub fn export(&self, s: &ExportSettings) -> ExportOutput {
        let out = export(&self.main, &self.ctx(), s).expect("export");
        validate(&out, s.export_format);
        // Debug hook: `FRAP_EXPORT_DUMP_DIR=<dir>` keeps every GLB (e.g. to import in Blender).
        if let Ok(dir) = std::env::var("FRAP_EXPORT_DUMP_DIR")
            && s.export_format == ExportFormat::Glb
        {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static N: AtomicUsize = AtomicUsize::new(0);
            let name = std::thread::current()
                .name()
                .unwrap_or("export")
                .replace("::", "_");
            let n = N.fetch_add(1, Ordering::SeqCst);
            let _ = std::fs::write(format!("{dir}/{name}_{n}.glb"), &out.bytes);
        }
        out
    }
}

/// A fresh node of type `bl_idname` with the generated sockets and properties.
pub fn node(uid: u32, bl_idname: &str) -> Node {
    let def = node_type(bl_idname).unwrap_or_else(|| panic!("node type {bl_idname}"));
    let sock = |d: &frap_nodes::generated::SocketDef| Socket {
        name: d.name.into(),
        identifier: d.identifier.into(),
        ty: d.ty,
        enabled: d.enabled,
        hide_value: d.hide_value,
        default_value: match d.default {
            SocketDefault::None => SocketValue::None,
            SocketDefault::Float(f) => SocketValue::Float(f),
            SocketDefault::Int(i) => SocketValue::Int(i as i32),
            SocketDefault::Bool(b) => SocketValue::Bool(b),
            SocketDefault::Vector(v) => {
                let mut a = [0.0; 4];
                a[..v.len()].copy_from_slice(v);
                SocketValue::Vector(a)
            }
            SocketDefault::Color(c) => SocketValue::Color(c),
            SocketDefault::Str(s) => SocketValue::String(s.into()),
            SocketDefault::Menu(s) => SocketValue::Menu(s.into()),
        },
    };
    let mut props = PropBag::default();
    for (k, v) in def.props {
        props.values.insert(
            k,
            match v {
                NodePropDefault::None => Value::Id(None),
                NodePropDefault::Bool(b) => Value::Bool(*b),
                NodePropDefault::Int(i) => Value::Int(*i),
                NodePropDefault::Float(f) => Value::Float(*f),
                NodePropDefault::Str(s) => Value::Enum((*s).into()),
                _ => Value::None,
            },
        );
    }
    Node {
        uid,
        name: def.name.into(),
        label: String::new(),
        bl_idname: def.bl_idname,
        location: [0.0, 0.0],
        width: def.width,
        parent: None,
        mute: false,
        hide: false,
        inputs: def.inputs.iter().map(sock).collect(),
        outputs: def.outputs.iter().map(sock).collect(),
        props,
    }
}

/// `materials.new()`'s graph with the given Principled inputs.
pub fn principled_tree(base: [f32; 4], metallic: f32, roughness: f32) -> NodeTree {
    let mut tree = NodeTree::new(SHADER_NODETREE_NAME, NodeTreeType::Shader, true);
    let out = node(0, "ShaderNodeOutputMaterial");
    let mut bsdf = node(1, "ShaderNodeBsdfPrincipled");
    for s in &mut bsdf.inputs {
        match s.name.as_str() {
            "Base Color" => s.default_value = SocketValue::Color(base),
            "Metallic" => s.default_value = SocketValue::Float(metallic),
            "Roughness" => s.default_value = SocketValue::Float(roughness),
            _ => {}
        }
    }
    tree.nodes = vec![out, bsdf];
    tree.links = vec![Link {
        from_node: 1,
        from_socket: "BSDF".into(),
        to_node: 0,
        to_socket: "Surface".into(),
    }];
    tree.next_node_uid = 2;
    tree
}

/// Sets a face-domain `material_index` attribute.
pub fn set_material_indices(mesh: &mut Mesh, indices: Vec<i32>) {
    mesh.attributes_mut().insert(Attribute {
        name: "material_index".into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(indices)),
    });
}

/// A tiny PNG (`w × h` RGBA, a gradient).
pub fn png(w: u32, h: u32) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h)
        .flat_map(|i| [(i * 40) as u8, 128, 255 - (i * 30) as u8, 255])
        .collect();
    frap_imbuf::encode(
        &frap_imbuf::DecodedImage {
            width: w,
            height: h,
            channels: 4,
            pixels: frap_imbuf::PixelData::U8(px),
        },
        &frap_imbuf::EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        },
    )
    .unwrap()
}

/// Every exported file passes `frap_gltf::validate_file` (no errors) and the `gltf =1.4.1`
/// validating reader.
pub fn validate(out: &ExportOutput, format: ExportFormat) {
    let report = frap_gltf::validate_file(&out.bytes).expect("readable glTF");
    assert!(report.is_ok(), "validation errors:\n{report}");
    // `gltf =1.4.1` requires `scene.nodes`, which the spec (and Blender, for a scene with
    // nothing exported) leaves out when empty: such files are checked by frap-gltf only.
    if out.document.scenes.iter().any(|s| s.nodes.is_empty()) {
        return;
    }
    match format {
        ExportFormat::Glb | ExportFormat::GltfEmbedded => {
            let g = gltf::Gltf::from_slice(&out.bytes).expect("gltf 1.4.1 validation");
            // Every accessor decodes through the independent reader.
            let blob = g.blob.clone();
            for mesh in g.document.meshes() {
                for prim in mesh.primitives() {
                    let r = prim.reader(|b| match b.source() {
                        gltf::buffer::Source::Bin => blob.as_deref(),
                        gltf::buffer::Source::Uri(u) => {
                            let _ = u;
                            None
                        }
                    });
                    if blob.is_some() {
                        let n = r.read_positions().expect("POSITION").count();
                        let idx: Vec<u32> = r.read_indices().unwrap().into_u32().collect();
                        assert!(idx.iter().all(|i| (*i as usize) < n));
                    }
                }
            }
        }
        ExportFormat::GltfSeparate => {
            gltf::Gltf::from_slice(&out.bytes).expect("gltf 1.4.1 validation");
            let doc = Document::from_slice(&out.bytes).unwrap();
            let bufs = frap_gltf::load_buffers(&doc, None, |uri| {
                out.files
                    .iter()
                    .find(|(p, _)| frap_gltf::accessor::uri_to_path(uri) == *p)
                    .map(|(_, b)| b.clone())
                    .ok_or_else(|| format!("missing {uri}"))
            })
            .expect("buffers");
            let r = frap_gltf::validate_data(&doc, &bufs);
            assert!(r.is_ok(), "data validation errors:\n{r}");
        }
    }
}

/// The document as stable pretty JSON for snapshots: non-integral numbers rounded to 6
/// significant digits (the last float32 bit of a decomposed matrix may differ across CPUs).
pub fn snapshot_json(doc: &Document) -> String {
    fn round(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let f = n.as_f64().unwrap();
                let r: f64 = format!("{f:.6e}").parse().unwrap();
                *v = serde_json::json!(r);
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(round),
            serde_json::Value::Object(o) => o.values_mut().for_each(round),
            _ => {}
        }
    }
    let text = doc.to_vec(&frap_gltf::WriteOpts::default());
    let mut v: serde_json::Value = serde_json::from_slice(&text).unwrap();
    round(&mut v);
    serde_json::to_string_pretty(&v).unwrap()
}
