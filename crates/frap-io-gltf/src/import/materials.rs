// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF materials → `bpy.data.materials`: the datablock and its non-node properties
//! (`use_backface_culling`, `surface_render_method`, viewport `diffuse_color`), a
//! [`LoweredMaterial`] read from the glTF material (textures resolved to imported images), and
//! the node graph built from it by `frap_material::raise` (WP 1.11, §16.2). Materials without
//! a glTF material but with `COLOR_0` get the add-on's "DefaultMaterial" graph.
//! `KHR_materials_pbrSpecularGlossiness` materials are raised as the importer's spec-gloss
//! graph (`frap_material::patterns::pbr_specular_glossiness`, with the roughness image baked
//! here as `make_roughness_image` does).
//! `KHR_materials_unlit` materials are raised as the importer's unlit graph (`unlit.rs`), an
//! `occlusionTexture` into the "glTF Material Output" group node, whose `node_tree`
//! `material_ext::bind_settings_nodes` links to `bpy.data.node_groups` (WP 3.5/3.6).
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
    NameAlloc, NormalMap, Sampler, SpecularGlossiness, TexRef, TexTransform, UvSource,
    VertexColorUse, Wrap,
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
        // WP 3.6: the settings node `raise` made gets the "glTF Material Output" group.
        crate::material_ext::bind_settings_nodes(main, log, &mut mat);
    }
    let r = main.materials.insert(&name, mat, log);
    cx.materials.insert((idx, vertex_color), r);
    Ok(r)
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
    let unlit = m.extensions.contains("KHR_materials_unlit");
    // `BlenderMaterial.create`: unlit first, then spec-gloss, else metallic-roughness (the
    // spec-gloss graph reads none of `pbrMetallicRoughness`, so its textures are not loaded).
    let spec_gloss = (!unlit)
        .then(|| m.extensions.get_raw(SPEC_GLOSS))
        .flatten()
        .cloned();
    let pbr = if spec_gloss.is_some() {
        Default::default()
    } else {
        m.pbr_metallic_roughness.clone().unwrap_or_default()
    };
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
        unlit,
        ext: MaterialExts::default(),
        // The importer's Color Attribute node names no layer (the active colour attribute,
        // which is the imported COLOR_0) and links its Alpha only when the material is not
        // OPAQUE (`base_color()`).
        vertex_color: vertex_color.then(|| {
            let mut v = VertexColorUse::color_and_alpha(None, true);
            v.alpha = alpha_mode != AlphaMode::Opaque;
            v
        }),
        diagnostics: Vec::new(),
    };
    if let Some(raw) = spec_gloss {
        // The add-on's spec-gloss graph ignores every other material extension.
        spec_gloss_material(cx, main, log, &raw, base_sel, &mut lm)?;
        return Ok(lm);
    }
    // `KHR_materials_*` (M3 W0 extension point, `crate::material_ext`; WP 3.6 registers the
    // importers).
    let mut host = ExtHost { cx, main, log };
    for imp in crate::material_ext::importers() {
        (imp.run)(&mut host, m, &mut lm)?;
    }
    Ok(lm)
}

/// `KHR_materials_pbrSpecularGlossiness`.
const SPEC_GLOSS: &str = "KHR_materials_pbrSpecularGlossiness";

/// A `textureInfo` of the raw extension JSON (`None` when absent or malformed).
fn raw_texture_info(v: &serde_json::Value, key: &str) -> Option<frap_gltf::json::TextureInfo> {
    serde_json::from_value(v.get(key)?.clone()).ok()
}

/// A number array of the raw extension JSON (`None` when absent or malformed).
fn raw_factor<const N: usize>(v: &serde_json::Value, key: &str) -> Option<[f32; N]> {
    let a = v.get(key)?.as_array()?;
    if a.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = x.as_f64()? as f32;
    }
    Some(out)
}

/// `pbr_specular_glossiness(mh)`'s inputs: `diffuseFactor`/`diffuseTexture` as the base
/// colour (`base_color(is_diffuse=True)`), `specularFactor` with the RGB of
/// `specularGlossinessTexture`, `glossinessFactor`, and the baked roughness image
/// (`make_roughness_image`) when the texture is used for glossiness. Ported from
/// io_scene_gltf2 5.2.41 `blender/imp/KHR_materials_pbrSpecularGlossiness.py` (Apache-2.0,
/// The glTF-Blender-IO authors).
fn spec_gloss_material(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    ext: &serde_json::Value,
    base_sel: ChannelSel,
    lm: &mut LoweredMaterial,
) -> Result<(), ImportError> {
    lm.base_color.factor = raw_factor::<4>(ext, "diffuseFactor").unwrap_or([1.0; 4]);
    lm.base_color.texture = match raw_texture_info(ext, "diffuseTexture") {
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
    let sg_tex = raw_texture_info(ext, "specularGlossinessTexture");
    let specular_texture = match &sg_tex {
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
    let glossiness_factor = ext
        .get("glossinessFactor")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(1.0) as f32;
    let roughness = match &sg_tex {
        Some(t) if glossiness_factor != 0.0 => {
            match bake_roughness(cx, main, log, t.index, glossiness_factor)? {
                Some(image) => tex_ref(
                    cx,
                    main,
                    log,
                    t.index,
                    t.tex_coord,
                    &t.extensions,
                    ChannelSel::A,
                    false,
                )?
                .map(|r| TexRef { image, ..r }),
                None => None,
            }
        }
        _ => None,
    };
    lm.ext = MaterialExts::default();
    lm.ext.pbr_specular_glossiness = Some(SpecularGlossiness {
        specular: Channel {
            factor: raw_factor::<3>(ext, "specularFactor").unwrap_or([1.0; 3]),
            texture: specular_texture,
        },
        glossiness_factor,
        has_texture: sg_tex.is_some(),
        roughness,
    });
    Ok(())
}

/// `make_roughness_image`: a packed copy of the texture's image (`image.copy()`, so
/// `<name>.001`) whose alpha is 1 − `glossiness` × alpha, through `Image.pixels` (bytes
/// rounded as `unit_float_to_uchar_clamp`), then `image.pack()` (PNG for a byte buffer in the
/// image's colour mode, OpenEXR for a float one). Cached per glTF image, whatever the factor,
/// as the add-on caches it.
fn bake_roughness(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    texture: u32,
    glossiness: f32,
) -> Result<Option<IdRef<frap_rna::ImageId>>, ImportError> {
    let Some(src) = images::texture_source(cx, texture as usize) else {
        return Ok(None);
    };
    let Some(orig) = images::create(cx, main, log, src)? else {
        return Ok(None);
    };
    if let Some(&r) = cx.roughness_images.get(&src) {
        return Ok(Some(r));
    }
    let Some(im) = main.images.get(orig) else {
        return Ok(None);
    };
    let base = frap_scene::id::PathBase {
        blend: main.filepath.clone(),
        cwd: None,
    };
    let mut image = im.clone();
    let name = image.header.name.clone();
    image.header.use_fake_user = false;
    image.header.library = None;
    image.header.is_linked_packed = false;
    if let Some(mut buf) = im.acquire(&base) {
        let mut px = buf.pixels();
        for p in px.chunks_exact_mut(4) {
            // `pixels[:, :, 3] *= factor; *= -1; += 1` on a float32 array (NumPy casts the
            // Python factor to float32).
            let mut a = p[3];
            if glossiness != 1.0 {
                a *= glossiness;
            }
            p[3] = -a + 1.0;
        }
        buf.set_pixels(&px);
        if let Some((bytes, mime, ff)) = memorypack(&buf) {
            image.packed_file = Some(std::sync::Arc::new(frap_scene::id::EncodedImage {
                bytes,
                mime: SmolStr::new_static(mime),
            }));
            image.packed_filepath = image.filepath.clone();
            image.file_format = SmolStr::new_static(ff);
            buf.dirty = false;
            buf.ftype = SmolStr::new_static(ff);
        }
        image.buffer = Some(std::sync::Arc::new(buf));
    }
    let r = main.images.insert(&name, image, log);
    cx.roughness_images.insert(src, r);
    Ok(Some(r))
}

/// `BKE_image_memorypack` of a modified buffer: PNG (8 bits, in the buffer's colour mode)
/// for bytes, OpenEXR (32 bits) for floats; (bytes, MIME type, `file_format`).
fn memorypack(buf: &frap_scene::id::ImBuf) -> Option<(Vec<u8>, &'static str, &'static str)> {
    use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
    use frap_scene::id::{ImBuf, ImBufData};
    let row = buf.x as usize * 4;
    let channels = match buf.planes {
        8 => 1,
        24 => 3,
        _ => 4,
    };
    let (pixels, format, mime, ff) = match &buf.data {
        ImBufData::Byte(b) => (
            PixelData::U8(ImBuf::flip_rows(b, row)),
            EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
            "image/png",
            "PNG",
        ),
        ImBufData::Float(f) => (
            PixelData::F32(ImBuf::flip_rows(f, row)),
            EncodeFormat::OpenExr { bit_depth: 32 },
            "image/x-exr",
            "OPEN_EXR",
        ),
    };
    let img = DecodedImage::new(buf.x, buf.y, 4, pixels)
        .ok()?
        .with_channels(channels)
        .ok()?;
    let bytes = frap_imbuf::encode(&img, &format).ok()?;
    Some((bytes, mime, ff))
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
