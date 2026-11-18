// SPDX-License-Identifier: GPL-3.0-or-later
//! Textures, samplers and images (ARCHITECTURE §17.2).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/:
//! - sampler.py: `interpolation == 'Closest'` → NEAREST / NEAREST_MIPMAP_NEAREST, otherwise
//!   LINEAR / LINEAR_MIPMAP_LINEAR (always written); EXTEND and CLIP → CLAMP_TO_EDGE, MIRROR →
//!   MIRRORED_REPEAT; both wraps omitted when both are REPEAT; equal samplers shared.
//! - material/texture.py: a texture is `{sampler, source}`, gathered sampler first; no textures
//!   at all with `export_image_format='NONE'`. `gather_texture` is `@cached` on the material's
//!   shader sockets and the exporter shares objects by identity (`__append_unique_and_get_index`),
//!   so there is one texture per material feature even when image and sampler are equal
//!   (verified on 5.2.4: two materials using the same image get two textures); samplers are
//!   shared by value (sampler.py, `__sampler_by_value`) and images by image and MIME type.
//! - material/texture_info.py: `texCoord` from the UV map the texture uses (0 omitted), the
//!   Mapping node as `KHR_texture_transform` (an `Extension` is required by default, so the
//!   add-on lists it in `extensionsRequired`).
//! - material/image.py and encode_image.py: the MIME type (`AUTO`: JPEG sources stay JPEG,
//!   everything else PNG; a texture whose alpha is used is PNG), the name (the file's base name
//!   for `.png`/`.jpg`/`.jpeg` files, else the image name without its extension), the original
//!   file bytes copied when their magic number matches the MIME type, otherwise the pixels
//!   re-encoded; `GLB`/`GLTF_EMBEDDED` store the bytes in a buffer view, `GLTF_SEPARATE` writes
//!   `<texture dir>/<name><ext>` with a `-N` suffix for repeated names (io/exp/image_data.py,
//!   `set_adjusted_name`).
//!
//! M1 limits (M3, WP 3.5): no channel packing (every texture is used "as is"), WebP sources and
//! `export_image_format='WEBP'` are written as PNG with a WARNING, generated images other than
//! BLANK are filled with their colour.

use std::collections::HashMap;
use std::path::PathBuf;

use frap_gltf::Extension;
use frap_gltf::ext::khr_texture_transform::KhrTextureTransform;
use frap_gltf::json::{
    Image as GImage, Index, MagFilter, MinFilter, Sampler, Texture, TextureInfo, WrapMode,
};
use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_material::{ChannelSel, Filter, TexRef, UvSource, Wrap};
use frap_mesh::Mesh;
use frap_rna::{IdRef, ImageId, MaterialId};
use frap_scene::id::Image;

use crate::export::gather::Gather;
use crate::export::mesh::convert::unit_to_u8;
use crate::export::{ExportError, ExportFormat, ExportImageFormat};

/// MIME types the exporter writes.
pub const PNG: &str = "image/png";
/// JPEG.
pub const JPEG: &str = "image/jpeg";

/// The material feature a texture is gathered for (the add-on's shader sockets).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureSlot {
    /// `emissiveTexture` (Emission Color).
    Emissive,
    /// `normalTexture` (Normal).
    Normal,
    /// `baseColorTexture` (Base Color, plus Alpha when it is used).
    BaseColor,
    /// `metallicRoughnessTexture` (Metallic and Roughness).
    MetallicRoughness,
    /// `occlusionTexture` (glTF Material Output `Occlusion`; M3 W0).
    Occlusion,
    /// A `KHR_materials_*` texture by its glTF member name (`"clearcoatTexture"`; M3 W0,
    /// `crate::material_ext`).
    Ext(&'static str),
}

/// A texture's identity: the material and the feature using it.
pub type TextureUse = (IdRef<MaterialId>, TextureSlot);

/// Textures, samplers and images gathered so far.
#[derive(Default)]
pub struct ImageCache {
    samplers: HashMap<(u32, u32, Option<u32>, Option<u32>), Index>,
    images: HashMap<(IdRef<ImageId>, &'static str), Option<Index>>,
    textures: HashMap<TextureUse, Option<Index>>,
    file_names: Vec<String>,
}

/// The `TEXCOORD_n` a texture reads on `mesh`: a named UV map's position in the mesh's UV
/// maps, the active-render map's position, or the given index (0 when the map is missing).
pub fn tex_coord(t: &TexRef, mesh: &Mesh) -> u32 {
    let maps = mesh.uv_maps();
    let pos = |name: &str| maps.iter().position(|(n, _)| *n == name).unwrap_or(0) as u32;
    match &t.uv {
        UvSource::Named(n) => pos(n),
        UvSource::Index(i) => *i,
        _ => mesh.uv_active_render.as_deref().map(pos).unwrap_or(0),
    }
}

/// The sampler of a texture (`None` wraps when both are REPEAT).
pub fn sampler(t: &TexRef) -> Sampler {
    let (mag, min) = match t.sampler.filter {
        Filter::Closest => (MagFilter::Nearest, MinFilter::NearestMipmapNearest),
        _ => (MagFilter::Linear, MinFilter::LinearMipmapLinear),
    };
    let wrap = |w: Wrap| match w {
        Wrap::Clamp => WrapMode::ClampToEdge,
        Wrap::Mirror => WrapMode::MirroredRepeat,
        _ => WrapMode::Repeat,
    };
    let (s, t) = (wrap(t.sampler.wrap_s), wrap(t.sampler.wrap_t));
    let both_repeat = s == WrapMode::Repeat && t == WrapMode::Repeat;
    Sampler {
        mag_filter: Some(mag),
        min_filter: Some(min),
        wrap_s: (!both_repeat).then_some(s),
        wrap_t: (!both_repeat).then_some(t),
        ..Sampler::default()
    }
}

fn is_jpeg(b: &[u8]) -> bool {
    b.starts_with(&[0xFF, 0xD8, 0xFF])
}
fn is_png(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG")
}
fn is_webp(b: &[u8]) -> bool {
    b.get(8..12) == Some(b"WEBP")
}

/// `os.path.basename` for both separators.
fn basename(p: &str) -> &str {
    p.rsplit(['/', '\\']).next().unwrap_or(p)
}

/// `os.path.splitext`: the stem and the extension (a leading dot is not an extension).
fn splitext(name: &str) -> (&str, &str) {
    let lead = name.len() - name.trim_start_matches('.').len();
    match name[lead..].rfind('.') {
        Some(i) => (&name[..lead + i], &name[lead + i..]),
        None => (name, ""),
    }
}

/// The add-on's image name (`__gather_name`).
pub fn image_name(im: &Image) -> String {
    let (stem, ext) = splitext(basename(&im.filepath));
    if matches!(ext.to_lowercase().as_str(), ".png" | ".jpg" | ".jpeg") && !stem.is_empty() {
        return stem.to_owned();
    }
    let (stem, _) = splitext(&im.header.name);
    if stem.is_empty() {
        "Image".to_owned()
    } else {
        stem.to_owned()
    }
}

/// Sources with file data (`__is_blender_image_a_jpeg`: FILE and TILED).
fn is_file_source(im: &Image) -> bool {
    matches!(im.source.as_str(), "FILE" | "SEQUENCE" | "TILED")
}

/// Sources whose file bytes may be copied as is (`__encode_from_image`: FILE and SEQUENCE,
/// and `not image.is_dirty`; frap does not track `is_dirty` yet, see deviations.md).
fn is_passthrough_source(im: &Image) -> bool {
    matches!(im.source.as_str(), "FILE" | "SEQUENCE")
}

/// The MIME type of a texture's image (`__gather_mime_type`).
pub fn mime_type(
    settings: &crate::export::ExportSettings,
    im: &Image,
    channel: ChannelSel,
) -> &'static str {
    if matches!(channel, ChannelSel::Rgba | ChannelSel::A) {
        return PNG;
    }
    if settings.wants_jpeg() {
        return JPEG;
    }
    let jpeg_source = matches!(im.source.as_str(), "FILE" | "TILED")
        && if im.filepath.is_empty() {
            im.packed_file.as_ref().is_some_and(|p| is_jpeg(&p.bytes))
        } else {
            let p = im.filepath.to_lowercase();
            p.ends_with(".jpg") || p.ends_with(".jpeg") || p.ends_with(".jpe")
        };
    if jpeg_source { JPEG } else { PNG }
}

/// `bpy.path.abspath`: `//` is relative to the project directory (or the cwd when unsaved).
fn resolve(settings: &crate::export::ExportSettings, path: &str) -> PathBuf {
    match path.strip_prefix("//") {
        Some(rest) => settings
            .blend_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest),
        None => PathBuf::from(path),
    }
}

/// The source file bytes of a FILE image (packed data first), if readable.
fn source_bytes(g: &Gather, im: &Image) -> Option<Vec<u8>> {
    if !is_file_source(im) {
        return None;
    }
    if let Some(p) = &im.packed_file {
        return Some(p.bytes.clone());
    }
    if im.filepath.is_empty() {
        return None;
    }
    std::fs::read(resolve(g.settings, &im.filepath)).ok()
}

/// The pixels of a GENERATED image (`BKE_image_buf_fill_color`: the colour as bytes, no
/// colour conversion; every generated type is filled with its colour in M1).
pub fn generated_pixels(im: &Image) -> DecodedImage {
    let (w, h) = (
        im.generated_width.max(1) as u32,
        im.generated_height.max(1) as u32,
    );
    let px = im.generated_color.map(unit_to_u8);
    DecodedImage {
        width: w,
        height: h,
        channels: 4,
        pixels: PixelData::U8(px.repeat((w * h) as usize)),
    }
}

/// The bytes to write for an image as `mime` (`ExportImage.encode`, happy path).
fn encode(g: &mut Gather, im: &Image, mime: &'static str) -> Option<Vec<u8>> {
    let name = im.header.name.clone();
    let src = source_bytes(g, im);
    if let Some(b) = &src {
        let passthrough = is_passthrough_source(im);
        if passthrough && ((mime == PNG && is_png(b)) || (mime == JPEG && is_jpeg(b))) {
            return Some(b.clone());
        }
        if is_webp(b) {
            g.warn(format!(
                "image '{name}': WebP textures (EXT_texture_webp) are not exported yet (M3); written as PNG"
            ));
        }
    }
    let pixels = match (&src, im.source.as_str()) {
        (Some(b), _) => match frap_imbuf::decode(b) {
            Ok(p) => p,
            Err(e) => {
                g.warn(format!("image '{name}' not exported: {e}"));
                return None;
            }
        },
        (None, "GENERATED") => {
            if im.generated_type != "BLANK" {
                g.warn(format!(
                    "image '{name}': generated type {} is exported as a blank fill (M3)",
                    im.generated_type
                ));
            }
            generated_pixels(im)
        }
        (None, _) => {
            g.warn(format!(
                "image '{name}' not exported: no packed data and '{}' cannot be read",
                im.filepath
            ));
            return None;
        }
    };
    let format = if mime == JPEG {
        EncodeFormat::Jpeg {
            quality: g.settings.export_image_quality,
        }
    } else {
        EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        }
    };
    match frap_imbuf::encode(&pixels, &format) {
        Ok(b) => Some(b),
        Err(e) => {
            g.warn(format!("image '{name}' not exported: {e}"));
            None
        }
    }
}

/// `urllib.parse.quote(path)` (safe: `/`, letters, digits, `_.-~`).
pub fn path_to_uri(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/_.-~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `ImageData.set_adjusted_name`: `name.ext`, then `name-1.ext`, `name-2.ext`, … (a trailing
/// `-N` is replaced).
pub fn adjusted_name(name: &str, ext: &str, taken: &[String]) -> String {
    let mut n = name.to_owned();
    let mut count = 1;
    while taken.iter().any(|t| *t == format!("{n}{ext}")) {
        let trailing = n
            .rfind('-')
            .filter(|&i| i + 1 < n.len() && n[i + 1..].bytes().all(|b| b.is_ascii_digit()));
        n = match trailing {
            Some(i) => format!("{}-{count}", &n[..i]),
            None => format!("{n}-{count}"),
        };
        count += 1;
    }
    format!("{n}{ext}")
}

/// The image index for `(image, mime)` (`None` when it cannot be written).
fn gather_image(
    g: &mut Gather,
    r: IdRef<ImageId>,
    im: &Image,
    mime: &'static str,
    bytes: Vec<u8>,
) -> Index {
    let name = image_name(im);
    let mut img = GImage {
        mime_type: Some(mime.to_owned()),
        name: Some(name.clone()),
        ..GImage::default()
    };
    if g.settings.export_format == ExportFormat::GltfSeparate {
        let ext = if mime == JPEG { ".jpg" } else { ".png" };
        let file = adjusted_name(&name, ext, &g.images.file_names);
        g.images.file_names.push(file.clone());
        let dir = g
            .settings
            .export_texture_dir
            .trim_start_matches("./")
            .trim_end_matches(['/', '\\']);
        let rel = if dir.is_empty() {
            file
        } else {
            format!("{}/{file}", dir.replace('\\', "/"))
        };
        img.uri = Some(path_to_uri(&rel));
        g.files.push((rel, bytes));
    } else {
        img.buffer_view = Some(g.buf.push_view(&bytes, 4, None, None));
    }
    g.doc.images.push(img);
    let i = (g.doc.images.len() - 1) as Index;
    g.images.images.insert((r, mime), Some(i));
    i
}

/// The texture index of a texture reference used by `usage` (`None` when its image cannot be
/// exported). One texture per usage, as the add-on does.
pub fn gather_texture(
    g: &mut Gather,
    t: &TexRef,
    usage: TextureUse,
) -> Result<Option<Index>, ExportError> {
    if let Some(i) = g.images.textures.get(&usage) {
        return Ok(*i);
    }
    let i = gather_new_texture(g, t)?;
    g.images.textures.insert(usage, i);
    Ok(i)
}

fn gather_new_texture(g: &mut Gather, t: &TexRef) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let Some(im) = main.images.get(t.image) else {
        return Ok(None);
    };
    if g.settings.export_image_format == ExportImageFormat::Webp {
        g.warn("export_image_format='WEBP' is not supported yet (M3); images are written as PNG");
    }
    let mime = mime_type(g.settings, im, t.channel);
    // Encode first: an image that cannot be written leaves no sampler behind.
    let cached = g.images.images.get(&(t.image, mime)).copied();
    let bytes = match cached {
        Some(None) => return Ok(None),
        Some(Some(_)) => None,
        None => match encode(g, im, mime) {
            Some(b) => Some(b),
            None => {
                g.images.images.insert((t.image, mime), None);
                return Ok(None);
            }
        },
    };
    // Members in order: sampler, then source.
    let s = sampler(t);
    let skey = (
        s.mag_filter.map_or(0, u32::from),
        s.min_filter.map_or(0, u32::from),
        s.wrap_s.map(u32::from),
        s.wrap_t.map(u32::from),
    );
    let sampler_index = match g.images.samplers.get(&skey) {
        Some(i) => *i,
        None => {
            g.doc.samplers.push(s);
            let i = (g.doc.samplers.len() - 1) as Index;
            g.images.samplers.insert(skey, i);
            i
        }
    };
    let image_index = match (cached, bytes) {
        (Some(Some(i)), _) => i,
        (_, Some(b)) => gather_image(g, t.image, im, mime, b),
        _ => return Ok(None),
    };
    g.doc.textures.push(Texture {
        sampler: Some(sampler_index),
        source: Some(image_index),
        ..Texture::default()
    });
    Ok(Some((g.doc.textures.len() - 1) as Index))
}

/// `KHR_texture_transform` of a texture (identity members omitted; `None` when all are).
pub fn texture_transform(t: &TexRef) -> Option<KhrTextureTransform> {
    let tr = t.transform?;
    let ext = KhrTextureTransform {
        offset: (tr.offset != [0.0, 0.0]).then(|| tr.offset.map(f64::from)),
        rotation: (tr.rotation != 0.0).then(|| f64::from(tr.rotation)),
        scale: (tr.scale != [1.0, 1.0]).then(|| tr.scale.map(f64::from)),
        ..KhrTextureTransform::default()
    };
    (ext != KhrTextureTransform::default()).then_some(ext)
}

/// A `textureInfo` for a texture reference used by `usage` on `mesh` (`None` when nothing is
/// written).
pub fn gather_texture_info(
    g: &mut Gather,
    t: &TexRef,
    usage: TextureUse,
    mesh: &Mesh,
) -> Result<Option<TextureInfo>, ExportError> {
    if g.settings.export_image_format == ExportImageFormat::None {
        return Ok(None);
    }
    let Some(index) = gather_texture(g, t, usage)? else {
        return Ok(None);
    };
    let n = tex_coord(t, mesh);
    let mut info = TextureInfo::new(index);
    info.tex_coord = (n != 0).then_some(n);
    if let Some(tt) = texture_transform(t) {
        info.extensions.set(Some(tt));
        g.use_extension(KhrTextureTransform::NAME, true);
    }
    Ok(Some(info))
}

#[cfg(test)]
mod tests {
    use frap_material::{Sampler as LSampler, TexTransform};
    use frap_scene::id::EncodedImage;
    use std::sync::Arc;

    use super::*;
    use crate::export::ExportSettings;

    fn tex(filter: Filter, s: Wrap, t: Wrap) -> TexRef {
        TexRef {
            image: IdRef::from_key(frap_rna::IdKey::next()),
            uv: UvSource::ActiveRender,
            channel: ChannelSel::Rgb,
            transform: None,
            sampler: LSampler {
                filter,
                wrap_s: s,
                wrap_t: t,
            },
            colorspace_is_data: false,
        }
    }

    #[test]
    fn sampler_rules() {
        let s = sampler(&tex(Filter::Linear, Wrap::Repeat, Wrap::Repeat));
        assert_eq!(s.mag_filter, Some(MagFilter::Linear));
        assert_eq!(s.min_filter, Some(MinFilter::LinearMipmapLinear));
        assert_eq!((s.wrap_s, s.wrap_t), (None, None));
        let s = sampler(&tex(Filter::Closest, Wrap::Clamp, Wrap::Repeat));
        assert_eq!(s.mag_filter, Some(MagFilter::Nearest));
        assert_eq!(s.min_filter, Some(MinFilter::NearestMipmapNearest));
        assert_eq!(
            (s.wrap_s, s.wrap_t),
            (Some(WrapMode::ClampToEdge), Some(WrapMode::Repeat))
        );
        let s = sampler(&tex(Filter::Cubic, Wrap::Mirror, Wrap::Mirror));
        assert_eq!(s.mag_filter, Some(MagFilter::Linear));
        assert_eq!(s.wrap_s, Some(WrapMode::MirroredRepeat));
    }

    #[test]
    fn names_and_uris() {
        let mut im = Image::new("Wood.001", 1, 1);
        assert_eq!(image_name(&im), "Wood");
        im.filepath = "//tex/oak planks.PNG".into();
        assert_eq!(image_name(&im), "oak planks");
        im.filepath = "//tex/oak.tga".into();
        assert_eq!(image_name(&im), "Wood");
        assert_eq!(splitext(".hidden"), (".hidden", ""));
        assert_eq!(path_to_uri("tex/oak planks.png"), "tex/oak%20planks.png");
        assert_eq!(path_to_uri("é.png"), "%C3%A9.png");
        let taken = vec!["a.png".to_owned(), "a-1.png".to_owned()];
        assert_eq!(adjusted_name("a", ".png", &taken), "a-2.png");
        assert_eq!(adjusted_name("b", ".png", &taken), "b.png");
    }

    #[test]
    fn mime_rules() {
        let s = ExportSettings::default();
        let mut im = Image::new("I", 1, 1);
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgb), PNG);
        im.source = "FILE".into();
        im.filepath = "//a.JPG".into();
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgb), JPEG);
        // Alpha forces PNG.
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgba), PNG);
        // `__is_blender_image_a_jpeg`: FILE and TILED only.
        im.source = "TILED".into();
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgb), JPEG);
        im.source = "SEQUENCE".into();
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgb), PNG);
        // `__encode_from_image` copies bytes for FILE and SEQUENCE only.
        assert!(is_passthrough_source(&im));
        im.source = "TILED".into();
        assert!(!is_passthrough_source(&im));
        im.source = "FILE".into();
        im.filepath.clear();
        im.packed_file = Some(Arc::new(EncodedImage {
            bytes: vec![0xFF, 0xD8, 0xFF, 0xE0],
            mime: "image/jpeg".into(),
        }));
        assert_eq!(mime_type(&s, &im, ChannelSel::Rgb), JPEG);
        let jpeg = ExportSettings {
            export_image_format: ExportImageFormat::Jpeg,
            ..ExportSettings::default()
        };
        assert_eq!(
            mime_type(&jpeg, &Image::new("G", 1, 1), ChannelSel::Rgb),
            JPEG
        );
    }

    #[test]
    fn transform_identity_omitted() {
        let mut t = tex(Filter::Linear, Wrap::Repeat, Wrap::Repeat);
        assert!(texture_transform(&t).is_none());
        t.transform = Some(TexTransform {
            offset: [0.0, 0.0],
            rotation: 0.0,
            scale: [1.0, 1.0],
        });
        assert!(texture_transform(&t).is_none());
        t.transform = Some(TexTransform {
            offset: [0.1, -1.2],
            rotation: 0.0,
            scale: [2.0, 2.0],
        });
        let e = texture_transform(&t).unwrap();
        assert_eq!(e.rotation, None);
        assert_eq!(e.scale, Some([2.0, 2.0]));
    }

    #[test]
    fn generated_fill_is_the_colour_as_bytes() {
        let mut im = Image::new("G", 2, 1);
        im.generated_color = [0.5, 0.25, 1.0, 0.5];
        let p = generated_pixels(&im);
        // Verified on 5.2.4: `pixels[:4]` = 128/255, 64/255, 1, 128/255.
        assert_eq!(
            p.pixels,
            PixelData::U8(vec![128, 64, 255, 128, 128, 64, 255, 128])
        );
    }
}
