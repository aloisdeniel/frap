// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.11 acceptance tests of `frap-material`: the M1 patterns (`base_color`,
//! `metallic_roughness`, constant `alpha`, constant `emission`, Image Texture on base colour),
//! the round-trip law `lower(raise(lower(x))) == lower(x)` on the M1 fixture materials, and
//! the extension-point rules (one `MATERIAL_PATTERNS` entry per `patterns/*.rs`; `lower` and
//! `raise` name no pattern).

use std::collections::HashMap;

use frap_nodes::{NodeTree, SocketAddr, SocketValue};
use frap_rna::{IdKey, IdRef, ImageId, ReportType, Value};
use frap_scene::id::{Image, Material};
use smol_str::SmolStr;

use crate::*;

// --------------------------------------------------------------------------------- helpers

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
        let mut im = Image::new(name, 4, 4);
        if non_color {
            im.colorspace_settings.name = "Non-Color".into();
        }
        self.0.insert(r, im);
        r
    }
}

/// `bpy.data.materials.new(name)`.
fn mat(name: &str) -> Material {
    let mut m = Material::new(name);
    m.node_tree = NodeTree::default_material();
    m
}

const BSDF: u32 = 0;

fn add(m: &mut Material, ty: &str) -> u32 {
    let i = m.node_tree.new_node(ty).unwrap();
    m.node_tree.nodes[i].uid
}

fn set_in(m: &mut Material, uid: u32, name: &str, v: SocketValue) {
    m.node_tree
        .node_mut(uid)
        .unwrap()
        .input_mut(name)
        .unwrap()
        .default_value = v;
}

fn set_in_at(m: &mut Material, uid: u32, i: usize, v: SocketValue) {
    m.node_tree.node_mut(uid).unwrap().inputs[i].default_value = v;
}

fn set_enum(m: &mut Material, uid: u32, prop: &str, v: &str) {
    assert!(
        m.node_tree
            .node_mut(uid)
            .unwrap()
            .set_prop(prop, Value::Enum(v.into()))
    );
}

fn link(m: &mut Material, from: (u32, &str), to: (u32, &str)) {
    m.node_tree.link_by_name(from, to).unwrap();
}

fn link_at(m: &mut Material, from: (u32, usize), to: (u32, usize)) {
    m.node_tree
        .link_new(
            SocketAddr::output(from.0, from.1),
            SocketAddr::input(to.0, to.1),
            true,
        )
        .unwrap();
}

fn image_node(m: &mut Material, image: IdRef<ImageId>) -> u32 {
    let t = add(m, "ShaderNodeTexImage");
    assert!(
        m.node_tree
            .node_mut(t)
            .unwrap()
            .set_prop("image", Value::Id(Some(image.any())))
    );
    t
}

fn color(r: f32, g: f32, b: f32) -> SocketValue {
    SocketValue::Color([r, g, b, 1.0])
}

fn float(f: f32) -> SocketValue {
    SocketValue::Float(f)
}

fn lower_m(m: &Material, images: &Images) -> LoweredMaterial {
    lower(&m.node_tree, m, images)
}

fn raise_lower(lm: &LoweredMaterial, images: &Images) -> (Material, LoweredMaterial) {
    let mut m = Material::new("raised");
    let mut names = NameAlloc::default();
    raise(lm, &mut m, &mut names);
    let names_in_tree: std::collections::BTreeSet<SmolStr> =
        m.node_tree.nodes.iter().map(|n| n.name.clone()).collect();
    assert_eq!(
        names.taken, names_in_tree,
        "NameAlloc holds every node name"
    );
    let again = lower_m(&m, images);
    (m, again)
}

/// The M1 fixture materials: every M1 pattern, in the node setups Blender users and the
/// glTF importer build.
fn fixtures(images: &mut Images) -> Vec<(&'static str, Material)> {
    let mut v = Vec::new();
    v.push(("default", mat("Material")));

    let mut m = mat("Steel");
    set_in(&mut m, BSDF, "Metallic", float(1.0));
    set_in(&mut m, BSDF, "Base Color", color(0.6, 0.6, 0.7));
    v.push(("steel", m));

    let mut m = mat("Red");
    set_in(&mut m, BSDF, "Base Color", color(1.0, 0.0, 0.0));
    set_in(&mut m, BSDF, "Roughness", float(0.2));
    set_in(&mut m, BSDF, "Metallic", float(0.3));
    v.push(("red", m));

    let mut m = mat("Overbright");
    set_in(&mut m, BSDF, "Base Color", color(2.0, -1.0, 0.5));
    v.push(("overbright", m));

    let mut m = mat("Glass");
    set_in(&mut m, BSDF, "Alpha", float(0.5));
    v.push(("blend", m));

    let mut m = mat("Cutout");
    set_in(&mut m, BSDF, "Alpha", float(0.0));
    v.push(("mask_zero", m));

    let mut m = mat("Lamp");
    set_in(&mut m, BSDF, "Emission Color", color(1.0, 0.5, 0.2));
    set_in(&mut m, BSDF, "Emission Strength", float(5.0));
    v.push(("emission_strong", m));

    let mut m = mat("Glow");
    set_in(&mut m, BSDF, "Emission Color", color(0.3, 0.3, 0.3));
    set_in(&mut m, BSDF, "Emission Strength", float(1.0));
    v.push(("emission_grey", m));

    let mut m = mat("Ember");
    set_in(&mut m, BSDF, "Emission Color", color(0.7, 0.3, 0.1));
    set_in(&mut m, BSDF, "Emission Strength", float(2.0));
    v.push(("emission_odd", m));

    let mut m = mat("Culled");
    m.use_backface_culling = true;
    v.push(("culled", m));

    let mut m = mat("Nodes");
    let rgb = add(&mut m, "ShaderNodeRGB");
    m.node_tree.node_mut(rgb).unwrap().outputs[0].default_value =
        SocketValue::Color([0.1, 0.2, 0.3, 1.0]);
    let val = add(&mut m, "ShaderNodeValue");
    m.node_tree.node_mut(val).unwrap().outputs[0].default_value = float(0.25);
    link(&mut m, (rgb, "Color"), (BSDF, "Base Color"));
    link(&mut m, (val, "Value"), (BSDF, "Roughness"));
    v.push(("rgb_value_nodes", m));

    let albedo = images.add("albedo.png", false);
    let mut m = mat("Textured");
    let t = image_node(&mut m, albedo);
    set_enum(&mut m, t, "interpolation", "Closest");
    set_enum(&mut m, t, "extension", "EXTEND");
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    v.push(("texture", m));

    let mut m = mat("Rerouted");
    let t = image_node(&mut m, albedo);
    let r = add(&mut m, "NodeReroute");
    link_at(&mut m, (t, 0), (r, 0));
    link_at(&mut m, (r, 0), (BSDF, 0));
    v.push(("texture_reroute", m));

    let mut m = mat("Tinted");
    let t = image_node(&mut m, albedo);
    let mix = add(&mut m, "ShaderNodeMix");
    set_enum(&mut m, mix, "data_type", "RGBA");
    set_enum(&mut m, mix, "blend_type", "MULTIPLY");
    set_in(&mut m, mix, "Factor", float(1.0));
    set_in(&mut m, mix, "B", color(0.5, 0.6, 0.7));
    link(&mut m, (t, "Color"), (mix, "A"));
    link(&mut m, (mix, "Result"), (BSDF, "Base Color"));
    let mul = add(&mut m, "ShaderNodeMath");
    set_enum(&mut m, mul, "operation", "MULTIPLY");
    set_in_at(&mut m, mul, 1, float(0.8));
    link_at(&mut m, (t, 1), (mul, 0));
    link_at(&mut m, (mul, 0), (BSDF, 4));
    v.push(("texture_factor_alpha", m));

    let mut m = mat("Mapped");
    let t = image_node(&mut m, albedo);
    let uv = add(&mut m, "ShaderNodeUVMap");
    assert!(
        m.node_tree
            .node_mut(uv)
            .unwrap()
            .set_prop("uv_map", Value::Str("UVMap.001".into()))
    );
    let map = add(&mut m, "ShaderNodeMapping");
    set_in(
        &mut m,
        map,
        "Location",
        SocketValue::Vector([0.1, 0.2, 0.0, 0.0]),
    );
    set_in(
        &mut m,
        map,
        "Scale",
        SocketValue::Vector([2.0, 2.0, 1.0, 0.0]),
    );
    link(&mut m, (uv, "UV"), (map, "Vector"));
    link(&mut m, (map, "Vector"), (t, "Vector"));
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    v.push(("texture_uv_mapping", m));

    let mut m = mat("Clipped");
    let t = image_node(&mut m, albedo);
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    let lt = add(&mut m, "ShaderNodeMath");
    set_enum(&mut m, lt, "operation", "LESS_THAN");
    set_in_at(&mut m, lt, 1, float(0.3));
    let sub = add(&mut m, "ShaderNodeMath");
    set_enum(&mut m, sub, "operation", "SUBTRACT");
    set_in_at(&mut m, sub, 0, float(1.0));
    link_at(&mut m, (t, 1), (lt, 0));
    link_at(&mut m, (lt, 0), (sub, 1));
    link_at(&mut m, (sub, 0), (BSDF, 4));
    v.push(("texture_alpha_clip", m));

    let mut m = mat("Rounded");
    let t = image_node(&mut m, albedo);
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    let round = add(&mut m, "ShaderNodeMath");
    set_enum(&mut m, round, "operation", "ROUND");
    link_at(&mut m, (t, 1), (round, 0));
    link_at(&mut m, (round, 0), (BSDF, 4));
    v.push(("texture_alpha_round", m));

    let orm = images.add("orm.png", true);
    let mut m = mat("Rough");
    let t = image_node(&mut m, orm);
    let sep = add(&mut m, "ShaderNodeSeparateColor");
    link(&mut m, (t, "Color"), (sep, "Color"));
    link(&mut m, (sep, "Blue"), (BSDF, "Metallic"));
    let mul = add(&mut m, "ShaderNodeMath");
    set_enum(&mut m, mul, "operation", "MULTIPLY");
    set_in_at(&mut m, mul, 1, float(0.5));
    link(&mut m, (sep, "Green"), (mul, "Value"));
    link_at(&mut m, (mul, 0), (BSDF, 2));
    v.push(("metallic_roughness_texture", m));

    let mut m = mat("Direct");
    let t = image_node(&mut m, orm);
    link(&mut m, (t, "Color"), (BSDF, "Roughness"));
    v.push(("roughness_texture_direct", m));

    let glow = images.add("glow.png", false);
    let mut m = mat("Screen");
    let t = image_node(&mut m, glow);
    link(&mut m, (t, "Color"), (BSDF, "Emission Color"));
    set_in(&mut m, BSDF, "Emission Strength", float(3.0));
    v.push(("emission_texture", m));
    v
}

// ------------------------------------------------------------------------------- patterns

#[test]
fn default_material_lowers_like_the_exporter() {
    let images = Images::default();
    let lm = lower_m(&mat("Material"), &images);
    assert_eq!(lm.base_color.factor, [0.8, 0.8, 0.8, 1.0]);
    assert_eq!(lm.base_color.texture, None);
    assert_eq!((lm.metallic.factor, lm.roughness.factor), (0.0, 0.5));
    assert_eq!(lm.emissive.factor, [0.0; 3]);
    assert_eq!(lm.emissive_strength, 1.0);
    assert_eq!(lm.alpha_mode, AlphaMode::Opaque);
    assert!(lm.double_sided);
    assert!(!lm.unlit);
    assert!(lm.diagnostics.is_empty());
}

#[test]
fn constant_patterns() {
    let mut images = Images::default();
    let f: HashMap<_, _> = fixtures(&mut images).into_iter().collect();
    let l = |k: &str| lower_m(&f[k], &images);

    let steel = l("steel");
    assert_eq!(steel.base_color.factor, [0.6, 0.6, 0.7, 1.0]);
    assert_eq!(steel.metallic.factor, 1.0);
    let red = l("red");
    assert_eq!((red.metallic.factor, red.roughness.factor), (0.3, 0.2));
    // The exporter clamps the base colour factor to [0, 1].
    assert_eq!(l("overbright").base_color.factor, [1.0, 0.0, 0.5, 1.0]);
    // Alpha mode from the graph.
    let blend = l("blend");
    assert_eq!(
        (blend.alpha_mode, blend.base_color.factor[3]),
        (AlphaMode::Blend, 0.5)
    );
    let cut = l("mask_zero");
    assert_eq!(
        (cut.alpha_mode, cut.base_color.factor[3]),
        (AlphaMode::Mask { cutoff: 0.5 }, 0.0)
    );
    // Emission: colour (1, 0.5, 0.2) at strength 5 → emissiveFactor [1, 0.5, 0.2] and
    // emissiveStrength 5 (verified exporter output, research §4.5).
    let e = l("emission_strong");
    assert_eq!(
        (e.emissive.factor, e.emissive_strength),
        ([1.0, 0.5, 0.2], 5.0)
    );
    let g = l("emission_grey");
    assert_eq!(
        (g.emissive.factor, g.emissive_strength),
        ([0.3, 0.3, 0.3], 1.0)
    );
    let o = l("emission_odd");
    assert_eq!(o.emissive_strength, 1.4);
    assert_eq!(o.emissive.factor[0], 1.0);
    assert!(!l("culled").double_sided);
    let n = l("rgb_value_nodes");
    assert_eq!(n.base_color.factor, [0.1, 0.2, 0.3, 1.0]);
    assert_eq!(n.roughness.factor, 0.25);
}

#[test]
fn image_texture_on_base_color() {
    let mut images = Images::default();
    let f: HashMap<_, _> = fixtures(&mut images).into_iter().collect();
    let l = |k: &str| lower_m(&f[k], &images);

    let t = l("texture");
    let tex = t.base_color.texture.clone().expect("base colour texture");
    assert_eq!(t.base_color.factor, [1.0; 4]);
    assert_eq!(tex.channel, ChannelSel::Rgb);
    assert_eq!(tex.uv, UvSource::ActiveRender);
    assert_eq!(tex.transform, None);
    assert_eq!(
        tex.sampler,
        Sampler {
            filter: Filter::Closest,
            wrap_s: Wrap::Clamp,
            wrap_t: Wrap::Clamp
        }
    );
    assert!(!tex.colorspace_is_data);
    assert_eq!(t.alpha_mode, AlphaMode::Opaque);
    assert!(t.diagnostics.is_empty());

    assert_eq!(
        l("texture_reroute").base_color.texture.map(|t| t.image),
        Some(tex.image)
    );

    let tf = l("texture_factor_alpha");
    assert_eq!(tf.base_color.factor, [0.5, 0.6, 0.7, 0.8]);
    assert_eq!(tf.alpha_mode, AlphaMode::Blend);
    assert_eq!(
        tf.base_color.texture.as_ref().unwrap().channel,
        ChannelSel::Rgba
    );

    // Mapping Location (0.1, 0.2), Scale 2 → offset [0.1, -1.2], scale [2, 2] (verified
    // exporter output, research §4.5).
    let mp = l("texture_uv_mapping");
    let mt = mp.base_color.texture.unwrap();
    assert_eq!(mt.uv, UvSource::Named("UVMap.001".into()));
    let tt = mt.transform.unwrap();
    assert_eq!(tt.rotation, 0.0);
    assert_eq!(tt.scale, [2.0, 2.0]);
    assert!((tt.offset[0] - 0.1).abs() < 1e-6 && (tt.offset[1] + 1.2).abs() < 1e-6);

    let clip = l("texture_alpha_clip");
    assert_eq!(clip.alpha_mode, AlphaMode::Mask { cutoff: 0.3 });
    assert_eq!(clip.base_color.texture.unwrap().channel, ChannelSel::Rgba);
    assert_eq!(
        l("texture_alpha_round").alpha_mode,
        AlphaMode::Mask { cutoff: 0.5 }
    );

    let mr = l("metallic_roughness_texture");
    assert_eq!(mr.metallic.factor, 1.0);
    assert_eq!(mr.roughness.factor, 0.5);
    assert_eq!(mr.metallic.texture.as_ref().unwrap().channel, ChannelSel::B);
    assert_eq!(
        mr.roughness.texture.as_ref().unwrap().channel,
        ChannelSel::G
    );
    assert!(mr.roughness.texture.as_ref().unwrap().colorspace_is_data);
    // Plugged in directly: glTF convention (roughness = G).
    let d = l("roughness_texture_direct");
    assert_eq!(d.roughness.texture.unwrap().channel, ChannelSel::G);
    assert_eq!(d.metallic.factor, 0.0);

    let et = l("emission_texture");
    assert_eq!(et.emissive.texture.unwrap().channel, ChannelSel::Rgb);
    assert_eq!((et.emissive.factor, et.emissive_strength), ([1.0; 3], 3.0));
}

// ------------------------------------------------------------------------ round-trip law

#[test]
fn round_trip_law_on_m1_fixtures() {
    let mut images = Images::default();
    for (name, m) in fixtures(&mut images) {
        let x = lower_m(&m, &images);
        assert!(x.diagnostics.is_empty(), "{name}: {:?}", x.diagnostics);
        let (raised, again) = raise_lower(&x, &images);
        assert_eq!(
            again, x,
            "{name}: lower(raise(lower(x))) != lower(x)\n{:#?}",
            raised.node_tree.nodes
        );
    }
}

#[test]
fn unrecognised_setups_fall_back_with_a_warning() {
    let mut images = Images::default();
    let albedo = images.add("a.png", false);
    // A procedural texture on Base Color.
    let mut m = mat("Noisy");
    let noise = add(&mut m, "ShaderNodeTexNoise");
    link(&mut m, (noise, "Color"), (BSDF, "Base Color"));
    let x = lower_m(&m, &images);
    assert_eq!(x.base_color.factor, [1.0; 4]);
    assert_eq!(x.base_color.texture, None);
    assert_eq!(x.diagnostics.len(), 1);
    assert_eq!(x.diagnostics[0].level, ReportType::Warning);
    assert!(
        x.diagnostics[0].message.contains("Base Color"),
        "{}",
        x.diagnostics[0].message
    );
    // Law modulo diagnostics.
    let (_, again) = raise_lower(&x, &images);
    assert_eq!(
        LoweredMaterial {
            diagnostics: vec![],
            ..x.clone()
        },
        again
    );

    // A surface that is not a Principled BSDF: the glTF default material.
    let mut m = mat("Unlit");
    let em = add(&mut m, "ShaderNodeEmission");
    let out = m
        .node_tree
        .first_of_type("ShaderNodeOutputMaterial")
        .unwrap()
        .uid;
    link(&mut m, (em, "Emission"), (out, "Surface"));
    let x = lower_m(&m, &images);
    assert_eq!(x.base_color.factor, [1.0; 4]);
    assert_eq!((x.metallic.factor, x.roughness.factor), (1.0, 1.0));
    assert_eq!(x.diagnostics.len(), 1);
    assert!(x.diagnostics[0].message.contains("not a Principled BSDF"));

    // An empty tree.
    let x = lower_m(&Material::new("Empty"), &images);
    assert!(x.diagnostics[0].message.contains("no shader is linked"));

    // Texture coordinates from an unsupported node.
    let mut m = mat("Coords");
    let t = image_node(&mut m, albedo);
    let geo = add(&mut m, "ShaderNodeNewGeometry");
    link(&mut m, (geo, "Position"), (t, "Vector"));
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    let x = lower_m(&m, &images);
    assert_eq!(
        x.base_color.texture.as_ref().unwrap().uv,
        UvSource::ActiveRender
    );
    assert_eq!(x.diagnostics.len(), 1);

    // An Image Texture without an image is not exported.
    let mut m = mat("NoImage");
    let t = add(&mut m, "ShaderNodeTexImage");
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    let x = lower_m(&m, &images);
    assert_eq!(x.base_color.texture, None);
    assert_eq!(x.diagnostics.len(), 1);
}

#[test]
fn principled_is_found_like_the_exporter() {
    let images = Images::default();
    let out = 1;
    // Principled BSDF → Mix Shader → Material Output: the exporter still reads the BSDF.
    let mut m = mat("Mixed");
    set_in(&mut m, BSDF, "Base Color", color(0.1, 0.2, 0.3));
    let mix = add(&mut m, "ShaderNodeMixShader");
    let tr = add(&mut m, "ShaderNodeBsdfTransparent");
    link(&mut m, (BSDF, "BSDF"), (mix, "Shader"));
    link_at(&mut m, (tr, 0), (mix, 2));
    link(&mut m, (mix, "Shader"), (out, "Surface"));
    let x = lower_m(&m, &images);
    assert_eq!(x.base_color.factor, [0.1, 0.2, 0.3, 1.0]);
    assert!(x.diagnostics.is_empty(), "{:?}", x.diagnostics);

    // A muted Principled BSDF is skipped for the next one that reaches the output.
    let p2 = add(&mut m, "ShaderNodeBsdfPrincipled");
    set_in(&mut m, p2, "Base Color", color(0.4, 0.5, 0.6));
    link_at(&mut m, (p2, 0), (mix, 2));
    m.node_tree.node_mut(BSDF).unwrap().mute = true;
    assert_eq!(lower_m(&m, &images).base_color.factor, [0.4, 0.5, 0.6, 1.0]);

    // The active output is the first one, whatever its target; a BSDF linked only to
    // another output is not exported.
    let mut m = mat("Outputs");
    set_in(&mut m, BSDF, "Base Color", color(0.1, 0.2, 0.3));
    set_enum(&mut m, out, "target", "CYCLES");
    let out2 = add(&mut m, "ShaderNodeOutputMaterial");
    let p2 = add(&mut m, "ShaderNodeBsdfPrincipled");
    set_in(&mut m, p2, "Base Color", color(0.4, 0.5, 0.6));
    link(&mut m, (p2, "BSDF"), (out2, "Surface"));
    let x = lower_m(&m, &images);
    assert_eq!(x.base_color.factor, [0.1, 0.2, 0.3, 1.0]);
}

// --------------------------------------------------------------------------------- raise

fn node_summary(m: &Material) -> Vec<(String, &'static str, String)> {
    m.node_tree
        .nodes
        .iter()
        .map(|n| (n.name.to_string(), n.bl_idname, n.label.clone()))
        .collect()
}

#[test]
fn raise_builds_the_importer_graph() {
    let mut images = Images::default();
    let albedo = images.add("albedo.png", false);
    // A glTF default material (baseColorFactor 0.8 grey, metallic 0, roughness 0.5).
    let mut lm = LoweredMaterial::gltf_default();
    lm.base_color.factor = [0.8, 0.8, 0.8, 1.0];
    lm.metallic.factor = 0.0;
    lm.roughness.factor = 0.5;
    lm.double_sided = true;
    let mut m = Material::new("M");
    raise(&lm, &mut m, &mut NameAlloc::default());
    let t = &m.node_tree;
    assert_eq!(
        node_summary(&m),
        [
            (
                "Principled BSDF".into(),
                "ShaderNodeBsdfPrincipled",
                String::new()
            ),
            (
                "Material Output".into(),
                "ShaderNodeOutputMaterial",
                String::new()
            ),
        ]
    );
    assert_eq!(
        (t.nodes[0].location, t.nodes[1].location),
        ([10.0, 300.0], [300.0, 300.0])
    );
    let b = &t.nodes[0];
    // Emission factor [0, 0, 0] folds into strength 0 with a white colour: Blender's default.
    assert_eq!(
        b.input("Emission Color").unwrap().default_value,
        SocketValue::Color([1.0; 4])
    );
    assert_eq!(
        b.input("Emission Strength").unwrap().default_value,
        SocketValue::Float(0.0)
    );
    assert_eq!(
        b.input("Base Color").unwrap().default_value,
        SocketValue::Color([0.8, 0.8, 0.8, 1.0])
    );
    assert_eq!(m.diffuse_color, [0.8, 0.8, 0.8, 1.0], "set_viewport_color");
    assert!(!m.use_backface_culling);
    assert_eq!(
        (m.surface_render_method.as_str(), m.blend_method.as_str()),
        ("DITHERED", "HASHED")
    );
    assert_eq!(t.links.len(), 1);

    // Base colour texture × factor with alpha clip and alpha factor (MASK 0.3, factor 0.8).
    let mut lm = LoweredMaterial::gltf_default();
    lm.base_color.factor = [0.5, 0.5, 0.5, 0.8];
    lm.base_color.texture = Some(TexRef {
        image: albedo,
        uv: UvSource::Index(1),
        channel: ChannelSel::Rgba,
        transform: None,
        sampler: Sampler {
            filter: Filter::Linear,
            wrap_s: Wrap::Repeat,
            wrap_t: Wrap::Mirror,
        },
        colorspace_is_data: false,
    });
    lm.alpha_mode = AlphaMode::Mask { cutoff: 0.3 };
    let mut m = Material::new("M");
    let mut names = NameAlloc::default();
    names.taken.insert("Image Texture".into()); // stale names are dropped by the rebuild
    raise(&lm, &mut m, &mut names);
    let summary = node_summary(&m);
    let find = |name: &str| summary.iter().find(|s| s.0 == name).cloned();
    assert_eq!(find("Mix").unwrap().2, "Color Factor");
    assert_eq!(find("Image Texture").unwrap().2, "BASE COLOR");
    assert_eq!(find("Frame").unwrap().2, "Alpha Clip");
    // Clip SUBTRACT, LESS_THAN, Alpha Factor, then the WRAP/PINGPONG UV math.
    assert_eq!(find("Math.002").unwrap().2, "Alpha Factor");
    assert_eq!(find("UV Map").unwrap().1, "ShaderNodeUVMap");
    assert_eq!(m.surface_render_method, "DITHERED");
    let tree = &m.node_tree;
    let img = tree.node_by_name("Image Texture").unwrap();
    assert_eq!(
        img.prop_str("extension"),
        "EXTEND",
        "mixed wraps use the UV math chain"
    );
    assert_eq!(
        tree.node_by_name("UV Map").unwrap().prop_str("uv_map"),
        "UVMap.001"
    );
    let lt = tree.node_by_name("Math.001").unwrap();
    assert_eq!(lt.prop_str("operation"), "LESS_THAN");
    assert_eq!(lt.inputs[1].default_value, SocketValue::Float(0.3));
    assert_eq!(lt.parent, tree.node_by_name("Frame").map(|f| f.uid));
    assert!(tree.links.iter().all(|l| tree.link_is_valid(l)));
    // Lowering it back recovers everything but the UV index (now the named map) and the
    // mixed wraps (M3 sampler pattern).
    let again = lower_m(&m, &images);
    assert_eq!(again.alpha_mode, AlphaMode::Mask { cutoff: 0.3 });
    assert_eq!(again.base_color.factor, [0.5, 0.5, 0.5, 0.8]);
    let t2 = again.base_color.texture.unwrap();
    assert_eq!(t2.channel, ChannelSel::Rgba);
    assert_eq!(t2.image, albedo);
}

// ---------------------------------------------------------------- extension-point rules

fn pattern_files() -> Vec<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/patterns");
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|f| f.ends_with(".rs") && !f.starts_with('_'))
        .map(|f| f.trim_end_matches(".rs").to_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn every_pattern_file_registers_exactly_one_entry() {
    let files = pattern_files();
    let mut names: Vec<&str> = MATERIAL_PATTERNS.iter().map(|p| p.name).collect();
    names.sort();
    assert_eq!(
        names, files,
        "one MATERIAL_PATTERNS entry per patterns/*.rs, named after it"
    );
    for f in &files {
        let src = std::fs::read_to_string(format!(
            "{}/src/patterns/{f}.rs",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        assert_eq!(
            src.matches("distributed_slice(crate::MATERIAL_PATTERNS)")
                .count(),
            1,
            "{f}.rs"
        );
    }
    let orders: std::collections::BTreeSet<u16> =
        MATERIAL_PATTERNS.iter().map(|p| p.order).collect();
    assert_eq!(orders.len(), MATERIAL_PATTERNS.len(), "orders are distinct");
    // Layout blocks: registered by the pattern files, distinct keys and orders.
    let blocks = &crate::build::LAYOUT_BLOCKS;
    let keys: std::collections::BTreeSet<&str> = blocks.iter().map(|b| b.key).collect();
    let block_orders: std::collections::BTreeSet<u16> = blocks.iter().map(|b| b.order).collect();
    assert_eq!(
        (keys.len(), block_orders.len()),
        (blocks.len(), blocks.len())
    );
    for k in ["base_color", "metallic_roughness", "emission"] {
        assert!(keys.contains(k), "layout block {k}");
    }
    for p in ["base_color", "metallic_roughness", "alpha", "emission"] {
        assert!(names.contains(&p), "M1 pattern {p}");
    }
}

#[test]
fn lower_and_raise_name_no_pattern() {
    let body = |src: &'static str, sig: &str| -> &'static str {
        let start = src.find(sig).unwrap_or_else(|| panic!("{sig}"));
        let end = start + src[start..].find("\n}\n").unwrap();
        &src[start..end]
    };
    let lowered = include_str!("../lowered.rs");
    let build = include_str!("build.rs");
    for f in [
        body(lowered, "pub fn lower("),
        body(lowered, "pub fn raise("),
        body(lowered, "pub fn patterns_in_order("),
        body(build, "pub fn calc_locations("),
    ] {
        assert!(!f.contains("patterns::"), "{f}");
        for p in pattern_files() {
            assert!(!f.contains(&format!("\"{p}\"")), "names pattern {p}: {f}");
            assert!(!f.contains(&format!("{p}::")), "names pattern {p}: {f}");
        }
    }
}

#[test]
fn image_lookup_through_main() {
    let mut main = frap_scene::Main::default();
    let mut log = frap_scene::ChangeLog::default();
    let mut im = Image::new("data.png", 2, 2);
    im.colorspace_settings.is_data = true;
    let r = main.images.insert("data.png", im, &mut log);
    let mut m = mat("M");
    let t = image_node(&mut m, r);
    link(&mut m, (t, "Color"), (BSDF, "Base Color"));
    let lm = lower(&m.node_tree, &m, &main);
    assert!(lm.base_color.texture.unwrap().colorspace_is_data);
}

// ------------------------------------------------- raise vs Blender's glTF importer (5.2.4)

/// `(name, bl_idname, label, location, parent)` of every node and the links
/// `(from, from_socket, to, to_socket)`, as dumped from Blender 5.2.4 after
/// `import_scene.gltf` of a material with `baseColorTexture` (texCoord 1, NEAREST,
/// REPEAT × MIRRORED_REPEAT), MASK 0.3, `baseColorFactor` [0.5, 0.5, 0.5, 0.8],
/// `metallicRoughnessTexture` with metallic 0.5, and `emissiveTexture` × [1, 0.5, 0.2].
type NodeRow = (
    &'static str,
    &'static str,
    &'static str,
    [f32; 2],
    Option<&'static str>,
);
const IMPORTED_NODES: [NodeRow; 18] = [
    (
        "Principled BSDF",
        "ShaderNodeBsdfPrincipled",
        "",
        [10.0, 300.0],
        None,
    ),
    (
        "Material Output",
        "ShaderNodeOutputMaterial",
        "",
        [300.0, 300.0],
        None,
    ),
    (
        "Mix",
        "ShaderNodeMix",
        "Emissive Factor",
        [-340.0, -250.0],
        None,
    ),
    (
        "Image Texture",
        "ShaderNodeTexImage",
        "EMISSIVE",
        [-640.0, -250.0],
        None,
    ),
    ("Frame", "NodeFrame", "Alpha Clip", [0.0, 0.0], None),
    ("Math", "ShaderNodeMath", "", [-340.0, 440.0], Some("Frame")),
    (
        "Math.001",
        "ShaderNodeMath",
        "",
        [-540.0, 440.0],
        Some("Frame"),
    ),
    (
        "Mix.001",
        "ShaderNodeMix",
        "Color Factor",
        [-740.0, 670.0],
        None,
    ),
    (
        "Math.002",
        "ShaderNodeMath",
        "Alpha Factor",
        [-740.0, 440.0],
        None,
    ),
    (
        "Image Texture.001",
        "ShaderNodeTexImage",
        "BASE COLOR",
        [-1040.0, 670.0],
        None,
    ),
    (
        "Combine XYZ",
        "ShaderNodeCombineXYZ",
        "",
        [-1280.0, 570.0],
        None,
    ),
    ("Math.003", "ShaderNodeMath", "", [-1480.0, 700.0], None),
    ("Math.004", "ShaderNodeMath", "", [-1480.0, 500.0], None),
    (
        "Separate XYZ",
        "ShaderNodeSeparateXYZ",
        "",
        [-1680.0, 570.0],
        None,
    ),
    ("UV Map", "ShaderNodeUVMap", "", [-1900.0, 600.0], None),
    (
        "Math.005",
        "ShaderNodeMath",
        "Metallic Factor",
        [-340.0, 210.0],
        None,
    ),
    (
        "Separate Color",
        "ShaderNodeSeparateColor",
        "",
        [-550.0, 135.0],
        None,
    ),
    (
        "Image Texture.002",
        "ShaderNodeTexImage",
        "METALLIC ROUGHNESS",
        [-840.0, 210.0],
        None,
    ),
];
const IMPORTED_LINKS: [(&str, &str, &str, &str); 19] = [
    ("Principled BSDF", "BSDF", "Material Output", "Surface"),
    ("Mix", "Result_Color", "Principled BSDF", "Emission Color"),
    ("Image Texture", "Color", "Mix", "A_Color"),
    ("Math", "Value", "Principled BSDF", "Alpha"),
    ("Math.001", "Value", "Math", "Value_001"),
    ("Mix.001", "Result_Color", "Principled BSDF", "Base Color"),
    ("Math.002", "Value", "Math.001", "Value"),
    ("Image Texture.001", "Color", "Mix.001", "A_Color"),
    ("Image Texture.001", "Alpha", "Math.002", "Value"),
    ("Combine XYZ", "Vector", "Image Texture.001", "Vector"),
    ("Math.003", "Value", "Combine XYZ", "X"),
    ("Math.004", "Value", "Combine XYZ", "Y"),
    ("Separate XYZ", "X", "Math.003", "Value"),
    ("Separate XYZ", "Y", "Math.004", "Value"),
    ("UV Map", "UV", "Separate XYZ", "Vector"),
    ("Math.005", "Value", "Principled BSDF", "Metallic"),
    ("Separate Color", "Blue", "Math.005", "Value"),
    ("Separate Color", "Green", "Principled BSDF", "Roughness"),
    ("Image Texture.002", "Color", "Separate Color", "Color"),
];

#[test]
fn raise_matches_blenders_gltf_importer() {
    let mut images = Images::default();
    let (albedo, orm, glow) = (
        images.add("albedo", false),
        images.add("orm", true),
        images.add("glow", false),
    );
    let tex = |image, uv, channel, sampler| TexRef {
        image,
        uv,
        channel,
        transform: None,
        sampler,
        colorspace_is_data: false,
    };
    let linear = Sampler {
        filter: Filter::Linear,
        wrap_s: Wrap::Repeat,
        wrap_t: Wrap::Repeat,
    };
    let nearest = Sampler {
        filter: Filter::Closest,
        wrap_s: Wrap::Repeat,
        wrap_t: Wrap::Mirror,
    };
    let mut lm = LoweredMaterial::gltf_default();
    lm.base_color = Channel {
        factor: [0.5, 0.5, 0.5, 0.8],
        texture: Some(tex(albedo, UvSource::Index(1), ChannelSel::Rgba, nearest)),
    };
    lm.alpha_mode = AlphaMode::Mask { cutoff: 0.3 };
    lm.metallic = Channel {
        factor: 0.5,
        texture: Some(TexRef {
            colorspace_is_data: true,
            ..tex(orm, UvSource::Index(0), ChannelSel::B, linear)
        }),
    };
    lm.roughness = Channel {
        factor: 1.0,
        texture: Some(TexRef {
            colorspace_is_data: true,
            ..tex(orm, UvSource::Index(0), ChannelSel::G, linear)
        }),
    };
    lm.emissive = Channel {
        factor: [1.0, 0.5, 0.2],
        texture: Some(tex(glow, UvSource::Index(0), ChannelSel::Rgb, linear)),
    };
    let mut m = Material::new("Textured");
    raise(&lm, &mut m, &mut NameAlloc::default());
    let t = &m.node_tree;
    let parent =
        |n: &frap_nodes::Node| n.parent.and_then(|u| t.node(u)).map(|p| p.name.to_string());
    let got: Vec<_> = t
        .nodes
        .iter()
        .map(|n| {
            (
                n.name.to_string(),
                n.bl_idname,
                n.label.clone(),
                n.location,
                parent(n),
            )
        })
        .collect();
    let want: Vec<_> = IMPORTED_NODES
        .iter()
        .map(|(a, b, c, d, e)| (a.to_string(), *b, c.to_string(), *d, e.map(str::to_owned)))
        .collect();
    // Same nodes in the same `tree.nodes` order (creation order, as Python sees it).
    assert_eq!(got, want);
    let links: Vec<_> = t
        .links
        .iter()
        .map(|l| {
            (
                t.node(l.from_node).unwrap().name.to_string(),
                l.from_socket.to_string(),
                t.node(l.to_node).unwrap().name.to_string(),
                l.to_socket.to_string(),
            )
        })
        .collect();
    let want_links: Vec<_> = IMPORTED_LINKS
        .iter()
        .map(|(a, b, c, d)| (a.to_string(), b.to_string(), c.to_string(), d.to_string()))
        .collect();
    // Same links in the same `tree.links` order.
    assert_eq!(links, want_links);
    let n = |name: &str| t.node_by_name(name).unwrap();
    assert_eq!(
        n("Principled BSDF")
            .input("Emission Strength")
            .unwrap()
            .default_value,
        float(1.0)
    );
    assert_eq!(
        n("Mix").input("B").unwrap().default_value,
        color(1.0, 0.5, 0.2)
    );
    assert_eq!(
        n("Mix.001").input("B").unwrap().default_value,
        color(0.5, 0.5, 0.5)
    );
    assert_eq!(n("Math.001").inputs[1].default_value, float(0.3));
    assert_eq!(n("Math.002").inputs[1].default_value, float(0.8));
    assert_eq!(n("Math.005").inputs[1].default_value, float(0.5));
    assert_eq!(n("Image Texture.001").prop_str("interpolation"), "Closest");
    assert_eq!(n("Image Texture.001").prop_str("extension"), "EXTEND");
    assert_eq!(n("UV Map").prop_str("uv_map"), "UVMap.001");
    assert_eq!(n("Math.003").prop_str("operation"), "WRAP");
    assert_eq!(n("Math.004").prop_str("operation"), "PINGPONG");
    assert_eq!(
        (m.surface_render_method.as_str(), m.blend_method.as_str()),
        ("DITHERED", "HASHED")
    );
    assert!(m.use_backface_culling);
    assert_eq!(
        m.diffuse_color,
        [0.8, 0.8, 0.8, 1.0],
        "a textured import keeps the viewport colour"
    );

    // The constant material: BLEND 0.5, grey emission folded into the strength.
    let mut lm = LoweredMaterial::gltf_default();
    lm.base_color.factor = [0.2, 0.3, 0.4, 0.5];
    lm.metallic.factor = 0.1;
    lm.roughness.factor = 0.7;
    lm.emissive.factor = [0.4; 3];
    lm.alpha_mode = AlphaMode::Blend;
    lm.double_sided = true;
    let mut m = Material::new("Plain");
    raise(&lm, &mut m, &mut NameAlloc::default());
    let b = &m.node_tree.nodes[0];
    assert_eq!(m.node_tree.nodes.len(), 2);
    assert_eq!(
        b.input("Base Color").unwrap().default_value,
        color(0.2, 0.3, 0.4)
    );
    assert_eq!(b.input("Metallic").unwrap().default_value, float(0.1));
    assert_eq!(b.input("Roughness").unwrap().default_value, float(0.7));
    assert_eq!(b.input("Alpha").unwrap().default_value, float(0.5));
    assert_eq!(
        b.input("Emission Color").unwrap().default_value,
        color(1.0, 1.0, 1.0)
    );
    assert_eq!(
        b.input("Emission Strength").unwrap().default_value,
        float(0.4)
    );
    assert_eq!(
        (m.surface_render_method.as_str(), m.blend_method.as_str()),
        ("BLENDED", "BLEND")
    );
    assert!(!m.use_backface_culling);
    assert_eq!(m.diffuse_color, [0.2, 0.3, 0.4, 0.5]);
}
