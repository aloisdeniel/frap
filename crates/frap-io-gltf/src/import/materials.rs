// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF materials → `bpy.data.materials`: the datablock and its non-node properties
//! (`use_backface_culling`, `surface_render_method`, viewport `diffuse_color`), a
//! [`LoweredMaterial`] read from the glTF material (textures resolved to imported images), and
//! the node graph built from it by `frap_material::raise` (WP 1.11, §16.2). Materials without
//! a glTF material but with `COLOR_0` get the add-on's "DefaultMaterial" graph.
//! `KHR_materials_unlit` materials are raised as the importer's unlit graph (`unlit.rs`), an
//! `occlusionTexture` into the "glTF Material Output" group node, whose `node_tree` this file
//! links to `bpy.data.node_groups` (WP 3.5).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/material.py` (`BlenderMaterial.create`,
//! `set_double_sided`, `set_eevee_surface_render_method`, `set_viewport_color`) and the glTF
//! reading half of `pbrMetallicRoughness.py`/`texture.py` (`set_filtering`, wrap modes,
//! KHR_texture_transform, `is_data` images), Apache-2.0, The glTF-Blender-IO authors.

use frap_gltf::ext::khr_materials_emissive_strength::KhrMaterialsEmissiveStrength;
use frap_gltf::ext::khr_texture_transform::KhrTextureTransform;
use frap_gltf::json::{AlphaMode as GAlphaMode, Extensions, MagFilter, MinFilter, WrapMode};
use frap_material::{
    AlphaMode, Channel, ChannelSel, Filter, LoweredMaterial, MATERIAL_PATTERNS, MaterialExts,
    NameAlloc, NormalMap, Sampler, TexRef, TexTransform, UvSource, VertexColorUse, Wrap,
};
use frap_nodes::generated::{NodePropDefault, NodeTypeDef, SocketDefault, node_type};
use frap_nodes::{Link, Node, NodeTree, PropBag, Socket, SocketValue};
use frap_rna::{IdRef, MaterialId, PropKind, Value};
use frap_scene::id::Material;
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::{Gltf, ImportError, extras, images};

/// Name of the colour attribute the importer creates for `COLOR_0`.
pub(crate) const COLOR_0_ATTRIBUTE: &str = "Color";

/// `BlenderMaterial.create(gltf, material_idx, vertex_color)`; `None` = "DefaultMaterial".
pub(crate) fn create(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    material_idx: Option<usize>,
    vertex_color: bool,
) -> Result<IdRef<MaterialId>, ImportError> {
    let Some(idx) = material_idx else {
        let mut mat = Material::new("");
        default_material_tree(&mut mat.node_tree, vertex_color);
        return Ok(main.materials.insert("DefaultMaterial", mat, log));
    };
    if let Some(&m) = cx.materials.get(&(idx, vertex_color)) {
        return Ok(m);
    }
    let pymat = cx
        .doc
        .materials
        .get(idx)
        .ok_or_else(|| ImportError::Data(format!("material {idx} does not exist")))?
        .clone();
    let name = pymat
        .name
        .clone()
        .unwrap_or_else(|| format!("Material_{idx}"));
    let mut mat = Material::new("");
    extras::set_extras(&mut mat.header, pymat.extras.as_ref(), &[]);
    // set_double_sided
    mat.use_backface_culling = !pymat.double_sided.unwrap_or(false);
    // set_eevee_surface_render_method
    let alpha_mode = pymat.alpha_mode.unwrap_or(GAlphaMode::Opaque);
    if alpha_mode == GAlphaMode::Blend {
        mat.surface_render_method = "BLENDED".into();
        mat.blend_method = "BLEND".into();
    } else {
        mat.surface_render_method = "DITHERED".into();
        mat.blend_method = "HASHED".into();
    }
    // set_viewport_color
    if !vertex_color
        && !pymat
            .extensions
            .contains("KHR_materials_pbrSpecularGlossiness")
        && let Some(pbr) = &pymat.pbr_metallic_roughness
        && pbr.base_color_texture.is_none()
    {
        let c = pbr.base_color_factor.unwrap_or([1.0; 4]);
        mat.diffuse_color = c.map(|x| x as f32);
    }
    let lm = lowered(cx, main, log, &pymat, vertex_color)?;
    // `raise` runs the MATERIAL_PATTERNS entries (WP 1.11); with none registered there is
    // nothing to raise and the tree stays empty, like `mat.node_tree.nodes.clear()`.
    if !MATERIAL_PATTERNS.is_empty() {
        let mut names = NameAlloc::default();
        frap_material::raise(&lm, &mut mat, &mut names);
        link_settings_group(main, log, &mut mat.node_tree);
    }
    let r = main.materials.insert(&name, mat, log);
    cx.materials.insert((idx, vertex_color), r);
    Ok(r)
}

/// `make_settings_node` / `get_settings_group`: `raise` builds the "glTF Material Output"
/// group node with the group's inputs but cannot reach `bpy.data`; its `node_tree` is set
/// here to `bpy.data.node_groups['glTF Material Output']`, created by
/// `create_settings_group` when it does not exist.
fn link_settings_group(main: &mut Main, log: &mut ChangeLog, tree: &mut NodeTree) {
    use frap_nodes::builtin_groups::gltf_material_output as gmo;
    let pending: Vec<usize> = tree
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| {
            frap_nodes::is_group_node(n.bl_idname)
                && n.group_tree().is_none()
                && n.input("Occlusion").is_some()
        })
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        return;
    }
    let group = match main.node_groups.find(gmo::NAME) {
        Some(g) => g,
        None => main.node_groups.insert(gmo::NAME, gmo::new_group(), log),
    };
    let Some(iface) = main.node_groups.get(group).map(|g| g.interface.clone()) else {
        return;
    };
    for i in pending {
        tree.nodes[i]
            .props
            .values
            .insert("node_tree", Value::Id(Some(group.any())));
        tree.sync_group_node(i, Some(&iface));
    }
}

/// `import_unused_materials`: materials (and images) no primitive used. The add-on gives them
/// a fake user, which frap's orphan rule (§5.5) does not need: `.frap` keeps every ID that a
/// user can reach by name until it is removed.
pub(crate) fn create_unused(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
) -> Result<(), ImportError> {
    for idx in 0..cx.doc.materials.len() {
        let used = cx.materials.keys().any(|(m, _)| *m == idx);
        if !used {
            create(cx, main, log, Some(idx), false)?;
        }
    }
    for idx in 0..cx.doc.images.len() {
        if !cx.images.contains_key(&idx) {
            images::create(cx, main, log, idx)?;
        }
    }
    Ok(())
}

/// The glTF material as a [`LoweredMaterial`] (what `raise` builds the Blender graph from).
fn lowered(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    m: &frap_gltf::json::Material,
    vertex_color: bool,
) -> Result<LoweredMaterial, ImportError> {
    let alpha_mode = match m.alpha_mode.unwrap_or(GAlphaMode::Opaque) {
        GAlphaMode::Opaque => AlphaMode::Opaque,
        GAlphaMode::Mask => AlphaMode::Mask {
            cutoff: m.alpha_cutoff.unwrap_or(0.5) as f32,
        },
        GAlphaMode::Blend => AlphaMode::Blend,
    };
    let pbr = m.pbr_metallic_roughness.clone().unwrap_or_default();
    let base_sel = if alpha_mode == AlphaMode::Opaque {
        ChannelSel::Rgb
    } else {
        ChannelSel::Rgba
    };
    let base_tex = match &pbr.base_color_texture {
        Some(t) => tex_ref(
            cx,
            main,
            log,
            t.index,
            t.tex_coord,
            &t.extensions,
            base_sel,
            false,
        )?,
        None => None,
    };
    let (metal_tex, rough_tex) = match &pbr.metallic_roughness_texture {
        Some(t) => (
            tex_ref(
                cx,
                main,
                log,
                t.index,
                t.tex_coord,
                &t.extensions,
                ChannelSel::B,
                true,
            )?,
            tex_ref(
                cx,
                main,
                log,
                t.index,
                t.tex_coord,
                &t.extensions,
                ChannelSel::G,
                true,
            )?,
        ),
        None => (None, None),
    };
    let normal = match &m.normal_texture {
        Some(t) => tex_ref(
            cx,
            main,
            log,
            t.index,
            t.tex_coord,
            &t.extensions,
            ChannelSel::Rgb,
            true,
        )?
        .map(|texture| NormalMap {
            texture,
            scale: t.scale.unwrap_or(1.0) as f32,
        }),
        None => None,
    };
    let occlusion = match &m.occlusion_texture {
        Some(t) => tex_ref(
            cx,
            main,
            log,
            t.index,
            t.tex_coord,
            &t.extensions,
            ChannelSel::R,
            true,
        )?,
        None => None,
    };
    let emissive_tex = match &m.emissive_texture {
        Some(t) => tex_ref(
            cx,
            main,
            log,
            t.index,
            t.tex_coord,
            &t.extensions,
            ChannelSel::Rgb,
            false,
        )?,
        None => None,
    };
    let emissive_strength = m
        .extensions
        .get::<KhrMaterialsEmissiveStrength>()
        .ok()
        .flatten()
        .map_or(1.0, |e| e.emissive_strength_or_default() as f32);
    let f4 = pbr.base_color_factor.unwrap_or([1.0; 4]).map(|x| x as f32);
    let e3 = m.emissive_factor.unwrap_or([0.0; 3]).map(|x| x as f32);
    let mut lm = LoweredMaterial {
        base_color: Channel {
            factor: f4,
            texture: base_tex,
        },
        metallic: Channel {
            factor: pbr.metallic_factor.unwrap_or(1.0) as f32,
            texture: metal_tex,
        },
        roughness: Channel {
            factor: pbr.roughness_factor.unwrap_or(1.0) as f32,
            texture: rough_tex,
        },
        normal,
        occlusion_strength: m
            .occlusion_texture
            .as_ref()
            .and_then(|o| o.strength)
            .map_or(1.0, |s| s as f32),
        occlusion,
        emissive: Channel {
            factor: e3,
            texture: emissive_tex,
        },
        emissive_strength,
        alpha_mode,
        double_sided: m.double_sided.unwrap_or(false),
        unlit: m.extensions.contains("KHR_materials_unlit"),
        ext: MaterialExts::default(),
        vertex_color: vertex_color.then(|| VertexColorUse {
            attribute: Some(COLOR_0_ATTRIBUTE.into()),
            multiply: true,
        }),
        diagnostics: Vec::new(),
    };
    // `KHR_materials_*` (M3 W0 extension point, `crate::material_ext`; WP 3.6 registers the
    // importers).
    let mut host = ExtHost { cx, main, log };
    for imp in crate::material_ext::importers() {
        (imp.run)(&mut host, m, &mut lm)?;
    }
    Ok(lm)
}

/// [`crate::material_ext::ExtImportHost`] over the importer state.
struct ExtHost<'x, 'a> {
    cx: &'x mut Gltf<'a>,
    main: &'x mut Main,
    log: &'x mut ChangeLog,
}

impl crate::material_ext::ExtImportHost for ExtHost<'_, '_> {
    fn texture(
        &mut self,
        index: u32,
        tex_coord: Option<u32>,
        extensions: &Extensions,
        channel: ChannelSel,
        is_data: bool,
    ) -> Result<Option<TexRef>, ImportError> {
        tex_ref(
            self.cx, self.main, self.log, index, tex_coord, extensions, channel, is_data,
        )
    }

    fn report(&mut self, level: frap_rna::ReportType, message: String) {
        if level == frap_rna::ReportType::Warning {
            let key = message.clone();
            self.cx.warn_once(&key, message);
        } else {
            self.cx.report(level, message);
        }
    }
}

/// `set_filtering` and the wrap-mode part of `texture()`.
fn sampler(cx: &Gltf, texture: usize) -> Sampler {
    let s = cx
        .doc
        .textures
        .get(texture)
        .and_then(|t| t.sampler)
        .and_then(|s| cx.doc.samplers.get(s as usize));
    let (minf, magf) = s.map_or((None, None), |s| (s.min_filter, s.mag_filter));
    let min_nearest = minf.map(|f| {
        matches!(
            f,
            MinFilter::Nearest | MinFilter::NearestMipmapNearest | MinFilter::NearestMipmapLinear
        )
    });
    let mag_nearest = magf.map(|f| f == MagFilter::Nearest);
    let filter = match (min_nearest, mag_nearest) {
        (Some(true), Some(true)) | (Some(true), None) | (None, Some(true)) => Filter::Closest,
        _ => Filter::Linear,
    };
    let wrap = |w: Option<WrapMode>| match w.unwrap_or(WrapMode::Repeat) {
        WrapMode::Repeat => Wrap::Repeat,
        WrapMode::ClampToEdge => Wrap::Clamp,
        WrapMode::MirroredRepeat => Wrap::Mirror,
    };
    Sampler {
        filter,
        wrap_s: wrap(s.and_then(|s| s.wrap_s)),
        wrap_t: wrap(s.and_then(|s| s.wrap_t)),
    }
}

/// A texture reference with its image imported (`None` when the texture has no image).
#[allow(clippy::too_many_arguments)]
fn tex_ref(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    texture: u32,
    tex_coord: Option<u32>,
    exts: &Extensions,
    channel: ChannelSel,
    is_data: bool,
) -> Result<Option<TexRef>, ImportError> {
    let texture = texture as usize;
    let Some(src) = images::texture_source(cx, texture) else {
        return Ok(None);
    };
    let Some(image) = images::create(cx, main, log, src)? else {
        return Ok(None);
    };
    if is_data && let Some(im) = main.images.get_mut(image, log) {
        im.colorspace_settings.is_data = true;
        im.colorspace_settings.name = "Non-Color".into();
    }
    let tt = exts.get::<KhrTextureTransform>().ok().flatten();
    let uv = tt
        .as_ref()
        .and_then(|t| t.tex_coord)
        .or(tex_coord)
        .unwrap_or(0);
    let transform = tt.map(|t| {
        let o = t.offset_or_default();
        let s = t.scale_or_default();
        TexTransform {
            offset: [o[0] as f32, o[1] as f32],
            rotation: t.rotation_or_default() as f32,
            scale: [s[0] as f32, s[1] as f32],
        }
    });
    Ok(Some(TexRef {
        image,
        uv: UvSource::Index(uv),
        channel,
        transform,
        sampler: sampler(cx, texture),
        colorspace_is_data: is_data,
    }))
}

/// A fresh node of type `bl_idname` (the sockets, defaults and properties of
/// `nodes.new(type=…)`), named uniquely in `tree`.
fn new_node(tree: &mut NodeTree, def: &'static NodeTypeDef, location: [f32; 2]) -> u32 {
    fn socket_value(d: &SocketDefault) -> SocketValue {
        match *d {
            SocketDefault::None => SocketValue::None,
            SocketDefault::Float(v) => SocketValue::Float(v),
            SocketDefault::Int(v) => SocketValue::Int(v as i32),
            SocketDefault::Bool(v) => SocketValue::Bool(v),
            SocketDefault::Vector(v) => {
                SocketValue::Vector(std::array::from_fn(|i| v.get(i).copied().unwrap_or(0.0)))
            }
            SocketDefault::Color(c) => SocketValue::Color(c),
            SocketDefault::Str(s) => SocketValue::String(s.into()),
            SocketDefault::Menu(s) => SocketValue::Menu(s.into()),
        }
    }
    let sockets = |defs: &'static [frap_nodes::generated::SocketDef]| -> Vec<Socket> {
        defs.iter()
            .map(|s| Socket {
                name: s.name.into(),
                identifier: s.identifier.into(),
                ty: s.ty,
                enabled: s.enabled,
                hide_value: s.hide_value,
                default_value: socket_value(&s.default),
            })
            .collect()
    };
    let mut props = PropBag::default();
    for (id, d) in def.props {
        let is_enum = def
            .srna
            .find_prop(id)
            .is_some_and(|p| matches!(p.kind, PropKind::Enum { .. }));
        let v = match *d {
            NodePropDefault::None => Value::Id(None),
            NodePropDefault::Bool(b) => Value::Bool(b),
            NodePropDefault::Int(i) => Value::Int(i),
            NodePropDefault::Float(f) => Value::Float(f),
            NodePropDefault::Floats(v) => Value::FloatArray {
                data: v.iter().copied().collect(),
                shape: frap_rna::ArrayShape::Fixed(v.len() as u16),
                tag: frap_rna::MathTag::None,
            },
            NodePropDefault::Str(s) if is_enum => Value::Enum(s.into()),
            NodePropDefault::Str(s) => Value::Str(s.into()),
            NodePropDefault::Flags(f) => {
                Value::EnumSet(f.iter().map(|s| SmolStr::new(*s)).collect())
            }
        };
        props.values.insert(id, v);
    }
    let mut name = SmolStr::new(def.name);
    let mut n = 1;
    while tree.nodes.iter().any(|x| x.name == name) {
        name = SmolStr::new(format!("{}.{:03}", def.name, n));
        n += 1;
    }
    let uid = tree.next_node_uid;
    tree.next_node_uid += 1;
    tree.nodes.push(Node {
        uid,
        name,
        label: String::new(),
        bl_idname: def.bl_idname,
        location,
        width: def.width,
        parent: None,
        mute: false,
        hide: false,
        inputs: sockets(def.inputs),
        outputs: sockets(def.outputs),
        props,
    });
    uid
}

/// The "DefaultMaterial" graph: Principled BSDF → Material Output, plus a Color Attribute on
/// Base Color when the primitive has `COLOR_0`.
fn default_material_tree(tree: &mut NodeTree, vertex_color: bool) {
    tree.nodes.clear();
    tree.links.clear();
    let (Some(out_def), Some(bsdf_def)) = (
        node_type("ShaderNodeOutputMaterial"),
        node_type("ShaderNodeBsdfPrincipled"),
    ) else {
        return;
    };
    let out = new_node(tree, out_def, [0.0, 0.0]);
    let bsdf = new_node(tree, bsdf_def, [-200.0, 0.0]);
    tree.links.push(Link {
        from_node: bsdf,
        from_socket: bsdf_def.outputs[0].identifier.into(),
        to_node: out,
        to_socket: out_def.inputs[0].identifier.into(),
    });
    if vertex_color && let Some(vc_def) = node_type("ShaderNodeVertexColor") {
        let vc = new_node(tree, vc_def, [-400.0, 0.0]);
        tree.links.push(Link {
            from_node: vc,
            from_socket: vc_def.outputs[0].identifier.into(),
            to_node: bsdf,
            to_socket: bsdf_def.inputs[0].identifier.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_material_graph() {
        let mut t = NodeTree::new("Shader Nodetree", frap_nodes::NodeTreeType::Shader, true);
        default_material_tree(&mut t, true);
        let names: Vec<&str> = t.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            ["Material Output", "Principled BSDF", "Color Attribute"]
        );
        assert_eq!(t.links.len(), 2);
        assert_eq!(t.links[0].from_socket, "BSDF");
        assert_eq!(t.links[0].to_socket, "Surface");
        assert_eq!(t.links[1].from_socket, "Color");
        assert_eq!(t.links[1].to_socket, "Base Color");
    }
}
