// SPDX-License-Identifier: GPL-3.0-or-later
//! The node-tree property a material pointer animates, in the tree the importer built
//! (`BlenderPointerAnim.do_channel`'s material branches, io_scene_gltf2 5.2.41
//! `blender/imp/animation_pointer.py`, Apache-2.0, The glTF-Blender-IO authors): the socket the
//! factor lives in, or, when the importer multiplied a texture by the factor, the factor input
//! of the Mix (`inputs[7]`, `B_Color`) or Math (`inputs[1]`) node in front of it.
//!
//! `KHR_texture_transform` channels (WP 7.7, [`texture_transform`]) animate the Mapping node
//! the importer put in front of the texture: `offset` its `Location` (`inputs[1]`, converted
//! by the caller), `scale` its `Scale` (`inputs[3]`); `rotation` is never dispatched by the
//! add-on. An unlit material's base colour animates the colour socket of
//! `detect_shadeless_material` and its alpha the Mix Shader's factor in front of the
//! Transparent BSDF.

use frap_gltf::Extension;
use frap_material::LowerCx;
use frap_material::graph::In;
use frap_nodes::{Node, NodeTree};
use frap_scene::Main;
use frap_scene::id::Material;

/// The F-curve a material channel becomes: path (relative to the node tree), number of
/// components, group, and the value conversion.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MatTarget {
    /// `data_path`
    pub path: String,
    /// Components written (`num_components`).
    pub components: usize,
    /// `group_name` (empty: no group).
    pub group: &'static str,
    /// The conversion of each value.
    pub convert: Convert,
}

/// Value conversions of `do_channel`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Convert {
    /// As is.
    None,
    /// `1 / v` (`attenuationDistance` → `Density`).
    Inverse,
    /// `v / 2` (`specularFactor` → `Specular IOR Level`).
    Half,
}

/// `socket.path_from_id() + ".default_value"`.
fn in_path(node: &Node, i: usize) -> String {
    format!(
        "nodes[\"{}\"].inputs[{i}].default_value",
        node.name.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

fn out_path(node: &Node, o: usize) -> String {
    format!(
        "nodes[\"{}\"].outputs[{o}].default_value",
        node.name.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// `socket.links[0].from_node` of an input.
fn from_node<'t>(tree: &'t NodeTree, i: In<'t>) -> Option<&'t Node> {
    let l = tree.link_into(i.node.uid, &i.socket().identifier)?;
    tree.node(l.from_node)
}

/// The importer's node-type checks (`node.type`).
fn is_type(n: &Node, ty: &str) -> bool {
    let id = match ty {
        "MIX" => "ShaderNodeMix",
        "MATH" => "ShaderNodeMath",
        "NORMAL_MAP" => "ShaderNodeNormalMap",
        "VALUE" => "ShaderNodeValue",
        _ => return false,
    };
    n.bl_idname == id
}

fn target(path: String, components: usize, group: &'static str) -> Option<MatTarget> {
    Some(MatTarget {
        path,
        components,
        group,
        convert: Convert::None,
    })
}

/// The socket itself when unlinked, else input `index` of the `ty` node linked into it
/// (`None`, the add-on's "Error, something is wrong…", when another node is there).
fn factor_input(
    tree: &NodeTree,
    s: In,
    ty: &str,
    index: usize,
    components: usize,
    group_unlinked: &'static str,
) -> Option<MatTarget> {
    match from_node(tree, s) {
        Some(n) if is_type(n, ty) => target(in_path(n, index), components, "Material"),
        Some(_) => None,
        None => target(in_path(s.node, s.index), components, group_unlinked),
    }
}

/// The target of `pointer` (split on `/`) in material `mat`; `is_unlit` for a
/// `KHR_materials_unlit` material. `None` when the add-on finds nothing ("Unsupported
/// animation target").
pub(crate) fn resolve(
    main: &Main,
    mat: &Material,
    tab: &[&str],
    is_unlit: bool,
) -> Option<MatTarget> {
    let cx = LowerCx::new(&mat.node_tree, mat, main);
    let tree = &mat.node_tree;
    let get = |name: &str| cx.get_socket(name);
    let gltf = |name: &str| cx.gltf_output_input(name);
    let n = tab.len();
    match (
        n,
        tab.get(3).copied(),
        tab.get(4).copied(),
        tab.get(5).copied(),
    ) {
        (4, Some("emissiveFactor"), ..) => {
            let s = get("Emissive")?;
            match from_node(tree, s) {
                Some(m) if is_type(m, "MIX") => target(in_path(m, 7), 3, "Material"),
                Some(_) => None,
                None => target(in_path(s.node, s.index), 3, ""),
            }
        }
        (4, Some("alphaCutoff"), ..) => {
            let s = get("Alpha")?;
            let sub = from_node(tree, s)?;
            if !is_type(sub, "MATH") {
                return None;
            }
            let less = from_node(
                tree,
                In {
                    node: sub,
                    index: 1,
                },
            )?;
            is_type(less, "MATH")
                .then(|| in_path(less, 1))
                .and_then(|p| target(p, 1, "Material"))
        }
        (5, Some("normalTexture"), Some("scale"), _) => {
            let s = get("Normal")?;
            let nm = from_node(tree, s)?;
            is_type(nm, "NORMAL_MAP")
                .then(|| in_path(nm, 0))
                .and_then(|p| target(p, 1, "Material"))
        }
        (5, Some("occlusionTexture"), Some("strength"), _) => {
            let s = get("Occlusion").or_else(|| gltf("Occlusion"))?;
            factor_input(tree, s, "MIX", 0, 1, "Material")
        }
        (5, Some("pbrMetallicRoughness"), Some(member), _) => match member {
            "baseColorFactor" => {
                if is_unlit {
                    // `detect_shadeless_material(...)['rgb_socket']`, linked or not.
                    let (rgb, _) = frap_material::patterns::unlit::shadeless_sockets(&cx)?;
                    return target(in_path(rgb.node, rgb.index), 3, "Material");
                }
                let s = get("Base Color")?;
                factor_input(tree, s, "MIX", 7, 3, "")
            }
            "roughnessFactor" => factor_input(tree, get("Roughness")?, "MATH", 1, 1, "Material"),
            "metallicFactor" => factor_input(tree, get("Metallic")?, "MATH", 1, 1, "Material"),
            _ => None,
        },
        (6, Some("extensions"), Some(ext), Some(member)) => ext_target(&cx, ext, member),
        (
            7,
            Some("extensions"),
            Some("KHR_materials_clearcoat"),
            Some("clearcoatNormalTexture"),
        ) if tab[6] == "scale" => {
            // `from_socket(Coat Normal, FilterByType(ShaderNodeNormalMap))`: the Normal Map
            // node feeding it.
            let s = get("Coat Normal")?;
            let nm = cx.node_from(s, "ShaderNodeNormalMap")?;
            let i = nm.input_index("Strength")?;
            target(in_path(nm, i), 1, "Material")
        }
        _ => None,
    }
}

/// The alpha F-curve of a base colour channel: the Alpha socket (an unlit material's: the
/// Mix Shader's factor of `detect_shadeless_material`), or the factor input of the Math node
/// multiplying it. `Ok(None)`: another node is linked there (the add-on prints an error and
/// writes no alpha); `Err(())`: there is no alpha socket (the add-on raises: an opaque unlit
/// material has none).
pub(crate) fn alpha_path(
    main: &Main,
    mat: &Material,
    is_unlit: bool,
) -> Result<Option<String>, ()> {
    let cx = LowerCx::new(&mat.node_tree, mat, main);
    let s = if is_unlit {
        frap_material::patterns::unlit::shadeless_sockets(&cx).and_then(|(_, a)| a)
    } else {
        cx.get_socket("Alpha")
    };
    let s = s.ok_or(())?;
    Ok(match from_node(&mat.node_tree, s) {
        Some(m) if is_type(m, "MATH") => Some(in_path(m, 1)),
        Some(_) => None,
        None => Some(in_path(s.node, s.index)),
    })
}

/// The textures whose `KHR_texture_transform` animations `BlenderAnimation.anim` imports, in
/// its order: `(owner, member)`, the owner being the material (`""`), its
/// `pbrMetallicRoughness` or one of its extensions.
pub(crate) const TEXTURES: [(&str, &str); 17] = [
    ("", "emissiveTexture"),
    ("", "normalTexture"),
    ("", "occlusionTexture"),
    ("pbrMetallicRoughness", "baseColorTexture"),
    ("pbrMetallicRoughness", "metallicRoughnessTexture"),
    ("KHR_materials_volume", "thicknessTexture"),
    ("KHR_materials_transmission", "transmissionTexture"),
    ("KHR_materials_specular", "specularTexture"),
    ("KHR_materials_specular", "specularColorTexture"),
    ("KHR_materials_sheen", "sheenColorTexture"),
    ("KHR_materials_sheen", "sheenRoughnessTexture"),
    ("KHR_materials_clearcoat", "clearcoatTexture"),
    ("KHR_materials_clearcoat", "clearcoatRoughnessTexture"),
    ("KHR_materials_clearcoat", "clearcoatNormalTexture"),
    ("KHR_materials_anisotropy", "anisotropyTexture"),
    ("KHR_materials_iridescence", "iridescenceTexture"),
    ("KHR_materials_iridescence", "iridescenceThicknessTexture"),
];

/// The `KHR_texture_transform` object of texture `i` of [`TEXTURES`] in material `gm` (`None`
/// when the texture or its transform is missing: the add-on's dispatch fails then).
pub(crate) fn texture_transform_of(
    gm: &frap_gltf::json::Material,
    i: usize,
) -> Option<serde_json::Value> {
    const TT: &str = frap_gltf::ext::khr_texture_transform::KhrTextureTransform::NAME;
    let (owner, member) = *TEXTURES.get(i)?;
    let pbr = gm.pbr_metallic_roughness.as_ref();
    let exts = match (owner, member) {
        ("", "emissiveTexture") => &gm.emissive_texture.as_ref()?.extensions,
        ("", "normalTexture") => &gm.normal_texture.as_ref()?.extensions,
        ("", "occlusionTexture") => &gm.occlusion_texture.as_ref()?.extensions,
        ("pbrMetallicRoughness", "baseColorTexture") => {
            &pbr?.base_color_texture.as_ref()?.extensions
        }
        ("pbrMetallicRoughness", _) => &pbr?.metallic_roughness_texture.as_ref()?.extensions,
        (ext, member) => {
            return gm
                .extensions
                .get_raw(ext)?
                .get(member)?
                .get("extensions")?
                .get(TT)
                .cloned();
        }
    };
    exts.get_raw(TT).cloned()
}

/// `do_channel`'s `KHR_texture_transform` branch for texture `i` of [`TEXTURES`]: the Mapping
/// node in front of the texture behind the socket the add-on picks for it (the unlit colour
/// socket for an unlit base colour), its `Location` (`inputs[1]`) for `offset`, its `Scale`
/// (`inputs[3]`) for `scale`, two components each. `None` when there is no such texture or
/// Mapping node (the add-on warns "Unsupported animation target", or fails on a missing
/// Mapping node).
pub(crate) fn texture_transform(
    main: &Main,
    mat: &Material,
    i: usize,
    member: &str,
    is_unlit: bool,
) -> Option<MatTarget> {
    let cx = LowerCx::new(&mat.node_tree, mat, main);
    let get = |name: &str| cx.get_socket(name);
    let gltf = |name: &str| cx.gltf_output_input(name);
    let socket = match TEXTURES.get(i)?.1 {
        "baseColorTexture" if is_unlit => {
            frap_material::patterns::unlit::shadeless_sockets(&cx).map(|(rgb, _)| rgb)
        }
        "baseColorTexture" => get("Base Color"),
        "emissiveTexture" => get("Emission Color"),
        "normalTexture" => get("Normal"),
        "occlusionTexture" => get("Occlusion").or_else(|| gltf("Occlusion")),
        "metallicRoughnessTexture" => get("Roughness"),
        "specularTexture" => get("Specular IOR Level"),
        "specularColorTexture" => get("Specular Tint"),
        "sheenColorTexture" => get("Sheen Tint"),
        "sheenRoughnessTexture" => get("Sheen Roughness"),
        "clearcoatTexture" => get("Coat Weight"),
        "clearcoatRoughnessTexture" => get("Coat Roughness"),
        "clearcoatNormalTexture" => get("Coat Normal"),
        "thicknessTexture" => gltf("Thickness"),
        "transmissionTexture" => get("Transmission Weight"),
        "iridescenceTexture" => gltf("Iridescence Factor"),
        "iridescenceThicknessTexture" => get("Thin Film Thickness"),
        "anisotropyTexture" => get("Anisotropic"),
        _ => None,
    }?;
    let tex = cx.texture_from(socket)?.node;
    let wrapping = frap_material::patterns::sampler::detect_manual_uv_wrapping(&cx, tex);
    let src = match &wrapping {
        Some(w) => cx.src_in(w.next),
        None => cx.src(tex, "Vector"),
    };
    let frap_material::graph::Src::Linked(mapping, _) = src else {
        return None;
    };
    if mapping.bl_idname != "ShaderNodeMapping" {
        return None;
    }
    let index = match member {
        "offset" => 1,
        "scale" => 3,
        _ => return None,
    };
    target(in_path(mapping, index), 2, "Material")
}

/// The material-extension branches of `do_channel`.
fn ext_target(cx: &LowerCx, ext: &str, member: &str) -> Option<MatTarget> {
    let tree = cx.tree;
    let get = |name: &str| cx.get_socket(name);
    let gltf = |name: &str| cx.gltf_output_input(name);
    let absorption = |name: &str| {
        cx.linked_node_of_type("ShaderNodeVolumeAbsorption")
            .and_then(|n| In::named(n, name))
    };
    let mut t = match (ext, member) {
        ("KHR_materials_emissive_strength", "emissiveStrength") => {
            let s = get("Emission Strength")?;
            target(in_path(s.node, s.index), 1, "Material")?
        }
        ("KHR_materials_volume", "thicknessFactor") => {
            factor_input(tree, gltf("Thickness")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_volume", "attenuationDistance") => {
            let s = absorption("Density")?;
            let mut t = target(in_path(s.node, s.index), 1, "Material")?;
            t.convert = Convert::Inverse;
            t
        }
        ("KHR_materials_volume", "attenuationColor") => {
            let s = absorption("Color")?;
            target(in_path(s.node, s.index), 3, "Material")?
        }
        ("KHR_materials_ior", "ior") => {
            let s = get("IOR")?;
            target(in_path(s.node, s.index), 1, "Material")?
        }
        ("KHR_materials_transmission", "transmissionFactor") => {
            factor_input(tree, get("Transmission Weight")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_clearcoat", "clearcoatFactor") => {
            factor_input(tree, get("Coat Weight")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_clearcoat", "clearcoatRoughnessFactor") => {
            factor_input(tree, get("Coat Roughness")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_sheen", "sheenColorFactor") => {
            factor_input(tree, get("Sheen Tint")?, "MIX", 7, 3, "Material")?
        }
        ("KHR_materials_sheen", "sheenRoughnessFactor") => {
            factor_input(tree, get("Sheen Roughness")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_specular", "specularFactor") => {
            let mut t = factor_input(tree, get("Specular IOR Level")?, "MATH", 1, 1, "Material")?;
            t.convert = Convert::Half;
            t
        }
        ("KHR_materials_specular", "specularColorFactor") => {
            factor_input(tree, get("Specular Tint")?, "MIX", 7, 3, "Material")?
        }
        ("KHR_materials_anisotropy", "anisotropyStrength") => {
            factor_input(tree, get("Anisotropic")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_anisotropy", "anisotropyRotation") => {
            let s = get("Anisotropic Rotation")?;
            match from_node(tree, s) {
                Some(m) if is_type(m, "MATH") => {
                    let r = from_node(tree, In { node: m, index: 0 })?;
                    is_type(r, "MATH")
                        .then(|| in_path(r, 1))
                        .and_then(|p| target(p, 1, "Material"))?
                }
                Some(_) => return None,
                None => target(in_path(s.node, s.index), 1, "Material")?,
            }
        }
        ("KHR_materials_dispersion", "dispersion") => {
            let s = gltf("Dispersion")?;
            if from_node(tree, s).is_some() {
                return None;
            }
            target(in_path(s.node, s.index), 1, "Material")?
        }
        ("KHR_materials_iridescence", "iridescenceFactor") => {
            factor_input(tree, gltf("Iridescence Factor")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_iridescence", "iridescenceIor") => {
            factor_input(tree, get("Thin Film IOR")?, "MATH", 1, 1, "Material")?
        }
        ("KHR_materials_iridescence", "iridescenceThicknessMinimum") => {
            let s = gltf("Iridescence Thickness Minimum")?;
            match from_node(tree, s) {
                Some(v) if is_type(v, "VALUE") => target(out_path(v, 0), 1, "Material")?,
                Some(_) => return None,
                None => target(in_path(s.node, s.index), 1, "Material")?,
            }
        }
        ("KHR_materials_iridescence", "iridescenceThicknessMaximum") => {
            let s = get("Thin Film Thickness")?;
            match from_node(tree, s) {
                Some(m) if is_type(m, "MIX") => {
                    let v = from_node(tree, In { node: m, index: 3 })?;
                    is_type(v, "VALUE")
                        .then(|| out_path(v, 0))
                        .and_then(|p| target(p, 1, "Material"))?
                }
                Some(_) => return None,
                None => target(in_path(s.node, s.index), 1, "Material")?,
            }
        }
        _ => return None,
    };
    if t.group.is_empty() {
        t.group = "Material";
    }
    Some(t)
}
