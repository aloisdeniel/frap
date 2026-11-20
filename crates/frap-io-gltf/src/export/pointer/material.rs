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
//! The material is read from its original node tree (an animated tree is never inlined), so
//! no constant folding applies here. Not registered (no channel is exported for them; recorded
//! in docs/deviations.md §2): the anisotropy node setup with textures, `KHR_materials_iridescence`,
//! `KHR_materials_dispersion`, the `KHR_texture_transform` of textures, `normalTexture.scale`,
//! `occlusionTexture.strength` and unlit materials.

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

/// `current_paths` of `gather_material` for `mat` (empty when the material has nothing
/// animatable the exporter reads).
pub fn paths(main: &Main, mat: &Material) -> IndexMap<String, PathInfo> {
    let nav = Nav {
        cx: LowerCx::new(&mat.node_tree, mat, main),
    };
    let mut out: IndexMap<String, PathInfo> = IndexMap::new();
    let mut put = |key: Option<String>, info: PathInfo| {
        if let Some(k) = key {
            out.insert(k, info);
        }
    };
    let image_none = false;
    emission(&nav, &mut put, image_none);
    clearcoat(&nav, &mut put);
    // transmission
    if let Some(s) = nav.cx.get_socket("Transmission Weight") {
        put(
            nav.ext_factor(s),
            PathInfo::new(
                "/materials/XXX/extensions/KHR_materials_transmission/transmissionFactor",
                1,
            ),
        );
    }
    volume(&nav, &mut put);
    specular(&nav, &mut put);
    sheen(&nav, &mut put);
    anisotropy(&nav, &mut put);
    // ior: unlinked, and a transmission/specular extension object exists (always for a
    // material with these sockets; `export_transmission` creates one unconditionally).
    if let Some(s) = nav.cx.get_socket("IOR")
        && !nav.is_linked(s)
    {
        put(
            Some(input_key(s.node, s.index)),
            PathInfo::new("/materials/XXX/extensions/KHR_materials_ior/ior", 1),
        );
    }
    pbr(&nav, &mut put);
    out
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

type Put<'p> = dyn FnMut(Option<String>, PathInfo) + 'p;

/// `export_emission_factor`.
fn emission(nav: &Nav, put: &mut Put, image_none: bool) {
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
        put(
            path.clone(),
            PathInfo {
                strength_channel: strength_path.clone(),
                ..PathInfo::new("/materials/XXX/emissiveFactor", 3)
            },
        );
    }
    if strength_path.is_some() {
        put(
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

/// `export_clearcoat`.
fn clearcoat(nav: &Nav, put: &mut Put) {
    for (name, member) in [
        ("Coat Weight", "clearcoatFactor"),
        ("Coat Roughness", "clearcoatRoughnessFactor"),
    ] {
        if let Some(s) = nav.cx.get_socket(name) {
            put(
                nav.ext_factor(s),
                PathInfo::new(
                    &format!("/materials/XXX/extensions/KHR_materials_clearcoat/{member}"),
                    1,
                ),
            );
        }
    }
}

/// `export_volume`.
fn volume(nav: &Nav, put: &mut Put) {
    let Some(thickness) = nav.cx.gltf_output_input("Thickness") else {
        return;
    };
    let absorption = nav.cx.linked_node_of_type("ShaderNodeVolumeAbsorption");
    let va = |name: &str| absorption.and_then(|n| In::named(n, name));
    if let Some(c) = va("Color")
        && c.socket().ty == SocketType::Rgba
    {
        put(
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
        put(
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
        put(Some(input_key(thickness.node, thickness.index)), info);
    } else if nav.has_image(thickness) {
        put(nav.factor(thickness).and_then(|c| c.path), info);
    }
}

/// `export_specular`.
fn specular(nav: &Nav, put: &mut Put) {
    let (Some(spec), Some(tint)) = (
        nav.cx.get_socket("Specular IOR Level"),
        nav.cx.get_socket("Specular Tint"),
    ) else {
        return;
    };
    let path = |s: In| {
        if nav.is_linked(s) {
            nav.factor(s).and_then(|c| c.path)
        } else {
            Some(input_key(s.node, s.index))
        }
    };
    put(
        path(spec),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_specular/specularFactor",
            1,
        ),
    );
    put(
        path(tint),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_specular/specularColorFactor",
            3,
        ),
    );
}

/// `export_sheen`.
fn sheen(nav: &Nav, put: &mut Put) {
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
    let path = |s: In| {
        if nav.is_linked(s) {
            nav.factor(s).and_then(|c| c.path)
        } else {
            Some(input_key(s.node, s.index))
        }
    };
    put(
        path(tint),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_sheen/sheenColorFactor",
            3,
        ),
    );
    put(
        path(rough),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_sheen/sheenRoughnessFactor",
            1,
        ),
    );
}

/// `export_anisotropy` (unlinked sockets only: the strength and the raw rotation socket;
/// the node setup with textures is not registered).
fn anisotropy(nav: &Nav, put: &mut Put) {
    let (Some(a), Some(r), Some(_)) = (
        nav.cx.get_socket("Anisotropic"),
        nav.cx.get_socket("Anisotropic Rotation"),
        nav.cx.get_socket("Tangent"),
    ) else {
        return;
    };
    if nav.is_linked(a) || nav.is_linked(r) {
        return;
    }
    put(
        Some(input_key(a.node, a.index)),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_anisotropy/anisotropyStrength",
            1,
        ),
    );
    put(
        Some(input_key(r.node, r.index)),
        PathInfo::new(
            "/materials/XXX/extensions/KHR_materials_anisotropy/anisotropyRotation",
            1,
        ),
    );
}

/// `gather_material_pbr_metallic_roughness` (base colour and alpha, metallic, roughness) and
/// `__gather_alpha_cutoff`.
fn pbr(nav: &Nav, put: &mut Put) {
    let base_color = "/materials/XXX/pbrMetallicRoughness/baseColorFactor";
    let (alpha_path, cutoff_path) = match nav.cx.get_socket("Alpha") {
        Some(a) => nav.alpha_info(a),
        None => (None, None),
    };
    let color_socket = nav
        .cx
        .get_socket("Base Color")
        .or_else(|| nav.cx.get_socket("BaseColor"))
        .or_else(|| nav.cx.gltf_output_input("BaseColorFactor"));
    let color_path = color_socket.and_then(|s| nav.factor_path(s));
    if color_path.is_some() {
        put(
            color_path.clone(),
            PathInfo {
                additional_path: alpha_path.clone(),
                ..PathInfo::new(base_color, 3)
            },
        );
    }
    if alpha_path.is_some() {
        put(
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
            put(
                nav.factor(s).and_then(|c| c.path),
                PathInfo::new(&format!("/materials/XXX/pbrMetallicRoughness/{member}"), 1),
            );
        }
    }
    if cutoff_path.is_some() {
        put(cutoff_path, PathInfo::new("/materials/XXX/alphaCutoff", 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let p = paths(&main, mat);
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

    #[test]
    fn keys_escape_node_names() {
        assert_eq!(escape("a\"b\\c"), "a\\\"b\\\\c");
    }
}
