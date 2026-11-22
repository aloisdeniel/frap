// SPDX-License-Identifier: GPL-3.0-or-later
//! The node-tree properties a material's glTF factors are read from (`current_paths` of
//! `gather_material`), for `KHR_animation_pointer`.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors):
//! `blender/exp/material/search_node_tree.py` (`NodeNav.get_constant` / `get_factor`,
//! `get_multiply_factors`, `detect_multiply_by_constant`, `gather_color_info`,
//! `gather_alpha_info`, `detect_alpha_clip`, `get_const_from_default_value_socket`: each
//! returns the value together with the path of the socket it came from) and the gatherers that
//! store those paths, in the order `gather_material` runs them:
//!
//! 1. `extensions/emission.py`: `emissiveFactor` (the factor of the `Emissive` socket, with the
//!    emission strength as `strength_channel`) and `KHR_materials_emissive_strength` (the
//!    strength, with the colour as `factor_channel`);
//! 2. `extensions/clearcoat.py`: `clearcoatFactor`, `clearcoatRoughnessFactor`;
//! 3. `extensions/transmission.py`: `transmissionFactor`;
//! 4. `extensions/volume.py` (only with the glTF Material Output's `Thickness`):
//!    `attenuationColor`, `attenuationDistance` (the Volume Absorption's `Color` and `Density`),
//!    `thicknessFactor`;
//! 5. `extensions/specular.py`: `specularFactor`, `specularColorFactor`;
//! 6. `extensions/sheen.py` (with a sheen weight): `sheenColorFactor`, `sheenRoughnessFactor`;
//!    `extensions/anisotropy.py` (both sockets unlinked): `anisotropyStrength` and the raw
//!    `Anisotropic Rotation` (the add-on samples it unconverted, unlike the static export);
//! 7. `extensions/ior.py`: `ior` (unlinked, with transmission, volume or specular, which a
//!    Principled material always has);
//! 8. `pbr_metallic_roughness.py`: `baseColorFactor` from the colour and from the alpha (both
//!    registered, merged into one VEC4 channel when sampled), `metallicFactor`,
//!    `roughnessFactor`;
//! 9. `materials.py`: `alphaCutoff` (an alpha clip whose cutoff is a socket).
//!
//! From WP 7.7: `texture_info.py` `__gather_normal_scale` (`normalTexture/scale`, the Normal
//! Map's `Strength`) and `__gather_occlusion_strength` (`occlusionTexture/strength`, the
//! Mix's `Factor`, reversed when the white is its `B` input) after the extensions, as
//! `__gather_normal_texture` / `__gather_occlusion_texture` merge them; `unlit.py`
//! `gather_base_color_factor` for an unlit material (the colour factor, with the alpha as its
//! `additional_path`); `export_image_format='NONE'` reading the base colour's and emission's
//! own sockets. `extensions/iridescence.py` (the glTF Material Output's `Iridescence Factor`,
//! the `Thin Film` sockets and the thickness texture set-up) and `extensions/dispersion.py`
//! (with a volume) are registered after the anisotropy.
//!
//! Also from WP 7.7, the `KHR_texture_transform` of every texture the gatherers write
//! (`get_texture_transform_from_mapping_node`'s `current_texture_transform`: the Mapping
//! node's `Location` → `offset`, `Scale` → `scale`, `Rotation[2]` → `rotation`), merged where
//! each gatherer merges them: the ORM texture first (`__gather_orm_texture`, with the
//! metallic-roughness texture as `additional`), the emissive texture after the emission, each
//! extension's textures after their factor, the normal texture before its scale, the
//! occlusion texture after its strength, the base colour and metallic-roughness textures
//! before the PBR factors. A key registered twice (one Mapping node or one Normal Map feeding
//! several textures, `clearcoatNormalTexture/scale` and `normalTexture/scale`) keeps its first
//! pointer and gets the others as `additional` (one channel copy each). The importer's
//! anisotropy texture set-up (`detect_anisotropy_nodes`) registers the strength and rotation
//! constants and its texture's transform; the anisotropy baked from grayscale textures its
//! texture's transform, then the strength and rotation factors (WP 8.12).
//!
//! The material is read from its original node tree (an animated tree is never inlined), so
//! no constant folding applies here. Not registered: light node trees (frap has none).

use frap_gltf::json::Material as GMaterial;
use frap_material::LowerCx;
use frap_material::graph::{In, Src};
use frap_nodes::{Node, SocketType, SocketValue};
use frap_scene::Main;
use frap_scene::id::Material;
use indexmap::IndexMap;

use super::registry::PathInfo;

/// `"node_tree." + socket.path_from_id() + ".default_value"` of input `i` of `node`.
pub fn input_key(node: &Node, i: usize) -> String {
    format!(
        "node_tree.nodes[\"{}\"].inputs[{i}].default_value",
        escape(&node.name)
    )
}

/// The same for output `o` (a Value or RGB node's value).
pub fn output_key(node: &Node, o: usize) -> String {
    format!(
        "node_tree.nodes[\"{}\"].outputs[{o}].default_value",
        escape(&node.name)
    )
}

/// RNA path string escaping (`BLI_str_escape`: `\` and `"`).
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A constant `get_constant` found: its value (the colour's RGB, a float) and the path of the
/// socket holding it (`None` for constants the add-on cannot animate).
#[derive(Clone, Debug, PartialEq)]
struct Konst {
    rgb: Option<[f32; 3]>,
    value: Option<f32>,
    path: Option<String>,
}

/// The navigation of `search_node_tree.py` on the original tree.
struct Nav<'a> {
    cx: LowerCx<'a>,
}

impl<'a> Nav<'a> {
    fn is_linked(&self, i: In<'a>) -> bool {
        self.cx
            .tree
            .link_into(i.node.uid, &i.socket().identifier)
            .is_some()
    }

    /// `peek_back()`: the node and output the input is linked from, reroutes skipped (an
    /// unlinked reroute stops the navigation: `None`).
    fn peek_back(&self, i: In<'a>) -> Option<(&'a Node, usize)> {
        match self.cx.src_in(i) {
            Src::Linked(n, o) => Some((n, o)),
            _ => None,
        }
    }

    /// `NodeNav.get_constant()`.
    fn constant(&self, i: In<'a>) -> Option<Konst> {
        let s = i.socket();
        if !self.is_linked(i) {
            return match (s.ty, &s.default_value) {
                (SocketType::Rgba, SocketValue::Color(c)) => Some(Konst {
                    rgb: Some([c[0], c[1], c[2]]),
                    value: None,
                    path: Some(input_key(i.node, i.index)),
                }),
                (SocketType::Shader, _) => Some(Konst {
                    rgb: Some([0.0; 3]),
                    value: None,
                    path: None,
                }),
                (SocketType::Vector, _) => Some(Konst {
                    rgb: None,
                    value: None,
                    path: None,
                }),
                (SocketType::Value, &SocketValue::Float(f)) => Some(Konst {
                    rgb: None,
                    value: Some(f),
                    path: Some(input_key(i.node, i.index)),
                }),
                _ => None,
            };
        }
        let mut cur = i;
        for _ in 0..=self.cx.tree.nodes.len() {
            let (n, o) = self.peek_back(cur)?;
            match (s.ty, n.bl_idname) {
                (SocketType::Rgba | SocketType::Shader, "ShaderNodeRGB") => {
                    return match &n.outputs[o].default_value {
                        SocketValue::Color(c) => Some(Konst {
                            rgb: Some([c[0], c[1], c[2]]),
                            value: None,
                            path: Some(output_key(n, o)),
                        }),
                        _ => None,
                    };
                }
                (SocketType::Rgba | SocketType::Shader, "ShaderNodeAmbientOcclusion") => {
                    let color = In::named(n, "Color")?;
                    if !self.is_linked(color) {
                        return match &color.socket().default_value {
                            SocketValue::Color(c) => Some(Konst {
                                rgb: Some([c[0], c[1], c[2]]),
                                value: None,
                                path: Some(input_key(n, color.index)),
                            }),
                            _ => None,
                        };
                    }
                    cur = color;
                }
                (SocketType::Value, "ShaderNodeValue") => {
                    return match n.outputs[o].default_value {
                        SocketValue::Float(f) => Some(Konst {
                            rgb: None,
                            value: Some(f),
                            path: Some(output_key(n, o)),
                        }),
                        _ => None,
                    };
                }
                _ => return None,
            }
        }
        None
    }

    /// `NodeNav.get_factor()`: a constant, or the single constant input of a Mix (RGBA,
    /// MULTIPLY) or Math MULTIPLY feeding the socket.
    fn factor(&self, i: In<'a>) -> Option<Konst> {
        if let Some(c) = self.constant(i) {
            return Some(c);
        }
        let (n, _) = self.peek_back(i)?;
        let (x1, x2) = match i.socket().ty {
            SocketType::Rgba
                if n.bl_idname == "ShaderNodeMix"
                    && n.prop_str("data_type") == "RGBA"
                    && n.prop_str("blend_type") == "MULTIPLY" =>
            {
                (
                    In::by_identifier(n, "A_Color").and_then(|s| self.constant(s)),
                    In::by_identifier(n, "B_Color").and_then(|s| self.constant(s)),
                )
            }
            SocketType::Value
                if n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY" =>
            {
                (
                    self.constant(In { node: n, index: 0 }),
                    self.constant(In { node: n, index: 1 }),
                )
            }
            _ => return None,
        };
        match (x1, x2) {
            (Some(c), None) | (None, Some(c)) => Some(c),
            _ => None,
        }
    }

    /// `get_multiply_factors(nav)`.
    fn multiply_factors(&self, i: In<'a>) -> Option<(In<'a>, In<'a>)> {
        let (n, _) = self.peek_back(i)?;
        match i.socket().ty {
            SocketType::Rgba => {
                let is_mul = n.bl_idname == "ShaderNodeMix"
                    && n.prop_str("data_type") == "RGBA"
                    && n.prop_str("blend_type") == "MULTIPLY"
                    && In::named(n, "Factor")
                        .and_then(|f| self.constant(f))
                        .and_then(|k| k.value)
                        == Some(1.0);
                if !is_mul {
                    return None;
                }
                Some((
                    In::by_identifier(n, "A_Color")?,
                    In::by_identifier(n, "B_Color")?,
                ))
            }
            SocketType::Value
                if n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == "MULTIPLY" =>
            {
                Some((In { node: n, index: 0 }, In { node: n, index: 1 }))
            }
            _ => None,
        }
    }

    /// `detect_multiply_by_constant(nav)`: the constant and the other factor.
    fn multiply_by_constant(&self, i: In<'a>) -> Option<(Konst, In<'a>)> {
        let (f1, f2) = self.multiply_factors(i)?;
        if let Some(c) = self.constant(f1) {
            return Some((c, f2));
        }
        self.constant(f2).map(|c| (c, f1))
    }

    /// `get_color_attrib(nav)` (whether a colour attribute feeds the socket).
    fn color_attrib(&self, i: In<'a>) -> bool {
        match self.peek_back(i) {
            Some((n, _)) => {
                n.bl_idname == "ShaderNodeVertexColor"
                    || (n.bl_idname == "ShaderNodeAttribute"
                        && n.prop_str("attribute_type") == "GEOMETRY")
            }
            None => false,
        }
    }

    /// `detect_multiply_by_color_attrib(nav)`: the other factor.
    fn multiply_by_color_attrib(&self, i: In<'a>) -> Option<In<'a>> {
        let (f1, f2) = self.multiply_factors(i)?;
        if self.color_attrib(f1) {
            return Some(f2);
        }
        self.color_attrib(f2).then_some(f1)
    }

    /// `gather_color_info(nav)['colorPath']` (and the alpha variant's `alphaPath`): the path
    /// of the constant factor of `factor × colour attribute × texture`.
    fn factor_path(&self, mut nav: In<'a>) -> Option<String> {
        let mut factor_found = false;
        let mut attrib_found = false;
        let mut path = None;
        for _ in 0..2 {
            if !factor_found {
                if let Some(c) = self.constant(nav) {
                    return c.path;
                }
                if let Some((c, other)) = self.multiply_by_constant(nav) {
                    factor_found = true;
                    path = c.path;
                    nav = other;
                    continue;
                }
            }
            if !attrib_found {
                if self.color_attrib(nav) {
                    break;
                }
                if let Some(other) = self.multiply_by_color_attrib(nav) {
                    attrib_found = true;
                    nav = other;
                    continue;
                }
            }
            break;
        }
        path
    }

    /// `detect_alpha_clip(alpha_nav)`: the cutoff's path (when it has one) and the socket the
    /// alpha continues from.
    fn alpha_clip(&self, i: In<'a>) -> Option<(Option<String>, In<'a>)> {
        let (n, _) = self.peek_back(i)?;
        if n.bl_idname != "ShaderNodeMath" {
            return None;
        }
        let op = n.prop_str("operation");
        let at = |k: usize| In { node: n, index: k };
        if op == "ROUND" {
            return Some((None, at(0)));
        }
        if op == "SUBTRACT"
            && self.constant(at(0)).and_then(|c| c.value) == Some(1.0)
            && let Some((n2, _)) = self.peek_back(at(1))
            && n2.bl_idname == "ShaderNodeMath"
        {
            let at2 = |k: usize| In { node: n2, index: k };
            let (in0, in1) = (self.constant(at2(0)), self.constant(at2(1)));
            let op2 = n2.prop_str("operation");
            if op2 == "LESS_THAN" && in0.is_none() && in1.is_some() {
                return Some((in1.and_then(|c| c.path), at2(0)));
            }
            if op2 == "GREATER_THAN" && in0.is_some() && in1.is_none() {
                return Some((in0.and_then(|c| c.path), at2(1)));
            }
        }
        let (in0, in1) = (self.constant(at(0)), self.constant(at(1)));
        if op == "GREATER_THAN" && in1.is_some() {
            return Some((in1.and_then(|c| c.path), at(0)));
        }
        if op == "LESS_THAN" && in0.is_some() {
            return Some((in0.and_then(|c| c.path), at(1)));
        }
        None
    }

    /// `gather_alpha_info`: `(alphaPath, alphaCutoffPath when the mode is MASK)`.
    fn alpha_info(&self, i: In<'a>) -> (Option<String>, Option<String>) {
        if let Some(c) = self.constant(i)
            && c.value == Some(1.0)
        {
            return (c.path, None);
        }
        let (mut cutoff, mut nav, mut mask) = (None, i, false);
        if let Some((path, next)) = self.alpha_clip(i) {
            cutoff = path;
            nav = next;
            mask = true;
        }
        let alpha_path = self.factor_path(nav);
        (alpha_path, if mask { cutoff } else { None })
    }

    /// `has_image_node_from_socket`.
    fn has_image(&self, i: In<'a>) -> bool {
        self.cx.has_image(Some(i))
    }

    /// The factor path of an extension socket (`get_factor_from_socket` when an image is
    /// behind it).
    fn ext_factor(&self, i: In<'a>) -> Option<String> {
        if self.is_linked(i) {
            if self.has_image(i) {
                return self.factor(i).and_then(|c| c.path);
            }
            None
        } else {
            Some(input_key(i.node, i.index))
        }
    }
}

/// What [`paths`] found for a material: `current_paths`, and the properties the add-on
/// registers under the key `None` (a factor without an animatable source: its sampler then
/// fails with `'NoneType' object has no attribute 'startswith'`; frap leaves them out).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Found {
    /// `current_paths`, in registration order.
    pub paths: IndexMap<String, PathInfo>,
    /// The members registered without a path.
    pub none_keys: Vec<&'static str>,
}

impl Found {
    /// `current_paths[key] = path_` (assigning an existing key keeps its place).
    fn put(&mut self, key: Option<String>, info: PathInfo) {
        if let Some(k) = key {
            self.paths.insert(k, info);
        }
    }

    /// [`Self::put`] for a member the add-on registers even without a path.
    fn put_or_none(&mut self, key: Option<String>, info: PathInfo, member: &'static str) {
        match key {
            Some(k) => {
                self.paths.insert(k, info);
            }
            None => {
                if !self.none_keys.contains(&member) {
                    self.none_keys.push(member);
                }
            }
        }
    }

    /// The merge of `current_texture_transform`, `current_normal_scale` and
    /// `current_occlusion_strength` into `current_paths`: a key already registered keeps its
    /// path and gets this one as `additional` when it differs.
    fn merge(&mut self, key: String, info: PathInfo) {
        match self.paths.get_mut(&key) {
            Some(p) => {
                if p.pointer != info.pointer {
                    p.additional.push(info.pointer);
                }
            }
            None => {
                self.paths.insert(key, info);
            }
        }
    }
}

/// Whether the extension `ext` of the glTF material has the member `member` (a texture the
/// gatherer wrote).
fn ext_has(gm: &GMaterial, ext: &str, member: &str) -> bool {
    gm.extensions
        .get_raw(ext)
        .is_some_and(|v| v.get(member).is_some())
}

/// `current_paths` of `gather_material` for `mat` (empty when the material has nothing
/// animatable the exporter reads).
///
/// `gm` is the glTF material written for it: an unlit one registers only its colour and its
/// texture's transform (`unlit.py`); the normal map's `Strength`, the occlusion strength and
/// the `KHR_texture_transform` of a texture are registered only when the texture was written
/// (`__gather_texture_info_helper` gathers them after its filter). `image_none` is
/// `export_image_format='NONE'`: the base colour and the emission are read from their
/// sockets' own values (`get_const_from_default_value_socket`). `orm` is
/// `__gather_orm_texture`'s answer (occlusion, roughness and metallic in one image): the
/// occlusion texture's transform is then registered first, with the metallic-roughness
/// texture as its `additional` pointer.
pub fn paths(main: &Main, mat: &Material, gm: &GMaterial, image_none: bool, orm: bool) -> Found {
    let nav = Nav {
        cx: LowerCx::new(&mat.node_tree, mat, main).with_original(true),
    };
    let mut p = Found::default();
    let pbr_tex = gm.pbr_metallic_roughness.as_ref();
    let has_base_color_texture = pbr_tex.is_some_and(|x| x.base_color_texture.is_some());
    if gm.extensions.contains("KHR_materials_unlit") {
        // `__export_unlit`: `gather_base_color_texture`, then the colour factor (its alpha
        // path is not stored on its own, only as the colour's `additional_path`).
        if let Some((rgb, alpha)) = frap_material::patterns::unlit::shadeless_sockets(&nav.cx) {
            if has_base_color_texture {
                texture_transform(&nav, Some(rgb), BASE_COLOR_TEXTURE, &mut p);
            }
            let alpha_path = alpha.and_then(|a| nav.alpha_info(a).0);
            p.put(
                nav.factor_path(rgb),
                PathInfo {
                    additional_path: alpha_path,
                    ..PathInfo::new("/materials/XXX/pbrMetallicRoughness/baseColorFactor", 3)
                },
            );
        }
        return p;
    }
    let occlusion = nav
        .cx
        .get_socket("Occlusion")
        .filter(|&s| nav.has_image(s))
        .or_else(|| nav.cx.gltf_output_input("Occlusion"));
    if orm {
        // `__gather_orm_texture`: the occlusion texture's transform, then the same keys for
        // the metallic-roughness texture (`additional`).
        texture_transform(&nav, occlusion, "occlusionTexture/extensions", &mut p);
        texture_transform(&nav, occlusion, METALLIC_ROUGHNESS_TEXTURE, &mut p);
    }
    emission(&nav, &mut p, image_none);
    if gm.emissive_texture.is_some() {
        let s = nav
            .cx
            .get_socket("Emissive")
            .or_else(|| nav.cx.gltf_output_input("Emissive"));
        texture_transform(&nav, s, "emissiveTexture/extensions", &mut p);
    }
    clearcoat(&nav, gm, &mut p);
    transmission(&nav, gm, &mut p);
    volume(&nav, gm, &mut p);
    specular(&nav, gm, &mut p);
    sheen(&nav, gm, &mut p);
    anisotropy(&nav, gm, &mut p);
    iridescence(&nav, gm, &mut p);
    if gm.extensions.contains("KHR_materials_volume") {
        dispersion(&nav, &mut p);
    }
    // ior: unlinked, and a transmission/specular extension object exists (always for a
    // material with these sockets; `export_transmission` creates one unconditionally).
    if let Some(s) = nav.cx.get_socket("IOR")
        && !nav.is_linked(s)
    {
        p.put(
            Some(input_key(s.node, s.index)),
            PathInfo::new("/materials/XXX/extensions/KHR_materials_ior/ior", 1),
        );
    }
    if gm.normal_texture.is_some() {
        // `__gather_normal_texture`: the transform, then the scale.
        let normal = nav.cx.get_socket("Normal");
        texture_transform(&nav, normal, "normalTexture/extensions", &mut p);
        normal_scale(&nav, normal, "/materials/XXX/normalTexture/scale", &mut p);
    }
    if gm.occlusion_texture.is_some() {
        // `__gather_occlusion_texture`: the strength, then the transform.
        let occ = nav
            .cx
            .get_socket("Occlusion")
            .or_else(|| nav.cx.gltf_output_input("Occlusion"));
        occlusion_strength(&nav, occ, &mut p);
        texture_transform(&nav, occ, "occlusionTexture/extensions", &mut p);
    }
    pbr(&nav, gm, &mut p, image_none);
    p
}

const BASE_COLOR_TEXTURE: &str = "pbrMetallicRoughness/baseColorTexture/extensions";
const METALLIC_ROUGHNESS_TEXTURE: &str = "pbrMetallicRoughness/metallicRoughnessTexture/extensions";

/// The components of a Vector socket's default value.
fn vector_of(n: &Node, name: &str) -> [f32; 3] {
    match In::named(n, name).map(|i| &i.socket().default_value) {
        Some(SocketValue::Vector(v)) => [v[0], v[1], v[2]],
        _ => [0.0; 3],
    }
}

/// The Mapping node `__gather_texture_transform_and_tex_coord` finds in front of the texture
/// `primary` reads (`[Mapping] => [UV wrapping] => [Image Texture]`), when
/// `get_texture_transform_from_mapping_node` accepts it: `vector_type` POINT, TEXTURE or
/// VECTOR, no X / Y rotation, and for TEXTURE an inverse that is a TRS
/// (`inverted_trs_mapping_node`). Only then are its paths registered.
pub fn mapping_of<'a>(cx: &LowerCx<'a>, primary: In<'a>) -> Option<&'a Node> {
    let tex = cx.texture_from(primary)?.node;
    let wrapping = frap_material::patterns::sampler::detect_manual_uv_wrapping(cx, tex);
    let src = match &wrapping {
        Some(w) => cx.src_in(w.next),
        None => cx.src(tex, "Vector"),
    };
    let Src::Linked(n, _) = src else {
        return None;
    };
    if n.bl_idname != "ShaderNodeMapping" {
        return None;
    }
    let vector_type = n.prop_str("vector_type");
    if !matches!(vector_type, "POINT" | "TEXTURE" | "VECTOR") {
        return None;
    }
    let rot = vector_of(n, "Rotation");
    if rot[0] != 0.0 || rot[1] != 0.0 {
        return None;
    }
    if vector_type == "TEXTURE" {
        let s = vector_of(n, "Scale").map(f64::from);
        let r = f64::from(rot[2]);
        if (r.abs() > 1e-5 && (s[0] - s[1]).abs() > 1e-5) || s[0].abs() < 1e-5 || s[1].abs() < 1e-5
        {
            return None;
        }
    }
    Some(n)
}

/// `get_texture_transform_from_mapping_node`'s `current_texture_transform` (the Mapping's
/// `Location` as `offset` unless VECTOR, `Scale` as `scale`, `Rotation[2]` as `rotation`) for
/// the texture `primary` reads, merged into the paths under `slot` (the `YYY` of the
/// pointer: `pbrMetallicRoughness/baseColorTexture/extensions`, …).
fn texture_transform(nav: &Nav, primary: Option<In>, slot: &str, p: &mut Found) {
    let Some(n) = primary.and_then(|i| mapping_of(&nav.cx, i)) else {
        return;
    };
    let vector_type = n.prop_str("vector_type");
    let info = |member: &str, length: usize| PathInfo {
        vector_type: Some(vector_type.into()),
        ..PathInfo::new(
            &format!("/materials/XXX/{slot}/KHR_texture_transform/{member}"),
            length,
        )
    };
    if vector_type != "VECTOR"
        && let Some(l) = In::named(n, "Location")
    {
        p.merge(input_key(n, l.index), info("offset", 2));
    }
    if let Some(s) = In::named(n, "Scale") {
        p.merge(input_key(n, s.index), info("scale", 2));
    }
    if let Some(r) = In::named(n, "Rotation") {
        p.merge(format!("{}[2]", input_key(n, r.index)), info("rotation", 1));
    }
}

/// `__gather_normal_scale`: the `Strength` of the first Normal Map node behind `socket`
/// (`from_socket` with `FilterByType(ShaderNodeNormalMap)`; registered linked or not),
/// merged under `pointer`.
fn normal_scale(nav: &Nav, socket: Option<In>, pointer: &str, p: &mut Found) {
    let Some(n) = socket.and_then(|s| nav.cx.node_from(s, "ShaderNodeNormalMap")) else {
        return;
    };
    let Some(strength) = In::named(n, "Strength") else {
        return;
    };
    p.merge(input_key(n, strength.index), PathInfo::new(pointer, 1));
}

/// `__gather_occlusion_strength`: the constant `Factor` of a Mix (MIX) node in front of the
/// occlusion socket, with `reverse` when the white is its `B` input. A Factor without a
/// constant source registers the key `None`.
fn occlusion_strength(nav: &Nav, occlusion: Option<In>, p: &mut Found) {
    let Some((n, _)) = occlusion.and_then(|o| nav.peek_back(o)) else {
        return;
    };
    if n.bl_idname != "ShaderNodeMix" || n.prop_str("blend_type") != "MIX" {
        return;
    }
    let fac = In::named(n, "Factor").and_then(|f| nav.constant(f));
    let rgb = |id: &str| {
        In::by_identifier(n, id)
            .and_then(|s| nav.constant(s))
            .and_then(|c| c.rgb)
    };
    let reverse = fac.as_ref().is_some_and(|f| f.value.is_some())
        && rgb("A_Color").is_none()
        && rgb("B_Color") == Some([1.0; 3]);
    let info = PathInfo {
        reverse,
        ..PathInfo::new("/materials/XXX/occlusionTexture/strength", 1)
    };
    match fac.and_then(|c| c.path) {
        Some(k) => p.merge(k, info),
        None => {
            if !p.none_keys.contains(&"occlusionTexture/strength") {
                p.none_keys.push("occlusionTexture/strength");
            }
        }
    }
}

/// `export_ior`'s value: the `IOR` socket's default value when the socket exists and is not
/// linked (`None` otherwise: the add-on creates no `KHR_materials_ior` object then).
pub fn ior(main: &Main, mat: &Material) -> Option<f32> {
    let nav = Nav {
        cx: LowerCx::new(&mat.node_tree, mat, main),
    };
    let s = nav.cx.get_socket("IOR")?;
    if nav.is_linked(s) {
        return None;
    }
    match s.socket().default_value {
        SocketValue::Float(f) => Some(f),
        _ => None,
    }
}

/// `export_emission_factor`.
fn emission(nav: &Nav, p: &mut Found, image_none: bool) {
    let Some(s) = nav
        .cx
        .get_socket("Emissive")
        .or_else(|| nav.cx.gltf_output_input("EmissiveFactor"))
    else {
        return;
    };
    let path = if image_none {
        (s.socket().ty == SocketType::Rgba).then(|| input_key(s.node, s.index))
    } else {
        nav.factor(s).and_then(|c| c.path)
    };
    let strength_socket = if s.node.bl_idname == "ShaderNodeEmission" {
        In::named(s.node, "Strength")
    } else {
        In::named(s.node, "Emission Strength")
    };
    let strength_path = strength_socket.and_then(|st| nav.factor(st).and_then(|c| c.path));
    if path.is_some() {
        p.put(
            path.clone(),
            PathInfo {
                strength_channel: strength_path.clone(),
                ..PathInfo::new("/materials/XXX/emissiveFactor", 3)
            },
        );
    }
    if strength_path.is_some() {
        p.put(
            strength_path,
            PathInfo {
                factor_channel: path,
                ..PathInfo::new(
                    "/materials/XXX/extensions/KHR_materials_emissive_strength/emissiveStrength",
                    1,
                )
            },
        );
    }
}

const CLEARCOAT: &str = "KHR_materials_clearcoat";

/// `export_clearcoat`: the two factors, the transforms of `clearcoatTexture` and
/// `clearcoatRoughnessTexture`, then `clearcoatNormalTexture`'s scale (the Normal Map's
/// `Strength`, merged: one Normal Map feeding `Normal` too adds `normalTexture/scale` to it
/// later) and transform.
fn clearcoat(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    let e = format!("/materials/XXX/extensions/{CLEARCOAT}");
    let weight = nav.cx.get_socket("Coat Weight");
    let roughness = nav.cx.get_socket("Coat Roughness");
    for (s, member) in [
        (weight, "clearcoatFactor"),
        (roughness, "clearcoatRoughnessFactor"),
    ] {
        if let Some(s) = s {
            p.put(
                nav.ext_factor(s),
                PathInfo::new(&format!("{e}/{member}"), 1),
            );
        }
    }
    let slot = |t: &str| format!("extensions/{CLEARCOAT}/{t}/extensions");
    if ext_has(gm, CLEARCOAT, "clearcoatTexture") {
        texture_transform(nav, weight, &slot("clearcoatTexture"), p);
    }
    if ext_has(gm, CLEARCOAT, "clearcoatRoughnessTexture") {
        texture_transform(nav, roughness, &slot("clearcoatRoughnessTexture"), p);
    }
    let normal = nav.cx.get_socket("Coat Normal");
    if ext_has(gm, CLEARCOAT, "clearcoatNormalTexture") && normal.is_some_and(|s| nav.has_image(s))
    {
        normal_scale(nav, normal, &format!("{e}/clearcoatNormalTexture/scale"), p);
        texture_transform(nav, normal, &slot("clearcoatNormalTexture"), p);
    }
}

/// `export_transmission`.
fn transmission(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    let Some(s) = nav.cx.get_socket("Transmission Weight") else {
        return;
    };
    p.put(
        nav.ext_factor(s),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_transmission/transmissionFactor",
            1,
        ),
    );
    if ext_has(gm, "KHR_materials_transmission", "transmissionTexture") {
        texture_transform(
            nav,
            Some(s),
            "extensions/KHR_materials_transmission/transmissionTexture/extensions",
            p,
        );
    }
}

/// `export_volume`.
fn volume(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    let Some(thickness) = nav.cx.gltf_output_input("Thickness") else {
        return;
    };
    let absorption = nav.cx.linked_node_of_type("ShaderNodeVolumeAbsorption");
    let va = |name: &str| absorption.and_then(|n| In::named(n, name));
    if let Some(c) = va("Color")
        && c.socket().ty == SocketType::Rgba
    {
        p.put(
            Some(input_key(c.node, c.index)),
            PathInfo::new(
                "/materials/XXX/extensions/KHR_materials_volume/attenuationColor",
                3,
            ),
        );
    }
    if let Some(d) = va("Density")
        && d.socket().ty == SocketType::Value
    {
        p.put(
            Some(input_key(d.node, d.index)),
            PathInfo::new(
                "/materials/XXX/extensions/KHR_materials_volume/attenuationDistance",
                1,
            ),
        );
    }
    let info = PathInfo::new(
        "/materials/XXX/extensions/KHR_materials_volume/thicknessFactor",
        1,
    );
    if !nav.is_linked(thickness) {
        if thickness.socket().default_value == SocketValue::Float(0.0) {
            return;
        }
        p.put(Some(input_key(thickness.node, thickness.index)), info);
    } else if nav.has_image(thickness) {
        p.put(nav.factor(thickness).and_then(|c| c.path), info);
        if ext_has(gm, "KHR_materials_volume", "thicknessTexture") {
            texture_transform(
                nav,
                Some(thickness),
                "extensions/KHR_materials_volume/thicknessTexture/extensions",
                p,
            );
        }
    }
}

/// The path of an extension socket read unlinked (its own value) or linked (its factor).
fn socket_or_factor(nav: &Nav, s: In) -> Option<String> {
    if nav.is_linked(s) {
        nav.factor(s).and_then(|c| c.path)
    } else {
        Some(input_key(s.node, s.index))
    }
}

/// `export_specular`: the factor and its texture's transform, then the colour and its
/// texture's transform.
fn specular(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    const EXT: &str = "KHR_materials_specular";
    let (Some(spec), Some(tint)) = (
        nav.cx.get_socket("Specular IOR Level"),
        nav.cx.get_socket("Specular Tint"),
    ) else {
        return;
    };
    let e = format!("/materials/XXX/extensions/{EXT}");
    p.put(
        socket_or_factor(nav, spec),
        PathInfo::new(&format!("{e}/specularFactor"), 1),
    );
    if ext_has(gm, EXT, "specularTexture") {
        texture_transform(
            nav,
            Some(spec),
            &format!("extensions/{EXT}/specularTexture/extensions"),
            p,
        );
    }
    p.put(
        socket_or_factor(nav, tint),
        PathInfo::new(&format!("{e}/specularColorFactor"), 3),
    );
    if ext_has(gm, EXT, "specularColorTexture") {
        texture_transform(
            nav,
            Some(tint),
            &format!("extensions/{EXT}/specularColorTexture/extensions"),
            p,
        );
    }
}

/// `export_sheen`: the colour and its texture's transform, then the roughness and its
/// texture's transform.
fn sheen(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    const EXT: &str = "KHR_materials_sheen";
    let (Some(tint), Some(rough), Some(weight)) = (
        nav.cx.get_socket("Sheen Tint"),
        nav.cx.get_socket("Sheen Roughness"),
        nav.cx.get_socket("Sheen Weight"),
    ) else {
        return;
    };
    if !nav.is_linked(weight) && weight.socket().default_value == SocketValue::Float(0.0) {
        return;
    }
    let e = format!("/materials/XXX/extensions/{EXT}");
    p.put(
        socket_or_factor(nav, tint),
        PathInfo::new(&format!("{e}/sheenColorFactor"), 3),
    );
    if ext_has(gm, EXT, "sheenColorTexture") {
        texture_transform(
            nav,
            Some(tint),
            &format!("extensions/{EXT}/sheenColorTexture/extensions"),
            p,
        );
    }
    p.put(
        socket_or_factor(nav, rough),
        PathInfo::new(&format!("{e}/sheenRoughnessFactor"), 1),
    );
    if ext_has(gm, EXT, "sheenRoughnessTexture") {
        texture_transform(
            nav,
            Some(rough),
            &format!("extensions/{EXT}/sheenRoughnessTexture/extensions"),
            p,
        );
    }
}

/// `socket.links[0]`: the node and output identifier linked into input `i` of `n` (no
/// reroute is skipped).
fn link_from<'a>(nav: &Nav<'a>, n: &Node, i: usize) -> Option<(&'a Node, &'a str)> {
    let s = n.inputs.get(i)?;
    let l = nav.cx.tree.link_into(n.uid, &s.identifier)?;
    Some((nav.cx.tree.node(l.from_node)?, l.from_socket.as_str()))
}

/// `output.links[0]`: the node and input identifier the first link from output `o` of `n`
/// goes to (`tree.links` order).
fn link_to<'a>(nav: &Nav<'a>, n: &Node, o: usize) -> Option<(&'a Node, &'a str)> {
    let id = &n.outputs.get(o)?.identifier;
    let l = nav
        .cx
        .tree
        .links
        .iter()
        .find(|l| l.from_node == n.uid && &l.from_socket == id)?;
    Some((nav.cx.tree.node(l.to_node)?, l.to_socket.as_str()))
}

/// The 2π `detect_anisotropy_nodes` compares the DIVIDE node's input with (a literal in the
/// add-on, within 1e-4).
#[allow(clippy::approx_constant)]
const ADDON_TAU: f64 = 6.283185;

fn is_math(n: &Node, op: &str) -> bool {
    n.bl_idname == "ShaderNodeMath" && n.prop_str("operation") == op
}

/// `detect_anisotropy_nodes` (`search_node_tree.py`): the importer's tangent-space set-up
/// (Tangent node in UV_MAP mode on `Tangent`; `Anisotropic` ← Math MULTIPLY ← Separate XYZ
/// `Z`; Separate XYZ `X` / `Y` → ARCTAN2 → ADD → DIVIDE by 2π → `Anisotropic Rotation`;
/// Separate XYZ ← Vector Math MULTIPLY_ADD (×(2, 2, 1), +(−1, −1, 0)) ← Image Texture). Returns
/// the MULTIPLY node (its `inputs[1]` is the strength), the ADD node (its `inputs[1]` is the
/// rotation) and the texture socket (the MULTIPLY_ADD's first input).
fn anisotropy_nodes<'a>(
    nav: &Nav<'a>,
    a: In<'a>,
    t: In<'a>,
) -> Option<(&'a Node, &'a Node, In<'a>)> {
    let (tangent, _) = nav.peek_back(t)?;
    if tangent.bl_idname != "ShaderNodeTangent" || tangent.prop_str("direction_type") != "UV_MAP" {
        return None;
    }
    let (mul, _) = link_from(nav, a.node, a.index)?;
    if !is_math(mul, "MULTIPLY") {
        return None;
    }
    let (sep, z) = link_from(nav, mul, 0)?;
    if sep.bl_idname != "ShaderNodeSeparateXYZ" || z != "Z" {
        return None;
    }
    let (atan, _) = link_to(nav, sep, 0)?;
    if !is_math(atan, "ARCTAN2")
        || link_from(nav, atan, 0)?.1 != "Y"
        || link_from(nav, atan, 1)?.1 != "X"
    {
        return None;
    }
    let (add, _) = link_to(nav, atan, 0)?;
    if !is_math(add, "ADD") {
        return None;
    }
    let (div, _) = link_to(nav, add, 0)?;
    if !is_math(div, "DIVIDE") {
        return None;
    }
    match div.inputs.get(1).map(|s| &s.default_value) {
        Some(SocketValue::Float(f)) if (f64::from(*f) - ADDON_TAU).abs() <= 0.0001 => {}
        _ => return None,
    }
    let (to, to_socket) = link_to(nav, div, 0)?;
    if to_socket != "Anisotropic Rotation" || to.bl_idname != "ShaderNodeBsdfPrincipled" {
        return None;
    }
    let (vm, _) = link_from(nav, sep, 0)?;
    let v3 = |i: usize| match vm.inputs.get(i).map(|s| &s.default_value) {
        Some(SocketValue::Vector(v)) => Some([v[0], v[1], v[2]]),
        _ => None,
    };
    if vm.bl_idname != "ShaderNodeVectorMath"
        || vm.prop_str("operation") != "MULTIPLY_ADD"
        || v3(1)? != [2.0, 2.0, 1.0]
        || v3(2)? != [-1.0, -1.0, 0.0]
    {
        return None;
    }
    let (tex, _) = link_from(nav, vm, 0)?;
    let tex_socket = In { node: vm, index: 0 };
    (tex.bl_idname == "ShaderNodeTexImage" && nav.has_image(tex_socket))
        .then_some((mul, add, tex_socket))
}

/// `export_anisotropy`: both sockets unlinked, the strength and the raw rotation socket (the
/// add-on samples it unconverted, unlike the static export); else the importer's texture
/// set-up ([`anisotropy_nodes`]: the MULTIPLY's and the ADD's second inputs, then the
/// texture's transform); else the grayscale-texture bake (WP 8.12: the texture transform, the
/// strength and the rotation factors).
fn anisotropy(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    const EXT: &str = "KHR_materials_anisotropy";
    let (Some(a), Some(r), Some(t)) = (
        nav.cx.get_socket("Anisotropic"),
        nav.cx.get_socket("Anisotropic Rotation"),
        nav.cx.get_socket("Tangent"),
    ) else {
        return;
    };
    let e = format!("/materials/XXX/extensions/{EXT}");
    let strength = PathInfo::new(&format!("{e}/anisotropyStrength"), 1);
    let rotation = PathInfo::new(&format!("{e}/anisotropyRotation"), 1);
    if !nav.is_linked(a) && !nav.is_linked(r) {
        p.put(Some(input_key(a.node, a.index)), strength);
        p.put(Some(input_key(r.node, r.index)), rotation);
        return;
    }
    // `detect_anisotropy_nodes` needs the rotation and the tangent linked.
    let nodes = (nav.is_linked(r) && nav.is_linked(t))
        .then(|| anisotropy_nodes(nav, a, t))
        .flatten();
    let Some((mul, add, tex)) = nodes else {
        // `export_anisotropy_from_grayscale_textures` (WP 8.12): the primary socket's
        // texture transform, then the factors of `Anisotropic` and `Anisotropic Rotation`.
        if ext_has(gm, EXT, "anisotropyTexture") {
            let primary = if nav.cx.texture_from(a).is_some() {
                a
            } else {
                r
            };
            texture_transform(
                nav,
                Some(primary),
                &format!("extensions/{EXT}/anisotropyTexture/extensions"),
                p,
            );
            p.put(nav.factor(a).and_then(|c| c.path), strength);
            p.put(nav.factor(r).and_then(|c| c.path), rotation);
        }
        return;
    };
    p.put(
        nav.constant(In {
            node: mul,
            index: 1,
        })
        .and_then(|c| c.path),
        strength,
    );
    p.put(
        nav.constant(In {
            node: add,
            index: 1,
        })
        .and_then(|c| c.path),
        rotation,
    );
    if ext_has(gm, EXT, "anisotropyTexture") {
        texture_transform(
            nav,
            Some(tex),
            &format!("extensions/{EXT}/anisotropyTexture/extensions"),
            p,
        );
    }
}

/// The node of the first link into the first input of `n` named `name` (`inputs[name]`,
/// enabled or not, `links[0].from_node`: no reroute is skipped).
fn first_link_from<'a>(nav: &Nav<'a>, n: &Node, name: &str) -> Option<&'a Node> {
    let s = n.inputs.iter().find(|s| s.name == name)?;
    let l = nav.cx.tree.link_into(n.uid, &s.identifier)?;
    nav.cx.tree.node(l.from_node)
}

/// The number of links from output `o` of `n`.
fn links_from(nav: &Nav, n: &Node, o: usize) -> usize {
    let id = &n.outputs[o].identifier;
    nav.cx
        .tree
        .links
        .iter()
        .filter(|l| l.from_node == n.uid && l.from_socket == *id)
        .count()
}

/// `detect_iridescence_thickness_texure`: the Value nodes holding the minimum and maximum
/// thickness of the texture set-up (Image Texture → Separate Color → the Factor of a FLOAT
/// Mix whose A is a Value node also linked into the glTF Material Output's minimum and whose B
/// is another Value node), and the texture socket (the Separate Color's input).
fn iridescence_texture<'a>(
    nav: &Nav<'a>,
    thickness: In<'a>,
    minimum: Option<In<'a>>,
) -> Option<(&'a Node, &'a Node, In<'a>)> {
    let mix = nav
        .cx
        .tree
        .link_into(thickness.node.uid, &thickness.socket().identifier)
        .and_then(|l| nav.cx.tree.node(l.from_node))?;
    if mix.bl_idname != "ShaderNodeMix" || mix.prop_str("data_type") != "FLOAT" {
        return None;
    }
    let sep = first_link_from(nav, mix, "Factor")?;
    if sep.bl_idname != "ShaderNodeSeparateColor" {
        return None;
    }
    let sep_in = sep.inputs.first()?;
    let tex = nav
        .cx
        .tree
        .link_into(sep.uid, &sep_in.identifier)
        .and_then(|l| nav.cx.tree.node(l.from_node))?;
    if tex.bl_idname != "ShaderNodeTexImage" {
        return None;
    }
    let min_node = first_link_from(nav, mix, "A")?;
    if min_node.bl_idname != "ShaderNodeValue" || links_from(nav, min_node, 0) != 2 {
        return None;
    }
    let minimum = minimum?;
    let from = nav
        .cx
        .tree
        .link_into(minimum.node.uid, &minimum.socket().identifier)?
        .from_node;
    if from != min_node.uid {
        return None;
    }
    let max_node = first_link_from(nav, mix, "B")?;
    (max_node.bl_idname == "ShaderNodeValue").then_some((
        min_node,
        max_node,
        In {
            node: sep,
            index: 0,
        },
    ))
}

/// `export_iridescence` (with the glTF Material Output's `Iridescence Factor`): the
/// `Thin Film IOR` always, the factor's path and its texture's transform, the
/// `Thin Film Thickness` (unlinked, or the maximum and minimum of the texture set-up and the
/// thickness texture's transform), else the thickness factor's path. A factor or thickness
/// without a path is registered under `None` by the add-on ([`Found::none_keys`]).
fn iridescence(nav: &Nav, gm: &GMaterial, p: &mut Found) {
    const EXT: &str = "KHR_materials_iridescence";
    let Some(thickness) = nav.cx.get_socket("Thin Film Thickness") else {
        return;
    };
    let Some(factor) = nav.cx.gltf_output_input("Iridescence Factor") else {
        return;
    };
    let minimum = nav.cx.gltf_output_input("Iridescence Thickness Minimum");
    let e = format!("/materials/XXX/extensions/{EXT}");
    if let Some(ior) = nav.cx.get_socket("Thin Film IOR") {
        p.put(
            Some(input_key(ior.node, ior.index)),
            PathInfo::new(&format!("{e}/iridescenceIor"), 1),
        );
    }
    p.put_or_none(
        nav.factor(factor).and_then(|c| c.path),
        PathInfo::new(&format!("{e}/iridescenceFactor"), 1),
        "iridescenceFactor",
    );
    if ext_has(gm, EXT, "iridescenceTexture") {
        texture_transform(
            nav,
            Some(factor),
            &format!("extensions/{EXT}/iridescenceTexture/extensions"),
            p,
        );
    }
    let maximum = PathInfo::new(&format!("{e}/iridescenceThicknessMaximum"), 1);
    if !nav.is_linked(thickness) {
        p.put(Some(input_key(thickness.node, thickness.index)), maximum);
        return;
    }
    if let Some((min_node, max_node, tex)) = iridescence_texture(nav, thickness, minimum) {
        p.put(Some(output_key(max_node, 0)), maximum);
        p.put(
            Some(output_key(min_node, 0)),
            PathInfo::new(&format!("{e}/iridescenceThicknessMinimum"), 1),
        );
        if ext_has(gm, EXT, "iridescenceThicknessTexture") {
            texture_transform(
                nav,
                Some(tex),
                &format!("extensions/{EXT}/iridescenceThicknessTexture/extensions"),
                p,
            );
        }
        return;
    }
    // `fac != 400.0` holds for a linked socket without a constant factor too (None).
    match nav.factor(thickness) {
        Some(c) if c.value == Some(400.0) => {}
        Some(c) => p.put_or_none(c.path, maximum, "iridescenceThicknessMaximum"),
        None => p.put_or_none(None, maximum, "iridescenceThicknessMaximum"),
    }
}

/// `export_dispersion` (with `KHR_materials_volume`): the glTF Material Output's
/// `Dispersion` socket itself (`get_const_from_default_value_socket`).
fn dispersion(nav: &Nav, p: &mut Found) {
    let Some(d) = nav.cx.gltf_output_input("Dispersion") else {
        return;
    };
    if d.socket().ty == SocketType::Value {
        p.put(
            Some(input_key(d.node, d.index)),
            PathInfo::new(
                "/materials/XXX/extensions/KHR_materials_dispersion/dispersion",
                1,
            ),
        );
    }
}

/// `gather_material_pbr_metallic_roughness`: the base colour texture's and the
/// metallic-roughness texture's transforms, the base colour and alpha, metallic, roughness;
/// then `__gather_alpha_cutoff`.
fn pbr(nav: &Nav, gm: &GMaterial, p: &mut Found, image_none: bool) {
    let base_color = "/materials/XXX/pbrMetallicRoughness/baseColorFactor";
    let pbr_tex = gm.pbr_metallic_roughness.as_ref();
    let alpha_socket = nav.cx.get_socket("Alpha");
    let get = |a: &str, b: &str, c: &str| {
        nav.cx
            .get_socket(a)
            .or_else(|| nav.cx.get_socket(b))
            .or_else(|| nav.cx.gltf_output_input(c))
    };
    if pbr_tex.is_some_and(|x| x.base_color_texture.is_some()) {
        // `__gather_base_color_texture`: the first of colour and alpha with an image.
        let primary = [get("Base Color", "BaseColor", "BaseColor"), alpha_socket]
            .into_iter()
            .flatten()
            .find(|&s| nav.has_image(s));
        texture_transform(nav, primary, BASE_COLOR_TEXTURE, p);
    }
    if pbr_tex.is_some_and(|x| x.metallic_roughness_texture.is_some()) {
        // `__gather_metallic_roughness_texture`: metallic when it has an image, else
        // roughness, else the glTF Material Output's `MetallicRoughness`.
        let metallic = nav.cx.get_socket("Metallic").filter(|&s| nav.has_image(s));
        let roughness = nav.cx.get_socket("Roughness").filter(|&s| nav.has_image(s));
        let primary = metallic
            .or(roughness)
            .or_else(|| nav.cx.gltf_output_input("MetallicRoughness"));
        texture_transform(nav, primary, METALLIC_ROUGHNESS_TEXTURE, p);
    }
    let (alpha_path, cutoff_path) = match alpha_socket {
        Some(a) => nav.alpha_info(a),
        None => (None, None),
    };
    let color_socket = get("Base Color", "BaseColor", "BaseColorFactor");
    let color_path = color_socket.and_then(|s| {
        if image_none {
            (s.socket().ty == SocketType::Rgba).then(|| input_key(s.node, s.index))
        } else {
            nav.factor_path(s)
        }
    });
    if color_path.is_some() {
        p.put(
            color_path.clone(),
            PathInfo {
                additional_path: alpha_path.clone(),
                ..PathInfo::new(base_color, 3)
            },
        );
    }
    if alpha_path.is_some() {
        p.put(
            alpha_path,
            PathInfo {
                additional_path: color_path,
                ..PathInfo::new(base_color, 1)
            },
        );
    }
    for (name, fallback, member) in [
        ("Metallic", "MetallicFactor", "metallicFactor"),
        ("Roughness", "RoughnessFactor", "roughnessFactor"),
    ] {
        if let Some(s) = nav
            .cx
            .get_socket(name)
            .or_else(|| nav.cx.gltf_output_input(fallback))
        {
            p.put(
                nav.factor(s).and_then(|c| c.path),
                PathInfo::new(&format!("/materials/XXX/pbrMetallicRoughness/{member}"), 1),
            );
        }
    }
    if cutoff_path.is_some() {
        p.put(cutoff_path, PathInfo::new("/materials/XXX/alphaCutoff", 1));
    }
}

/// The `KHR_texture_transform` slots of `gm` that a Mapping node feeds
/// ([`texture_transform`]'s `YYY`), as `__fix_json` sees them: a slot whose static transform
/// is the identity has an empty `KHR_texture_transform` object, kept when any pointer
/// channel animates `KHR_texture_transform`.
pub fn mapped_slots(main: &Main, mat: &Material, gm: &GMaterial, orm: bool) -> Vec<String> {
    let found = paths(main, mat, gm, false, orm);
    let mut out: Vec<String> = Vec::new();
    for info in found.paths.values() {
        for pointer in std::iter::once(&info.pointer).chain(&info.additional) {
            if let Some(slot) = pointer
                .strip_prefix("/materials/XXX/")
                .and_then(|s| s.split_once("/KHR_texture_transform/"))
                .map(|(slot, _)| slot.to_owned())
                && !out.contains(&slot)
            {
                out.push(slot);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::Value;

    /// The factory "Material" (Principled BSDF, nothing linked): the sockets every gatherer
    /// reads, in `gather_material`'s order (socket indices of Blender 5.2.4's Principled BSDF,
    /// verified with `path_from_id()`).
    #[test]
    fn factory_principled_paths_in_gather_order() {
        let (main, _) = frap_scene::startup::factory();
        let (_, mat) = main
            .materials
            .iter()
            .find(|(_, m)| m.header.name == "Material")
            .unwrap();
        let p = paths(&main, mat, &GMaterial::default(), false, false).paths;
        let got: Vec<(String, &str)> = p
            .iter()
            .map(|(k, v)| (k.clone(), v.pointer.as_str()))
            .collect();
        let pb =
            |i: usize| format!("node_tree.nodes[\"Principled BSDF\"].inputs[{i}].default_value");
        let x = "/materials/XXX";
        let e = "/materials/XXX/extensions";
        let want = vec![
            (pb(28), format!("{x}/emissiveFactor")),
            (
                pb(29),
                format!("{e}/KHR_materials_emissive_strength/emissiveStrength"),
            ),
            (
                pb(20),
                format!("{e}/KHR_materials_clearcoat/clearcoatFactor"),
            ),
            (
                pb(21),
                format!("{e}/KHR_materials_clearcoat/clearcoatRoughnessFactor"),
            ),
            (
                pb(19),
                format!("{e}/KHR_materials_transmission/transmissionFactor"),
            ),
            (pb(14), format!("{e}/KHR_materials_specular/specularFactor")),
            (
                pb(15),
                format!("{e}/KHR_materials_specular/specularColorFactor"),
            ),
            (
                pb(16),
                format!("{e}/KHR_materials_anisotropy/anisotropyStrength"),
            ),
            (
                pb(17),
                format!("{e}/KHR_materials_anisotropy/anisotropyRotation"),
            ),
            (pb(3), format!("{e}/KHR_materials_ior/ior")),
            (pb(0), format!("{x}/pbrMetallicRoughness/baseColorFactor")),
            (pb(4), format!("{x}/pbrMetallicRoughness/baseColorFactor")),
            (pb(1), format!("{x}/pbrMetallicRoughness/metallicFactor")),
            (pb(2), format!("{x}/pbrMetallicRoughness/roughnessFactor")),
        ];
        let want: Vec<(String, &str)> = want.iter().map(|(k, v)| (k.clone(), v.as_str())).collect();
        assert_eq!(got, want);
        assert_eq!(p[&pb(28)].strength_channel, Some(pb(29)));
        assert_eq!(p[&pb(29)].factor_channel, Some(pb(28)));
        assert_eq!(p[&pb(0)].additional_path, Some(pb(4)));
        assert_eq!(p[&pb(4)].additional_path, Some(pb(0)));
        assert_eq!(p[&pb(0)].length, 3);
        assert_eq!(p[&pb(4)].length, 1);
    }

    /// WP 7.7: `__gather_occlusion_strength` behind the glTF Material Output's `Occlusion`
    /// (a Mix whose white is `B`: reversed), `__gather_normal_scale` (the Normal Map's
    /// `Strength`), both after the extensions and before the base colour, only when the glTF
    /// material has the texture; an unlit material registers its colour only.
    #[test]
    fn normal_scale_occlusion_strength_and_unlit_paths() {
        use frap_nodes::Link;
        let (mut main, _) = frap_scene::startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let ma = main.materials.find("Material").unwrap();
        let mat = main.materials.get_mut(ma, &mut log).unwrap();
        let t = &mut mat.node_tree;
        let (gi, mi, ni, ti) = (
            t.new_node("ShaderNodeGroup").unwrap(),
            t.new_node("ShaderNodeMix").unwrap(),
            t.new_node("ShaderNodeNormalMap").unwrap(),
            t.new_node("ShaderNodeTexImage").unwrap(),
        );
        let g = frap_nodes::builtin_groups::gltf_material_output::new_group();
        t.nodes[gi].sync_group_sockets(Some(&g.interface));
        t.nodes[mi].set_prop("data_type", Value::Enum("RGBA".into()));
        t.nodes[mi].update_availability();
        let ids = |n: &Node, name: &str| {
            n.inputs
                .iter()
                .find(|s| s.name == name && s.enabled)
                .unwrap()
                .identifier
                .to_string()
        };
        let occ = ids(&t.nodes[gi], "Occlusion");
        let fac = t.nodes[mi].input_index("Factor").unwrap();
        t.nodes[mi].inputs[fac].default_value = SocketValue::Float(0.7);
        let b = t.nodes[mi]
            .inputs
            .iter()
            .position(|s| s.identifier == "B_Color")
            .unwrap();
        t.nodes[mi].inputs[b].default_value = SocketValue::Color([1.0; 4]);
        let bsdf = t
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap()
            .uid;
        let (gu, mu, nu, tu) = (
            t.nodes[gi].uid,
            t.nodes[mi].uid,
            t.nodes[ni].uid,
            t.nodes[ti].uid,
        );
        for (f, fs, to, ts) in [
            (tu, "Color".to_owned(), mu, "A_Color".to_owned()),
            (mu, "Result_Color".to_owned(), gu, occ),
            (tu, "Color".to_owned(), nu, "Color".to_owned()),
            (nu, "Normal".to_owned(), bsdf, "Normal".to_owned()),
        ] {
            t.links.push(Link {
                from_node: f,
                from_socket: fs.into(),
                to_node: to,
                to_socket: ts.into(),
            });
        }
        let mat = main.materials.get(ma).unwrap();
        let gm = GMaterial {
            normal_texture: Some(frap_gltf::json::NormalTextureInfo::default()),
            occlusion_texture: Some(frap_gltf::json::OcclusionTextureInfo::default()),
            ..GMaterial::default()
        };
        let p = paths(&main, mat, &gm, false, false).paths;
        let keys: Vec<&str> = p.values().map(|v| v.pointer.as_str()).collect();
        let at = |x: &str| keys.iter().position(|k| *k == x).unwrap();
        let ior = at("/materials/XXX/extensions/KHR_materials_ior/ior");
        let normal = at("/materials/XXX/normalTexture/scale");
        let occlusion = at("/materials/XXX/occlusionTexture/strength");
        let base = at("/materials/XXX/pbrMetallicRoughness/baseColorFactor");
        assert!(
            ior < normal && normal < occlusion && occlusion < base,
            "{keys:?}"
        );
        assert_eq!(
            p.get_index(normal).unwrap().0,
            "node_tree.nodes[\"Normal Map\"].inputs[0].default_value"
        );
        let (k, info) = p.get_index(occlusion).unwrap();
        assert_eq!(
            k,
            &format!("node_tree.nodes[\"Mix\"].inputs[{fac}].default_value")
        );
        assert!(info.reverse);
        // Without the textures in the glTF material: neither path.
        let p = paths(&main, mat, &GMaterial::default(), false, false).paths;
        assert!(
            !p.values()
                .any(|v| v.pointer.contains("normalTexture") || v.pointer.contains("occlusion"))
        );
        // An unlit glTF material: the colour only.
        let mut unlit = GMaterial::default();
        unlit.extensions.set(Some(
            frap_gltf::ext::khr_materials_unlit::KhrMaterialsUnlit::default(),
        ));
        let (mut main, _) = frap_scene::startup::factory();
        let ma = main.materials.find("Material").unwrap();
        let mat = main.materials.get_mut(ma, &mut log).unwrap();
        let t = &mut mat.node_tree;
        let bg = t.new_node("ShaderNodeBackground").unwrap();
        let bgu = t.nodes[bg].uid;
        let out = t
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeOutputMaterial")
            .unwrap()
            .uid;
        t.links.retain(|l| l.to_node != out);
        t.links.push(Link {
            from_node: bgu,
            from_socket: "Background".into(),
            to_node: out,
            to_socket: "Surface".into(),
        });
        let mat = main.materials.get(ma).unwrap();
        let p = paths(&main, mat, &unlit, false, false).paths;
        let got: Vec<(&str, &str)> = p
            .iter()
            .map(|(k, v)| (k.as_str(), v.pointer.as_str()))
            .collect();
        assert_eq!(
            got,
            [(
                "node_tree.nodes[\"Background\"].inputs[0].default_value",
                "/materials/XXX/pbrMetallicRoughness/baseColorFactor"
            )]
        );
    }

    /// `occlusionTexture/strength` with `reverse` is `1 - Factor` when sampled.
    #[test]
    fn reversed_occlusion_strength_is_one_minus_the_factor() {
        let mut paths = IndexMap::new();
        paths.insert(
            "k".to_owned(),
            PathInfo {
                reverse: true,
                ..PathInfo::new("/materials/XXX/occlusionTexture/strength", 1)
            },
        );
        let (main, _) = frap_scene::startup::factory();
        let asset = super::super::registry::Asset {
            id: main.materials.find("Material").unwrap().any(),
            index: 0,
            paths,
        };
        let mut data = super::super::sample::FrameData::new();
        data.insert("k".to_owned(), vec![0.25]);
        let v = super::super::sample::key_value(
            super::super::registry::DataKind::Materials,
            &asset,
            "k",
            &data,
            false,
        );
        assert_eq!(v, Some(vec![0.75]));
    }

    /// A key registered twice keeps its first pointer and collects the others as
    /// `additional` (an identical pointer is not added); `None` keys are recorded by member.
    #[test]
    fn merge_collects_additional_pointers_and_none_keys() {
        let mut p = Found::default();
        p.merge("k".into(), PathInfo::new("/materials/XXX/a/scale", 1));
        p.merge("k".into(), PathInfo::new("/materials/XXX/b/scale", 1));
        p.merge("k".into(), PathInfo::new("/materials/XXX/a/scale", 1));
        p.merge("k".into(), PathInfo::new("/materials/XXX/c/scale", 1));
        assert_eq!(p.paths["k"].pointer, "/materials/XXX/a/scale");
        assert_eq!(
            p.paths["k"].additional,
            ["/materials/XXX/b/scale", "/materials/XXX/c/scale"]
        );
        assert_eq!(
            p.paths["k"].resolved_additional(4),
            ["/materials/4/b/scale", "/materials/4/c/scale"]
        );
        p.put_or_none(None, PathInfo::default(), "iridescenceFactor");
        p.put_or_none(None, PathInfo::default(), "iridescenceFactor");
        assert_eq!(p.none_keys, ["iridescenceFactor"]);
    }

    /// An `Iridescence Factor` fed by an image through Separate Color has no constant factor:
    /// the add-on registers it under `None` (its sampler then fails), frap records the member.
    #[test]
    fn iridescence_factor_from_a_texture_has_no_path() {
        use frap_nodes::Link;
        let (mut main, _) = frap_scene::startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let ma = main.materials.find("Material").unwrap();
        let mat = main.materials.get_mut(ma, &mut log).unwrap();
        let t = &mut mat.node_tree;
        let (gi, si, ti) = (
            t.new_node("ShaderNodeGroup").unwrap(),
            t.new_node("ShaderNodeSeparateColor").unwrap(),
            t.new_node("ShaderNodeTexImage").unwrap(),
        );
        let g = frap_nodes::builtin_groups::gltf_material_output::new_group();
        t.nodes[gi].sync_group_sockets(Some(&g.interface));
        let factor = t.nodes[gi]
            .inputs
            .iter()
            .find(|s| s.name == "Iridescence Factor")
            .unwrap()
            .identifier
            .to_string();
        let (gu, su, tu) = (t.nodes[gi].uid, t.nodes[si].uid, t.nodes[ti].uid);
        let sep_in = t.nodes[si].inputs[0].identifier.to_string();
        t.links.push(Link {
            from_node: tu,
            from_socket: "Color".into(),
            to_node: su,
            to_socket: sep_in.into(),
        });
        t.links.push(Link {
            from_node: su,
            from_socket: "Red".into(),
            to_node: gu,
            to_socket: factor.into(),
        });
        let mat = main.materials.get(ma).unwrap();
        let found = paths(&main, mat, &GMaterial::default(), false, false);
        assert_eq!(found.none_keys, ["iridescenceFactor"]);
        assert!(found.paths.values().any(|v| {
            v.pointer
                .ends_with("KHR_materials_iridescence/iridescenceIor")
        }));
    }

    #[test]
    fn keys_escape_node_names() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
    }
}
