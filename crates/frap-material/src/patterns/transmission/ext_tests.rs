// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 3.6 acceptance tests of the `KHR_materials_*` patterns:
//! - `raise` builds the graph Blender 5.2.4's glTF importer builds for a material using every
//!   extension with every texture (`import_ext/import_ext.blender.json`, written by
//!   `import_ext/dump.py`): same nodes (name, type, label, location, parent, properties) in
//!   the same `tree.nodes` order, same links in the same `tree.links` order, same values;
//! - lowering that graph gives the glTF values back (the importer's known losses aside), and
//!   the round-trip law `lower(raise(lower(x))) == lower(x)` holds on it and on constant and
//!   node-setup fixtures of every extension;
//! - the exporter-facing rules of each pattern (thresholds, defaults, unsupported setups).

use std::collections::HashMap;

use frap_nodes::{NodeTree, SocketValue};
use frap_rna::{IdKey, IdRef, ImageId, Value};
use frap_scene::id::{Image, Material};
use serde_json::Value as J;

use crate::*;

#[derive(Default)]
struct Images(HashMap<IdRef<ImageId>, Image>);

impl ImageLookup for Images {
    fn image(&self, im: IdRef<ImageId>) -> Option<&Image> {
        self.0.get(&im)
    }
}

impl Images {
    fn add(&mut self, name: &str, non_color: bool) -> IdRef<ImageId> {
        let r = IdRef::from_key(IdKey::next());
        let mut im = Image::new(name, 1, 1);
        if non_color {
            im.colorspace_settings.name = "Non-Color".into();
        }
        self.0.insert(r, im);
        r
    }
}

fn dump() -> J {
    serde_json::from_str(include_str!("import_ext/import_ext.blender.json")).unwrap()
}

const LINEAR: Sampler = Sampler {
    filter: Filter::Linear,
    wrap_s: Wrap::Repeat,
    wrap_t: Wrap::Repeat,
};

/// The images `img0` … `img11` with the colour spaces Blender gave them, and the lowered
/// material frap's importer reads from `import_ext.gltf`.
fn imported() -> (Images, Vec<IdRef<ImageId>>, LoweredMaterial) {
    let d = dump();
    let mut images = Images::default();
    let mut ids = Vec::new();
    for i in 0..12 {
        let name = format!("img{i}");
        let space = d["images"]
            .as_object()
            .unwrap()
            .values()
            .find(|v| v[0] == name.as_str())
            .map(|v| v[1].as_str().unwrap().to_owned())
            .unwrap();
        ids.push(images.add(&name, space == "Non-Color"));
    }
    let t = |i: usize, channel| TexRef {
        image: ids[i],
        uv: UvSource::Index(0),
        channel,
        transform: None,
        sampler: LINEAR,
        colorspace_is_data: images.0[&ids[i]].colorspace_settings.name == "Non-Color",
    };
    let mut lm = LoweredMaterial::gltf_default();
    lm.ext.clearcoat = Some(Clearcoat {
        factor: Channel {
            factor: 0.5,
            texture: Some(t(0, ChannelSel::R)),
        },
        roughness: Channel {
            factor: 0.25,
            texture: Some(t(1, ChannelSel::G)),
        },
        normal: Some(NormalMap {
            texture: t(2, ChannelSel::Rgb),
            scale: 0.5,
        }),
    });
    lm.ext.transmission = Some(Transmission {
        factor: Channel {
            factor: 1.0,
            texture: Some(t(3, ChannelSel::R)),
        },
    });
    lm.ext.volume = Some(Volume {
        thickness: Channel {
            factor: 2.0,
            texture: Some(t(4, ChannelSel::G)),
        },
        attenuation_distance: 0.5,
        attenuation_color: [0.5, 0.25, 1.0],
    });
    lm.ext.dispersion = Some(0.125);
    lm.ext.specular = Some(Specular {
        factor: Channel {
            factor: 0.5,
            texture: Some(t(5, ChannelSel::A)),
        },
        color: Channel {
            factor: [0.5, 1.0, 0.25],
            texture: Some(t(6, ChannelSel::Rgb)),
        },
    });
    lm.ext.anisotropy = Some(Anisotropy {
        strength: 0.5,
        rotation: 1.25,
        texture: Some(t(7, ChannelSel::Rgb)),
    });
    lm.ext.sheen = Some(Sheen {
        color: Channel {
            factor: [1.0, 0.5, 0.25],
            texture: Some(t(8, ChannelSel::Rgb)),
        },
        roughness: Channel {
            factor: 0.75,
            texture: Some(t(9, ChannelSel::A)),
        },
    });
    // The add-on's importer reads the maximum from `iridescenceThickness`: 400.
    lm.ext.iridescence = Some(Iridescence {
        factor: Channel {
            factor: 0.5,
            texture: Some(t(10, ChannelSel::R)),
        },
        ior: 1.4,
        thickness_min: 150.0,
        thickness_max: Channel {
            factor: 400.0,
            texture: Some(t(11, ChannelSel::G)),
        },
    });
    lm.ext.ior = Some(1.25);
    (images, ids, lm)
}

fn raised(lm: &LoweredMaterial) -> Material {
    let mut m = Material::new("M");
    raise(lm, &mut m, &mut NameAlloc::default());
    m
}

fn round4(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

fn approx(a: &J, b: &J) -> bool {
    match (a, b) {
        (J::Number(x), J::Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-5,
        (J::Array(x), J::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| approx(p, q))
        }
        _ => a == b,
    }
}

fn socket_json(v: &SocketValue) -> J {
    match v {
        SocketValue::Float(f) => serde_json::json!(f),
        SocketValue::Color(c) => serde_json::json!(c),
        SocketValue::Vector(v) => serde_json::json!(&v[..3]),
        SocketValue::Bool(b) => serde_json::json!(b),
        _ => J::Null,
    }
}

#[test]
fn raise_matches_blenders_gltf_importer_for_every_extension() {
    let d = dump();
    let (_, ids, lm) = imported();
    let m = raised(&lm);
    let t = &m.node_tree;
    let parent =
        |n: &frap_nodes::Node| n.parent.and_then(|u| t.node(u)).map(|p| p.name.to_string());
    for (i, (n, want)) in t
        .nodes
        .iter()
        .zip(d["nodes"].as_array().unwrap())
        .enumerate()
    {
        let w = want.as_array().unwrap();
        let what = format!("node {i} {}", n.name);
        assert_eq!(n.name, w[0].as_str().unwrap(), "{what}");
        assert_eq!(n.bl_idname, w[1].as_str().unwrap(), "{what}");
        assert_eq!(n.label, w[2].as_str().unwrap(), "{what}");
        assert_eq!(
            [round4(n.location[0]), round4(n.location[1])],
            [w[3][0].as_f64().unwrap(), w[3][1].as_f64().unwrap()],
            "{what} location"
        );
        assert_eq!(parent(n).as_deref(), w[4].as_str(), "{what}");
        for (p, v) in w[5].as_object().unwrap() {
            match p.as_str() {
                "value" => assert!(
                    approx(&socket_json(&n.outputs[0].default_value), v),
                    "{what} value"
                ),
                // Bound to the datablock by the importer (`bind_settings_nodes`).
                "node_tree" => assert_eq!(n.group_tree(), None, "{what}"),
                _ => assert_eq!(n.prop_str(p), v.as_str().unwrap(), "{what} {p}"),
            }
        }
    }
    assert_eq!(t.nodes.len(), d["nodes"].as_array().unwrap().len());
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
    assert_eq!(J::Array(links), d["links"]);
    // Images on the right nodes.
    for (node, v) in d["images"].as_object().unwrap() {
        let i: usize = v[0].as_str().unwrap()[3..].parse().unwrap();
        let n = t.node_by_name(node).unwrap();
        assert_eq!(
            n.prop("image"),
            Some(&Value::Id(Some(ids[i].any()))),
            "{node}"
        );
    }
    // Unlinked socket values of the three fixed nodes.
    for (node, key) in [
        ("Principled BSDF", "bsdf"),
        ("Group", "group"),
        ("Volume Absorption", "volume_absorption"),
    ] {
        let n = t.node_by_name(node).unwrap();
        for (id, v) in d[key].as_object().unwrap() {
            let s = n
                .inputs
                .iter()
                .find(|s| s.identifier == id.as_str())
                .unwrap();
            assert!(
                approx(&socket_json(&s.default_value), v),
                "{node}.{id}: {:?} != {v}",
                s.default_value
            );
        }
    }
}

/// What lowering the imported graph gives: the glTF values, except where the add-on's
/// importer loses them (the thin-film IOR of a textured iridescence keeps Blender's 1.33) or
/// where the graph carries no UV map node (active render UV map).
#[test]
fn lowering_the_imported_graph_gives_the_gltf_values() {
    let (images, _, lm) = imported();
    let m = raised(&lm);
    let got = lower(&m.node_tree, &m, &images);
    assert!(got.diagnostics.is_empty(), "{:?}", got.diagnostics);
    let mut want = lm.ext.clone();
    fn active(t: &mut Option<TexRef>) {
        if let Some(t) = t {
            t.uv = UvSource::ActiveRender;
        }
    }
    let c = want.clearcoat.as_mut().unwrap();
    active(&mut c.factor.texture);
    active(&mut c.roughness.texture);
    if let Some(n) = c.normal.as_mut() {
        n.texture.uv = UvSource::ActiveRender;
    }
    active(&mut want.transmission.as_mut().unwrap().factor.texture);
    active(&mut want.volume.as_mut().unwrap().thickness.texture);
    let s = want.specular.as_mut().unwrap();
    active(&mut s.factor.texture);
    active(&mut s.color.texture);
    active(&mut want.anisotropy.as_mut().unwrap().texture);
    let s = want.sheen.as_mut().unwrap();
    active(&mut s.color.texture);
    active(&mut s.roughness.texture);
    let ir = want.iridescence.as_mut().unwrap();
    active(&mut ir.factor.texture);
    active(&mut ir.thickness_max.texture);
    ir.ior = 1.33;
    // The raised tint is a factor × texture: its found factor is written even when white
    // (WP 8.12).
    want.specular_color_set = true;
    assert_eq!(got.ext, want);
    // The law on the imported graph.
    let again = raised(&got);
    assert_eq!(lower(&again.node_tree, &again, &images), got);
}

// ------------------------------------------------------------------ hand-built graphs

fn mat() -> Material {
    let mut m = Material::new("M");
    m.node_tree = NodeTree::default_material();
    m
}

const BSDF: u32 = 0;

fn add(m: &mut Material, ty: &str) -> u32 {
    let i = m.node_tree.new_node(ty).unwrap();
    m.node_tree.nodes[i].uid
}

fn set(m: &mut Material, uid: u32, name: &str, v: SocketValue) {
    m.node_tree
        .node_mut(uid)
        .unwrap()
        .input_mut(name)
        .unwrap()
        .default_value = v;
}

fn link(m: &mut Material, from: (u32, &str), to: (u32, &str)) {
    m.node_tree.link_by_name(from, to).unwrap();
}

fn f(v: f32) -> SocketValue {
    SocketValue::Float(v)
}

fn rgb(r: f32, g: f32, b: f32) -> SocketValue {
    SocketValue::Color([r, g, b, 1.0])
}

/// A "glTF Material Output" group node (as the add-on's operator makes one).
fn settings(m: &mut Material) -> u32 {
    let g = frap_nodes::builtin_groups::gltf_material_output::new_group();
    let i = m.node_tree.add_group_node(
        IdRef::<frap_rna::NodeTreeId>::from_key(IdKey::next()).any(),
        &g.interface,
    );
    m.node_tree.nodes[i].uid
}

fn law(m: &Material, images: &Images) -> LoweredMaterial {
    let lm = lower(&m.node_tree, m, images);
    let again = raised(&lm);
    assert_eq!(lower(&again.node_tree, &again, images), lm, "law");
    lm
}

#[test]
fn default_material_has_only_empty_extensions() {
    let m = mat();
    let lm = law(&m, &Images::default());
    let e = &lm.ext;
    let cc = e.clearcoat.as_ref().unwrap();
    assert_eq!((cc.factor.factor, cc.roughness.factor), (0.0, 0.03));
    assert_eq!(e.transmission.as_ref().unwrap().factor.factor, 0.0);
    let s = e.specular.as_ref().unwrap();
    assert_eq!((s.factor.factor, s.color.factor), (1.0, [1.0; 3]));
    assert_eq!(e.ior, Some(1.5));
    let a = e.anisotropy.as_ref().unwrap();
    assert_eq!((a.strength, a.rotation), (0.0, 0.0));
    assert!(e.sheen.is_none() && e.volume.is_none() && e.iridescence.is_none());
    assert!(e.dispersion.is_none());
    // Raising the default material adds no node.
    assert_eq!(raised(&lm).node_tree.nodes.len(), 2);
}

#[test]
fn constant_extensions_lower_like_the_exporter() {
    let mut m = mat();
    for (s, v) in [
        ("Coat Weight", f(0.8)),
        ("Coat Roughness", f(0.2)),
        ("Transmission Weight", f(0.6)),
        ("IOR", f(1.45)),
        ("Specular IOR Level", f(0.75)),
        ("Specular Tint", rgb(0.5, 0.6, 0.7)),
        ("Sheen Weight", f(0.3)),
        ("Sheen Tint", rgb(0.2, 0.3, 0.4)),
        ("Sheen Roughness", f(0.25)),
        ("Anisotropic", f(0.5)),
        ("Anisotropic Rotation", f(0.25)),
    ] {
        set(&mut m, BSDF, s, v);
    }
    let lm = law(&m, &Images::default());
    let e = &lm.ext;
    let cc = e.clearcoat.as_ref().unwrap();
    assert_eq!((cc.factor.factor, cc.roughness.factor), (0.8, 0.2));
    assert_eq!(e.transmission.as_ref().unwrap().factor.factor, 0.6);
    assert_eq!(e.ior, Some(1.45));
    // 0.75 × 2 = 1.5 > 1: factor capped, the tint multiplied by 1.5.
    let s = e.specular.as_ref().unwrap();
    assert_eq!(s.factor.factor, 1.0);
    assert_eq!(
        s.color.factor,
        [0.5f32, 0.6, 0.7].map(|c| (f64::from(c) * 1.5) as f32)
    );
    let sh = e.sheen.as_ref().unwrap();
    assert_eq!(
        (sh.color.factor, sh.roughness.factor),
        ([0.2, 0.3, 0.4], 0.25)
    );
    let a = e.anisotropy.as_ref().unwrap();
    assert_eq!(a.strength, 0.5);
    assert_eq!(a.rotation, (0.25 * std::f64::consts::TAU) as f32);
}

#[test]
fn specular_factor_below_one_and_linked_quirk() {
    let mut images = Images::default();
    let img = images.add("spec", false);
    let mut m = mat();
    set(&mut m, BSDF, "Specular IOR Level", f(0.3));
    assert_eq!(
        law(&m, &images).ext.specular.unwrap().factor.factor,
        (0.3f64 as f32 as f64 * 2.0) as f32
    );
    // A linked level multiplied by exactly 1 is not doubled (the add-on's `fac != 1.0` test):
    // fac stays 1, so the tint is not scaled.
    let tex = add(&mut m, "ShaderNodeTexImage");
    m.node_tree
        .node_mut(tex)
        .unwrap()
        .set_prop("image", Value::Id(Some(img.any())));
    let mul = add(&mut m, "ShaderNodeMath");
    m.node_tree
        .node_mut(mul)
        .unwrap()
        .set_prop("operation", Value::Enum("MULTIPLY".into()));
    m.node_tree.node_mut(mul).unwrap().inputs[1].default_value = f(1.0);
    link(&mut m, (tex, "Alpha"), (mul, "Value"));
    link(&mut m, (mul, "Value"), (BSDF, "Specular IOR Level"));
    let s = law(&m, &images).ext.specular.unwrap();
    assert_eq!(s.factor.factor, 1.0);
    assert_eq!(s.factor.texture.unwrap().channel, ChannelSel::A);
    assert_eq!(s.color.factor, [1.0; 3]);
}

#[test]
fn sheen_needs_a_weight() {
    let mut m = mat();
    set(&mut m, BSDF, "Sheen Tint", rgb(1.0, 0.0, 0.0));
    assert!(law(&m, &Images::default()).ext.sheen.is_none());
    // A Value node is constant-folded by the inliner (verified on 5.2.4): 0 disables sheen,
    // anything else enables it.
    let v = add(&mut m, "ShaderNodeValue");
    link(&mut m, (v, "Value"), (BSDF, "Sheen Weight"));
    assert!(law(&m, &Images::default()).ext.sheen.is_none());
    m.node_tree.node_mut(v).unwrap().outputs[0].default_value = f(0.3);
    let s = law(&m, &Images::default()).ext.sheen.unwrap();
    assert_eq!(s.color.factor, [1.0, 0.0, 0.0]);
    // A weight the inliner cannot fold enables it whatever its value.
    let fr = add(&mut m, "ShaderNodeFresnel");
    link(&mut m, (fr, "Factor"), (BSDF, "Sheen Weight"));
    let s = law(&m, &Images::default()).ext.sheen.unwrap();
    assert_eq!(s.color.factor, [1.0, 0.0, 0.0]);
}

#[test]
fn volume_dispersion_and_iridescence_from_the_settings_group() {
    let mut m = mat();
    let g = settings(&mut m);
    // Thickness 0: no volume; the other group inputs still lower.
    set(&mut m, g, "Dispersion", f(0.3));
    set(&mut m, g, "Iridescence Factor", f(0.75));
    set(&mut m, BSDF, "Thin Film Thickness", f(550.0));
    let lm = lower(&m.node_tree, &m, &Images::default());
    assert!(lm.ext.volume.is_none());
    assert_eq!(lm.ext.dispersion, Some(0.3));
    let ir = lm.ext.iridescence.clone().unwrap();
    assert_eq!(
        (
            ir.factor.factor,
            ir.thickness_max.factor,
            ir.thickness_min,
            ir.ior
        ),
        (0.75, 550.0, 100.0, 1.33)
    );
    // With a thickness and a Volume Absorption node on the output.
    set(&mut m, g, "Thickness", f(0.25));
    let va = add(&mut m, "ShaderNodeVolumeAbsorption");
    set(&mut m, va, "Color", rgb(0.9, 0.5, 0.25));
    set(&mut m, va, "Density", f(4.0));
    let out = m
        .node_tree
        .first_of_type("ShaderNodeOutputMaterial")
        .unwrap()
        .uid;
    link(&mut m, (va, "Volume"), (out, "Volume"));
    let lm = law(&m, &Images::default());
    let v = lm.ext.volume.unwrap();
    assert_eq!(v.thickness.factor, 0.25);
    assert_eq!(v.attenuation_distance, 0.25);
    assert_eq!(v.attenuation_color, [0.9, 0.5, 0.25]);
    let none = Images::default();
    let cx = LowerCx::new(&m.node_tree, &m, &none);
    assert!(crate::patterns::volume::absorption_node(&cx).is_some());
    // The raised graph: settings node first (after the BSDF and the output), then the
    // Volume Absorption node linked into the output's Volume.
    let r = raised(&lower(&m.node_tree, &m, &Images::default()));
    let names: Vec<&str> = r.node_tree.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Principled BSDF",
            "Material Output",
            "Group",
            "Volume Absorption"
        ]
    );
    // No Volume Absorption: the attenuation keeps glTF's defaults.
    let mut m2 = mat();
    let g2 = settings(&mut m2);
    set(&mut m2, g2, "Thickness", f(2.0));
    let v = lower(&m2.node_tree, &m2, &Images::default())
        .ext
        .volume
        .unwrap();
    assert_eq!(
        (v.attenuation_distance, v.attenuation_color),
        (f32::INFINITY, [1.0; 3])
    );
}

#[test]
fn unsupported_setups_warn() {
    let mut images = Images::default();
    let img = images.add("grey", false);
    let mut m = mat();
    // Coat Weight from a Value node is constant-folded (0.7, verified on 5.2.4).
    let v = add(&mut m, "ShaderNodeValue");
    m.node_tree.node_mut(v).unwrap().outputs[0].default_value = f(0.7);
    link(&mut m, (v, "Value"), (BSDF, "Coat Weight"));
    let lm = lower(&m.node_tree, &m, &images);
    assert_eq!(lm.ext.clearcoat.unwrap().factor.factor, 0.7);
    assert!(lm.diagnostics.is_empty(), "{:?}", lm.diagnostics);
    // Coat Weight from a node the inliner cannot fold: not exported (only constants and
    // textures are), WARNING.
    let fr = add(&mut m, "ShaderNodeFresnel");
    link(&mut m, (fr, "Factor"), (BSDF, "Coat Weight"));
    let lm = lower(&m.node_tree, &m, &images);
    assert_eq!(lm.ext.clearcoat.unwrap().factor.factor, 0.0);
    assert!(
        lm.diagnostics
            .iter()
            .any(|d| d.message.contains("Coat Weight")),
        "{:?}",
        lm.diagnostics
    );
    // A grayscale texture on Anisotropic: baked into a new image (WP 8.12), no WARNING.
    let tex = add(&mut m, "ShaderNodeTexImage");
    m.node_tree
        .node_mut(tex)
        .unwrap()
        .set_prop("image", Value::Id(Some(img.any())));
    link(&mut m, (tex, "Color"), (BSDF, "Anisotropic"));
    let lm = lower(&m.node_tree, &m, &images);
    assert!(lm.ext.anisotropy.is_some());
    assert!(lm.ext.anisotropy_grayscale.is_some());
    assert!(
        !lm.diagnostics
            .iter()
            .any(|d| d.message.contains("Anisotropic"))
    );
}

#[test]
fn deep_texture_search_and_channels() {
    let mut images = Images::default();
    let img = images.add("mask", true);
    let mut m = mat();
    // Image → Separate Color (Green) → Math MULTIPLY 0.5 → Coat Roughness: packed from G.
    let tex = add(&mut m, "ShaderNodeTexImage");
    m.node_tree
        .node_mut(tex)
        .unwrap()
        .set_prop("image", Value::Id(Some(img.any())));
    let sep = add(&mut m, "ShaderNodeSeparateColor");
    let mul = add(&mut m, "ShaderNodeMath");
    m.node_tree
        .node_mut(mul)
        .unwrap()
        .set_prop("operation", Value::Enum("MULTIPLY".into()));
    m.node_tree.node_mut(mul).unwrap().inputs[1].default_value = f(0.5);
    link(&mut m, (tex, "Color"), (sep, "Color"));
    link(&mut m, (sep, "Green"), (mul, "Value"));
    link(&mut m, (mul, "Value"), (BSDF, "Coat Roughness"));
    // Image Alpha → Math POWER → Transmission Weight: found through any node, factor 1.
    let pow = add(&mut m, "ShaderNodeMath");
    m.node_tree
        .node_mut(pow)
        .unwrap()
        .set_prop("operation", Value::Enum("POWER".into()));
    link(&mut m, (tex, "Alpha"), (pow, "Value"));
    link(&mut m, (pow, "Value"), (BSDF, "Transmission Weight"));
    let lm = lower(&m.node_tree, &m, &images);
    let r = lm.ext.clearcoat.unwrap().roughness;
    assert_eq!(r.factor, 0.5);
    let t = r.texture.unwrap();
    assert_eq!(
        (t.channel, t.image, t.colorspace_is_data),
        (ChannelSel::G, img, true)
    );
    let tr = lm.ext.transmission.unwrap().factor;
    assert_eq!(tr.factor, 1.0);
    assert_eq!(tr.texture.unwrap().channel, ChannelSel::R, "copied whole");
}
