// SPDX-License-Identifier: GPL-3.0-or-later
//! Textures, samplers and images (ARCHITECTURE §17.2).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/ (glTF-Blender-IO
//! 5.2.41, Apache-2.0, The glTF-Blender-IO authors):
//! - sampler.py: `interpolation == 'Closest'` → NEAREST / NEAREST_MIPMAP_NEAREST, otherwise
//!   LINEAR / LINEAR_MIPMAP_LINEAR (always written); EXTEND and CLIP → CLAMP_TO_EDGE, MIRROR →
//!   MIRRORED_REPEAT; both wraps omitted when both are REPEAT; equal samplers shared.
//! - material/texture.py: a texture is `{sampler, source}`, gathered sampler first (the
//!   sampler of the first socket's Image Texture); no textures at all with
//!   `export_image_format='NONE'`. `gather_texture` is `@cached` on the material's shader
//!   sockets, so there is one texture per material feature even when image and sampler are
//!   equal (verified on 5.2.4: two materials using the same image get two textures), except
//!   that the occlusion and metallic-roughness textures of an ORM image are one texture.
//! - material/texture_info.py: `texCoord` from the UV map of the primary socket's texture (0
//!   omitted), its Mapping node as `KHR_texture_transform` (an `Extension` is required by
//!   default, so the add-on lists it in `extensionsRequired`).
//! - material/image.py and encode_image.py: an image is described by how each channel is
//!   filled (`ExportImage.fills`): a socket without a glTF channel (Base Color, Normal,
//!   Emission) copies every channel of its image; Metallic → B, Roughness → G, Occlusion →
//!   R, Alpha → A take the channel the lowering found (`TexRef::channel`), with the partner
//!   channels filled white (G for metallic, B for roughness, RGB for alpha). When every
//!   channel comes from the same channel of one image (the "happy path"), that image is
//!   written: its file bytes as they are when it is an unmodified FILE/SEQUENCE image whose
//!   bytes match the MIME type (packed bytes first, `Image.is_dirty` respected), else its
//!   pixels re-encoded as `Image.save()` of a copy would (8-bit PNG for a byte buffer, 16-bit
//!   for a float one, colour-managed like WP 3.2's save). Otherwise the channels are packed
//!   into a new image (`__encode_unhappy`): the size of the largest source (others scaled
//!   with `Image.scale`), every channel 1 unless filled, the floats of `Image.pixels` stored
//!   into a byte image (`unit_float_to_uchar_clamp`), RGBA when alpha is filled. An image
//!   source of size (0, 0) (a missing file) is dropped with a WARNING.
//! - MIME type (`__gather_mime_type`): a texture feeding Alpha is PNG; `AUTO` keeps JPEG
//!   sources (by file name, or packed bytes without one) JPEG and writes everything else
//!   (packed images included) PNG; `JPEG` writes JPEG. Name (`__gather_name`): the common
//!   file's base name when every source image has the same `.png`/`.jpg`/`.jpeg` path, else
//!   the source image names without extension joined by `-` (`Image` when empty).
//! - images are shared when bytes, MIME type and name are equal (`__make_image` is cached
//!   on the buffer view, compared by bytes); `GLB`/`GLTF_EMBEDDED` store the bytes in a
//!   buffer view, `GLTF_SEPARATE` writes `<texture dir>/<name><ext>`, every texture taking a
//!   `-N`-suffixed name in turn (io/exp/image_data.py, `set_adjusted_name`; a texture whose
//!   image is shared consumes a name without writing a file, as the add-on does).
//!
//! Image pixels come from `Image::acquire` (WP 3.2): pixel edits, `Image.scale()`, generated
//! patterns and colour spaces are honoured. Limits: WebP sources and
//! `export_image_format='WEBP'` are written as PNG with a WARNING (EXT_texture_webp is not
//! written); JPEG re-encoding uses frap's encoder (not byte-identical to Blender's).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use frap_gltf::Extension;
use frap_gltf::ext::khr_texture_transform::KhrTextureTransform;
use frap_gltf::json::{
    Image as GImage, Index, MagFilter, MinFilter, Sampler, Texture, TextureInfo, WrapMode,
};
use frap_imbuf::{DecodedImage, EncodeFormat, PixelData};
use frap_material::{ChannelSel, Filter, TexRef, UvSource, Wrap};
use frap_mesh::Mesh;
use frap_rna::{IdRef, ImageId, MaterialId};
use frap_scene::id::{ImBuf, ImBufData, Image, PathBase};

use crate::export::gather::Gather;
use crate::export::mesh::convert::{linear_to_srgb, unit_to_u8};
use crate::export::{ExportError, ExportFormat, ExportImageFormat, ExportSettings};

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
    /// `metallicRoughnessTexture` (Metallic and Roughness), and the `occlusionTexture` that
    /// shares its ORM image.
    MetallicRoughness,
    /// `occlusionTexture` (glTF Material Output `Occlusion`; M3 W0).
    Occlusion,
    /// A `KHR_materials_*` texture by its glTF member name (`"clearcoatTexture"`; M3 W0,
    /// `crate::material_ext`).
    Ext(&'static str),
}

/// A texture's identity: the material and the feature using it.
pub type TextureUse = (IdRef<MaterialId>, TextureSlot);

/// How one shader socket fills the exported image (`__get_image_data_mapping`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// A socket without a glTF channel (Base Color, Normal, Emission, unlit colour): every
    /// channel of its image, replacing what earlier sockets filled.
    Full,
    /// `Metallic` → B (G filled white when still empty).
    Metallic,
    /// `Roughness` → G (B filled white when still empty).
    Roughness,
    /// `Occlusion` → R.
    Occlusion,
    /// `Alpha` → A (R, G and B filled white when still empty); the image is PNG.
    Alpha,
    /// Another socket with a fixed destination channel (`Coat Weight` → R, `Thickness` → G,
    /// `Specular IOR Level` → A, …).
    Channel(u8),
}

/// One socket of a texture: its texture reference and its role.
#[derive(Clone, Copy, Debug)]
pub struct TexInput<'t> {
    /// The texture found behind the socket.
    pub tex: &'t TexRef,
    /// How it fills the image.
    pub role: Role,
}

/// What fills one channel of an exported image (`FillImage`, `FillWhite`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Fill {
    /// Channel `src` of an image.
    Image(IdRef<ImageId>, u8),
    /// All ones.
    White,
}

/// `ExportImage`: channel fills in insertion order (a refill keeps the position).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
struct ExportImage {
    fills: Vec<(u8, Fill)>,
}

impl ExportImage {
    fn fill(&mut self, dst: u8, f: Fill) {
        match self.fills.iter_mut().find(|(d, _)| *d == dst) {
            Some(slot) => slot.1 = f,
            None => self.fills.push((dst, f)),
        }
    }

    fn is_filled(&self, dst: u8) -> bool {
        self.fills.iter().any(|(d, _)| *d == dst)
    }

    /// `ExportImage.from_blender_image`: every channel of `image` (frap buffers have four).
    fn from_image(image: IdRef<ImageId>) -> ExportImage {
        ExportImage {
            fills: (0..4).map(|c| (c, Fill::Image(image, c))).collect(),
        }
    }

    /// The images used, in fill order, once each.
    fn images(&self) -> Vec<IdRef<ImageId>> {
        let mut v = Vec::new();
        for (_, f) in &self.fills {
            if let Fill::Image(i, _) = f
                && !v.contains(i)
            {
                v.push(*i);
            }
        }
        v
    }

    /// `__on_happy_path`: every fill is the same channel of one image; that image.
    fn happy(&self) -> Option<IdRef<ImageId>> {
        let mut image = None;
        for (dst, f) in &self.fills {
            match f {
                Fill::Image(i, src) if src == dst => match image {
                    None => image = Some(*i),
                    Some(j) if j == *i => {}
                    Some(_) => return None,
                },
                _ => return None,
            }
        }
        image
    }
}

/// Textures, samplers and images gathered so far.
#[derive(Default)]
pub struct ImageCache {
    samplers: HashMap<(u32, u32, Option<u32>, Option<u32>), Index>,
    /// Encoded bytes per image description and MIME type (`None`: cannot be written).
    encoded: HashMap<(ExportImage, &'static str), Option<Arc<[u8]>>>,
    /// glTF images by (bytes, MIME type, name).
    images: HashMap<(Arc<[u8]>, &'static str, String), Index>,
    textures: HashMap<TextureUse, Option<Index>>,
    file_names: Vec<String>,
}

/// The `TEXCOORD_n` a texture reads on `mesh`: a named UV map's position in the mesh's UV
/// maps, the active-render map's position, or the given index (0 when the map is missing).
pub fn tex_coord(t: &TexRef, mesh: &Mesh) -> u32 {
    let maps = mesh.uv_maps();
    let pos = |name: &str| maps.iter().position(|(n, _)| *n == name);
    let render = || mesh.uv_active_render.as_deref().and_then(pos).unwrap_or(0) as u32;
    match &t.uv {
        // `get_final_material`: a name that is not a UV map uses the render map.
        UvSource::Named(n) => pos(n).map_or_else(render, |p| p as u32),
        UvSource::Index(i) => *i,
        _ => render(),
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

/// `__gather_name` for the images an exported image is made of.
fn images_name(ims: &[&Image]) -> String {
    let one_path = ims.first().map(|i| i.filepath.as_str());
    if let Some(p) = one_path
        && ims.iter().all(|i| i.filepath == p)
    {
        let (stem, ext) = splitext(basename(p));
        if matches!(ext.to_lowercase().as_str(), ".png" | ".jpg" | ".jpeg") && !stem.is_empty() {
            return stem.to_owned();
        }
    }
    let names: Vec<&str> = ims.iter().map(|i| splitext(&i.header.name).0).collect();
    let name = names.join("-");
    if name.is_empty() {
        "Image".to_owned()
    } else {
        name
    }
}

/// The add-on's name of an image used alone (`__gather_name`).
pub fn image_name(im: &Image) -> String {
    images_name(&[im])
}

/// Sources with file data (`__is_blender_image_a_jpeg`: FILE and TILED).
fn is_file_source(im: &Image) -> bool {
    matches!(im.source.as_str(), "FILE" | "TILED")
}

/// Sources whose file bytes may be copied as is (`__encode_from_image`: FILE and SEQUENCE,
/// and `not image.is_dirty`).
fn is_passthrough_source(im: &Image) -> bool {
    matches!(im.source.as_str(), "FILE" | "SEQUENCE")
}

/// `__is_blender_image_a_jpeg`.
fn is_jpeg_image(im: &Image) -> bool {
    is_file_source(im)
        && if im.filepath.is_empty() {
            im.packed_file.as_ref().is_some_and(|p| is_jpeg(&p.bytes))
        } else {
            let p = im.filepath.to_lowercase();
            p.ends_with(".jpg") || p.ends_with(".jpeg") || p.ends_with(".jpe")
        }
}

/// `__is_blender_image_a_webp`.
fn is_webp_image(im: &Image) -> bool {
    is_file_source(im)
        && if im.filepath.is_empty() {
            im.packed_file.as_ref().is_some_and(|p| is_webp(&p.bytes))
        } else {
            im.filepath.to_lowercase().ends_with(".webp")
        }
}

/// The MIME type of an image used alone by `channel` (`__gather_mime_type`).
pub fn mime_type(settings: &ExportSettings, im: &Image, channel: ChannelSel) -> &'static str {
    mime_for(
        settings,
        Some(im),
        matches!(channel, ChannelSel::Rgba | ChannelSel::A),
    )
}

/// `__gather_mime_type`: `happy` is the image written as is (`None` for a packed image),
/// `alpha` whether a socket of the texture is an Alpha socket.
fn mime_for(settings: &ExportSettings, happy: Option<&Image>, alpha: bool) -> &'static str {
    // WebP is written as PNG (no EXT_texture_webp yet, module documentation).
    if alpha || settings.export_image_format == ExportImageFormat::Webp {
        return PNG;
    }
    if settings.wants_jpeg() {
        return JPEG;
    }
    if happy.is_some_and(is_jpeg_image) {
        JPEG
    } else {
        PNG
    }
}

/// Where image paths resolve (`bpy.path.abspath`, then the session's cwd).
fn path_base(g: &Gather) -> PathBase {
    PathBase {
        blend: g.main.filepath.clone(),
        cwd: g.settings.cwd.clone(),
    }
}

/// The file of an image path: `//` relative to the project directory (or the cwd when
/// unsaved), other relative paths against the session's cwd.
fn resolve(g: &Gather, path: &str) -> PathBuf {
    match path.strip_prefix("//") {
        Some(rest) => match &g.settings.blend_dir {
            Some(d) => d.join(rest),
            None => path_base(g).file(rest),
        },
        None => path_base(g).file(path),
    }
}

/// `Image.size`: (0, 0) when no pixels can be loaded.
fn image_size(g: &Gather, im: &Image) -> (u32, u32) {
    if let Some(i) = im.info(&path_base(g)) {
        return (i.x, i.y);
    }
    match file_bytes(g, im) {
        Some(b) if frap_scene::id::IMAGE_PROBES.is_empty() => {
            frap_imbuf::probe(&b).map_or((0, 0), |i| (i.width, i.height))
        }
        _ => (0, 0),
    }
}

/// The encoded bytes of a file image (packed first), if readable.
fn file_bytes(g: &Gather, im: &Image) -> Option<Vec<u8>> {
    if !matches!(im.source.as_str(), "FILE" | "SEQUENCE" | "MOVIE" | "TILED") {
        return None;
    }
    match &im.packed_file {
        Some(p) => Some(p.bytes.clone()),
        None if !im.filepath.is_empty() => std::fs::read(resolve(g, &im.filepath)).ok(),
        None => None,
    }
}

/// `BKE_image_acquire_ibuf` (`Image::acquire`). Library use without the session's image
/// decoder (`IMAGE_DECODERS` empty) decodes file images with frap-imbuf here.
fn acquire(g: &Gather, im: &Image) -> Option<ImBuf> {
    if let Some(b) = im.acquire(&path_base(g)) {
        return Some(b);
    }
    if !frap_scene::id::IMAGE_DECODERS.is_empty() || im.live_buffer().is_some() {
        return None;
    }
    let bytes = file_bytes(g, im)?;
    let d = frap_imbuf::decode(&bytes).ok()?;
    let c = usize::from(d.channels);
    let row = d.width as usize * 4;
    let rgba = |s: &[f32], one: f32| -> [f32; 4] {
        match c {
            1 => [s[0], s[0], s[0], one],
            2 => [s[0], s[0], s[0], s[1]],
            3 => [s[0], s[1], s[2], one],
            _ => [s[0], s[1], s[2], s[3]],
        }
    };
    let data = match &d.pixels {
        PixelData::U8(v) => {
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                let f: Vec<f32> = s.iter().map(|&b| f32::from(b)).collect();
                out.extend(rgba(&f, 255.0).map(|x| x as u8));
            }
            ImBufData::Byte(ImBuf::flip_rows(&out, row))
        }
        PixelData::U16(v) => {
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                let f: Vec<f32> = s.iter().map(|&x| f32::from(x) / 65535.0).collect();
                out.extend(rgba(&f, 1.0));
            }
            ImBufData::Float(ImBuf::flip_rows(&out, row))
        }
        PixelData::F32(v) => {
            let mut out = Vec::with_capacity(v.len() / c * 4);
            for s in v.chunks_exact(c) {
                out.extend(rgba(s, 1.0));
            }
            ImBufData::Float(ImBuf::flip_rows(&out, row))
        }
        _ => return None,
    };
    let mut buf = ImBuf::new(0, 0, (c * 8) as u8, false);
    buf.x = d.width;
    buf.y = d.height;
    buf.data = data;
    buf.handle_colorspace_and_alpha(
        &im.colorspace_settings.name,
        im.is_data_colorspace(),
        &im.alpha_mode,
    );
    Some(buf)
}

/// The pixels of a GENERATED image as bytes (`BKE_image_buf_fill_color` of a BLANK image).
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

/// `BKE_imbuf_alpha_test`: some pixel is not opaque.
fn alpha_test(buf: &ImBuf) -> bool {
    match &buf.data {
        ImBufData::Byte(b) => b.chunks_exact(4).any(|p| p[3] != 255),
        ImBufData::Float(f) => f.chunks_exact(4).any(|p| p[3] != 1.0),
    }
}

/// `IMB_alpha_under_color_byte` over black.
fn under_black_byte(p: &mut [u8]) {
    match p[3] {
        255 => {}
        0 => p[..3].fill(0),
        a => {
            let alpha = (f64::from(a) / 255.0) as f32;
            for c in &mut p[..3] {
                *c = (f32::from(*c) * alpha) as u8;
            }
        }
    }
    p[3] = 255;
}

/// `Image.save()` of a copy of `im` with pixels `buf` as `mime` (`_encode_temp_image` after
/// `make_temp_image_copy`), following `BKE_image_save` as WP 3.2 ports it: the buffer's
/// colour mode (RGB for 24 planes, and for an RGBA buffer whose alpha is opaque), 8-bit PNG
/// for a byte buffer and 16-bit for a float one, a float buffer converted from scene linear
/// to the image's colour space (`sRGB` for a generated image) and unpremultiplied.
fn save_copy(g: &Gather, im: &Image, buf: &ImBuf, mime: &'static str) -> Option<Vec<u8>> {
    encode_copy(im, buf, mime, g.settings.export_image_quality)
}

/// [`save_copy`] with the JPEG quality given. Alpha is composited over black
/// (`IMB_alpha_under_color`) only when the buffer's own colour mode has no alpha (24 planes,
/// or an RGBA buffer whose alpha is opaque); a JPEG written from an RGBA buffer drops the
/// alpha and keeps the straight colour (verified on 5.2.4: a generated image of colour
/// (0.7, 0.1, 0.3, 0.2) exports as (0.702, 0.106, 0.306) with `export_image_format='JPEG'`).
fn encode_copy(im: &Image, buf: &ImBuf, mime: &'static str, quality: u8) -> Option<Vec<u8>> {
    let mut channels: u8 = match buf.planes {
        8 => 1,
        24 | 0 => 3,
        _ => 4,
    };
    // The copy is scaled (`tmp_image.scale(*size)`), which marks it dirty: alpha is tested.
    if channels == 4 && !alpha_test(buf) {
        channels = 3;
    }
    let is_data = im.is_data_colorspace();
    let to_colorspace = if im.source == "GENERATED" && !is_data {
        "sRGB"
    } else {
        im.colorspace_settings.name.as_str()
    };
    let float = buf.is_float();
    // From the buffer's colour mode, before JPEG narrows the output to RGB.
    let under_black = channels != 4;
    let format = if mime == JPEG {
        channels = channels.min(3);
        EncodeFormat::Jpeg { quality }
    } else {
        EncodeFormat::Png {
            bit_depth: if float { 16 } else { 8 },
            compression: 15,
        }
    };
    let row = buf.x as usize * 4;
    let pixels = match &buf.data {
        ImBufData::Byte(b) => {
            let mut v = ImBuf::flip_rows(b, row);
            if under_black {
                v.chunks_exact_mut(4).for_each(under_black_byte);
            }
            PixelData::U8(v)
        }
        ImBufData::Float(f) => {
            let convert =
                !is_data && frap_scene::id::imbuf::to_linear_curve(to_colorspace).is_some();
            let packed = im.source != "GENERATED" && (is_data || im.alpha_mode == "CHANNEL_PACKED");
            // 16-bit PNG: unpremultiplied after the conversion when converted or data;
            // 8-bit output: before it (`IMB_byte_from_float` with `predivide`).
            let (before, after) = match &format {
                EncodeFormat::Png { bit_depth: 16, .. } => (false, convert || is_data),
                _ => (!packed, false),
            };
            let divide = |p: &mut [f32]| {
                let a = p[3];
                if a != 0.0 && a != 1.0 {
                    let inv = 1.0 / a;
                    p[..3].iter_mut().for_each(|c| *c *= inv);
                }
            };
            let mut v = ImBuf::flip_rows(f, row);
            for p in v.chunks_exact_mut(4) {
                if under_black {
                    p[3] = 1.0;
                }
                if before {
                    divide(p);
                }
                if convert {
                    p[..3].iter_mut().for_each(|c| *c = linear_to_srgb(*c));
                }
                if after {
                    divide(p);
                }
            }
            PixelData::F32(v)
        }
    };
    let img = DecodedImage::new(buf.x, buf.y, 4, pixels)
        .ok()?
        .with_channels(channels)
        .ok()?;
    frap_imbuf::encode(&img, &format).ok()
}

/// `__encode_from_image`: the file bytes of an unmodified FILE/SEQUENCE image whose magic
/// number matches `mime`, else its pixels re-encoded.
fn encode_happy(g: &mut Gather, im: &Image, mime: &'static str) -> Option<Vec<u8>> {
    let name = im.header.name.clone();
    let mut data = None;
    if is_passthrough_source(im) && !im.is_dirty() {
        if let Some(p) = &im.packed_file {
            data = Some(p.bytes.clone());
        } else if !im.filepath.is_empty() {
            data = std::fs::read(resolve(g, &im.filepath)).ok();
        }
    }
    if let Some(b) = &data {
        if (mime == PNG && is_png(b)) || (mime == JPEG && is_jpeg(b)) {
            return data;
        }
        if is_webp(b) {
            g.warn(format!(
                "image '{name}': WebP textures (EXT_texture_webp) are not exported yet; written as PNG"
            ));
        }
    }
    let Some(buf) = acquire(g, im) else {
        g.warn(format!(
            "Error while saving image: image '{name}' has no data"
        ));
        return None;
    };
    let out = save_copy(g, im, &buf, mime);
    if out.is_none() {
        g.warn(format!("image '{name}' not exported: it cannot be encoded"));
    }
    out
}

/// `__encode_unhappy`: the channels packed into a new image (see the module documentation).
fn encode_unhappy(g: &mut Gather, ex: &ExportImage, mime: &'static str) -> Option<Vec<u8>> {
    let main = g.main;
    let images = ex.images();
    let (w, h, out) = if images.is_empty() {
        (1, 1, vec![1.0f32; 4])
    } else {
        let sizes: Vec<(u32, u32)> = images
            .iter()
            .map(|r| main.images.get(*r).map_or((0, 0), |im| image_size(g, im)))
            .collect();
        let w = sizes.iter().map(|s| s.0).max().unwrap_or(1);
        let h = sizes.iter().map(|s| s.1).max().unwrap_or(1);
        frap_math::budget::check_buffer(
            frap_scene::id::imbuf::BUFFER,
            frap_scene::id::imbuf::buffer_bytes(w, h, true),
        );
        let mut out = vec![1.0f32; w as usize * h as usize * 4];
        for r in &images {
            let im = main.images.get(*r)?;
            let mut buf = acquire(g, im)?;
            if (buf.x, buf.y) != (w, h) {
                buf.scale(w, h);
            }
            let px = buf.pixels();
            for (dst, f) in &ex.fills {
                if let Fill::Image(i, src) = f
                    && i == r
                {
                    let (d, s) = (usize::from(*dst), usize::from(*src));
                    for (o, p) in out.chunks_exact_mut(4).zip(px.chunks_exact(4)) {
                        o[d] = p[s];
                    }
                }
            }
        }
        (w, h, out)
    };
    // `images.new(…, alpha=Channel.A in fills)` + `pixels.foreach_set` + `save()`.
    let alpha = ex.is_filled(3);
    let mut bytes: Vec<u8> = out.iter().map(|v| unit_to_u8(*v)).collect();
    let row = w as usize * 4;
    bytes = ImBuf::flip_rows(&bytes, row);
    let mut channels = if alpha { 4 } else { 3 };
    if alpha && bytes.chunks_exact(4).all(|p| p[3] == 255) {
        channels = 3;
    }
    let format = if mime == JPEG {
        channels = 3;
        EncodeFormat::Jpeg {
            quality: g.settings.export_image_quality,
        }
    } else {
        EncodeFormat::Png {
            bit_depth: 8,
            compression: 15,
        }
    };
    if channels == 3 && alpha {
        bytes.chunks_exact_mut(4).for_each(under_black_byte);
    }
    let img = DecodedImage::new(w, h, 4, PixelData::U8(bytes))
        .ok()?
        .with_channels(channels)
        .ok()?;
    frap_imbuf::encode(&img, &format).ok()
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

/// The fills of a texture's sockets (`__get_image_data_mapping`), images of size (0, 0)
/// dropped with a WARNING.
fn compose(g: &mut Gather, inputs: &[TexInput]) -> ExportImage {
    let src = |t: &TexRef, default: u8| match t.channel {
        ChannelSel::R => 0,
        ChannelSel::G => 1,
        ChannelSel::B => 2,
        ChannelSel::A | ChannelSel::Rgba => 3,
        _ => default,
    };
    let mut ex = ExportImage::default();
    for i in inputs {
        let img = i.tex.image;
        match i.role {
            Role::Full => ex = ExportImage::from_image(img),
            Role::Metallic => {
                ex.fill(2, Fill::Image(img, src(i.tex, 2)));
                if !ex.is_filled(1) {
                    ex.fill(1, Fill::White);
                }
            }
            Role::Roughness => {
                ex.fill(1, Fill::Image(img, src(i.tex, 1)));
                if !ex.is_filled(2) {
                    ex.fill(2, Fill::White);
                }
            }
            Role::Occlusion => ex.fill(0, Fill::Image(img, src(i.tex, 0))),
            Role::Alpha => {
                ex.fill(3, Fill::Image(img, src(i.tex, 3)));
                for c in 0..3 {
                    if !ex.is_filled(c) {
                        ex.fill(c, Fill::White);
                    }
                }
            }
            Role::Channel(dst) => ex.fill(dst, Fill::Image(img, src(i.tex, dst))),
        }
    }
    // "Check that we don't have some empty channels".
    let main = g.main;
    let mut empty = Vec::new();
    for (_, f) in &ex.fills {
        if let Fill::Image(r, _) = f
            && main
                .images
                .get(*r)
                .is_none_or(|im| image_size(g, im).0 == 0 || image_size(g, im).1 == 0)
            && !empty.contains(r)
        {
            empty.push(*r);
        }
    }
    for r in empty {
        let name = main
            .images
            .get(r)
            .map_or_else(String::new, |im| im.header.name.to_string());
        g.warn(format!(
            "Image '{name}' has no size and cannot be exported."
        ));
        ex.fills
            .retain(|(_, f)| !matches!(f, Fill::Image(i, _) if *i == r));
    }
    ex
}

/// The texture index for the sockets `inputs` used by `usage` (`None` when its image cannot
/// be exported). One texture per usage, as the add-on does; the sampler is the first
/// socket's.
pub fn gather_texture_from(
    g: &mut Gather,
    inputs: &[TexInput],
    usage: TextureUse,
) -> Result<Option<Index>, ExportError> {
    if let Some(i) = g.images.textures.get(&usage) {
        return Ok(*i);
    }
    let i = gather_new_texture(g, inputs)?;
    g.images.textures.insert(usage, i);
    Ok(i)
}

/// The texture index of a texture reference used alone by `usage` (see [`slot_inputs`]).
pub fn gather_texture(
    g: &mut Gather,
    t: &TexRef,
    usage: TextureUse,
) -> Result<Option<Index>, ExportError> {
    let inputs = slot_inputs(usage.1, t);
    gather_texture_from(g, &inputs, usage)
}

/// The sockets a single texture reference stands for in `slot`: the base colour's alpha
/// (`Rgba`: colour and alpha of one image; `A`/`R`/`G`/`B`: alpha only), the metallic and
/// roughness channels, the occlusion channel, and the fixed channels of the extension
/// textures (`clearcoatTexture` R, `clearcoatRoughnessTexture` G, `thicknessTexture` G,
/// `specularTexture` A, `sheenRoughnessTexture` A); every other texture is copied whole.
pub fn slot_inputs(slot: TextureSlot, t: &TexRef) -> Vec<TexInput<'_>> {
    let one = |role| vec![TexInput { tex: t, role }];
    match slot {
        TextureSlot::BaseColor => match t.channel {
            ChannelSel::Rgb => one(Role::Full),
            ChannelSel::Rgba => vec![
                TexInput {
                    tex: t,
                    role: Role::Full,
                },
                TexInput {
                    tex: t,
                    role: Role::Alpha,
                },
            ],
            _ => one(Role::Alpha),
        },
        TextureSlot::MetallicRoughness => match t.channel {
            ChannelSel::G => one(Role::Roughness),
            _ => one(Role::Metallic),
        },
        TextureSlot::Occlusion => one(Role::Occlusion),
        TextureSlot::Ext("clearcoatTexture") => one(Role::Channel(0)),
        TextureSlot::Ext("clearcoatRoughnessTexture" | "thicknessTexture") => one(Role::Channel(1)),
        TextureSlot::Ext("specularTexture" | "sheenRoughnessTexture") => one(Role::Channel(3)),
        _ => one(Role::Full),
    }
}

fn gather_new_texture(g: &mut Gather, inputs: &[TexInput]) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let Some(first) = inputs.first() else {
        return Ok(None);
    };
    if g.settings.export_image_format == ExportImageFormat::Webp {
        g.warn("export_image_format='WEBP' is not supported yet (EXT_texture_webp); images are written as PNG");
    }
    let ex = compose(g, inputs);
    if ex.fills.is_empty() {
        return Ok(None);
    }
    let happy = ex.happy().and_then(|r| main.images.get(r));
    if let Some(im) = happy.filter(|im| is_webp_image(im)) {
        g.warn(format!(
            "image '{}': WebP textures (EXT_texture_webp) are not exported yet; written as PNG",
            im.header.name
        ));
    }
    let alpha = inputs.iter().any(|i| i.role == Role::Alpha);
    let mime = mime_for(g.settings, happy, alpha);
    let names: Vec<&Image> = ex
        .images()
        .iter()
        .filter_map(|r| main.images.get(*r))
        .collect();
    let name = images_name(&names);
    // Encode first: an image that cannot be written leaves no sampler behind.
    let key = (ex.clone(), mime);
    let bytes = match g.images.encoded.get(&key) {
        Some(b) => b.clone(),
        None => {
            let b = match ex.happy() {
                Some(r) => match main.images.get(r) {
                    Some(im) => encode_happy(g, im, mime),
                    None => None,
                },
                None => encode_unhappy(g, &ex, mime),
            };
            let b: Option<Arc<[u8]>> = b.filter(|b| !b.is_empty()).map(Arc::from);
            g.images.encoded.insert(key, b.clone());
            b
        }
    };
    let Some(bytes) = bytes else {
        g.warn(format!(
            "image '{name}': Image data is empty, not exporting image"
        ));
        return Ok(None);
    };
    // Members in order: sampler, then source.
    let s = sampler(first.tex);
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
    let image_index = gather_image(g, bytes, mime, name);
    g.doc.textures.push(Texture {
        sampler: Some(sampler_index),
        source: Some(image_index),
        ..Texture::default()
    });
    Ok(Some((g.doc.textures.len() - 1) as Index))
}

/// The image index for encoded `bytes` (`__make_image`, shared by bytes, MIME type and name).
fn gather_image(g: &mut Gather, bytes: Arc<[u8]>, mime: &'static str, name: String) -> Index {
    let separate = g.settings.export_format == ExportFormat::GltfSeparate;
    // `set_real_uri`: every gathered image takes a file name, used or not.
    let rel = separate.then(|| {
        let ext = if mime == JPEG { ".jpg" } else { ".png" };
        let file = adjusted_name(&name, ext, &g.images.file_names);
        g.images.file_names.push(file.clone());
        let dir = g
            .settings
            .export_texture_dir
            .trim_start_matches("./")
            .trim_end_matches(['/', '\\']);
        if dir.is_empty() {
            file
        } else {
            format!("{}/{file}", dir.replace('\\', "/"))
        }
    });
    let key = (bytes.clone(), mime, name.clone());
    if let Some(i) = g.images.images.get(&key) {
        return *i;
    }
    let mut img = GImage {
        mime_type: Some(mime.to_owned()),
        name: Some(name),
        ..GImage::default()
    };
    match rel {
        Some(rel) => {
            img.uri = Some(path_to_uri(&rel));
            g.files.push((rel, bytes.to_vec()));
        }
        None => img.buffer_view = Some(g.buf.push_view(&bytes, 4, None, None)),
    }
    g.doc.images.push(img);
    let i = (g.doc.images.len() - 1) as Index;
    g.images.images.insert(key, i);
    i
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

/// A `textureInfo` for the sockets `inputs` used by `usage` on `mesh`: `texCoord` and
/// `KHR_texture_transform` from `primary` (`None` when nothing is written).
pub fn gather_texture_info_from(
    g: &mut Gather,
    primary: &TexRef,
    inputs: &[TexInput],
    usage: TextureUse,
    mesh: &Mesh,
) -> Result<Option<TextureInfo>, ExportError> {
    if g.settings.export_image_format == ExportImageFormat::None {
        return Ok(None);
    }
    // A texture info without a texture is dropped with its extensions, so
    // `KHR_texture_transform` is listed only when the texture is written.
    let tt = texture_transform(primary);
    let Some(index) = gather_texture_from(g, inputs, usage)? else {
        return Ok(None);
    };
    let n = tex_coord(primary, mesh);
    let mut info = TextureInfo::new(index);
    info.tex_coord = (n != 0).then_some(n);
    if let Some(tt) = tt {
        info.extensions.set(Some(tt));
        g.use_extension(KhrTextureTransform::NAME, true);
    }
    Ok(Some(info))
}

/// A `textureInfo` for a texture reference used alone by `usage` on `mesh` (`None` when
/// nothing is written).
pub fn gather_texture_info(
    g: &mut Gather,
    t: &TexRef,
    usage: TextureUse,
    mesh: &Mesh,
) -> Result<Option<TextureInfo>, ExportError> {
    let inputs = slot_inputs(usage.1, t);
    gather_texture_info_from(g, t, &inputs, usage, mesh)
}

#[cfg(test)]
mod tests {
    use frap_material::{Sampler as LSampler, TexTransform};
    use frap_scene::id::EncodedImage;

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
        // Packed channels: the source names joined; one common file: its name.
        let a = Image::new("metal.png", 1, 1);
        let b = Image::new("rough", 1, 1);
        assert_eq!(images_name(&[&a, &b]), "metal-rough");
        let mut c = Image::new("x", 1, 1);
        let mut d = Image::new("y", 1, 1);
        c.filepath = "//orm.png".into();
        d.filepath = "//orm.png".into();
        assert_eq!(images_name(&[&c, &d]), "orm");
        assert_eq!(images_name(&[]), "Image");
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

    /// The first pixel of an encoded image, as RGB bytes.
    fn first_rgb(bytes: &[u8]) -> [u8; 3] {
        let d = frap_imbuf::decode(bytes).unwrap().with_channels(3).unwrap();
        match d.pixels {
            PixelData::U8(v) => [v[0], v[1], v[2]],
            _ => panic!("8-bit image expected"),
        }
    }

    #[test]
    fn jpeg_of_an_rgba_buffer_keeps_the_straight_colour() {
        // Verified on 5.2.4 (`export_image_format='JPEG'`): an RGBA image (32 planes) with
        // generated colour (0.7, 0.1, 0.3, 0.2) exports as (0.702, 0.106, 0.306): the alpha is
        // dropped, not composited over black.
        let mut im = Image::new("Q", 16, 16);
        im.generated_depth = 32;
        im.generated_color = [0.7, 0.1, 0.3, 0.2];
        let buf = im.generated_buffer().unwrap();
        let jpeg = encode_copy(&im, &buf, JPEG, 75).unwrap();
        assert!(is_jpeg(&jpeg));
        let [r, g, b] = first_rgb(&jpeg);
        for (got, want) in [(r, 179u8), (g, 27), (b, 78)] {
            assert!(got.abs_diff(want) <= 3, "({r}, {g}, {b})");
        }
        // PNG keeps the alpha channel and the straight colour.
        let png = encode_copy(&im, &buf, PNG, 75).unwrap();
        let d = frap_imbuf::decode(&png).unwrap();
        assert_eq!(d.channels, 4);
        // A 24-plane buffer whose pixels carry alpha (`pixels = …` on an image made without
        // alpha) is composited over black, as Blender does (0.7 × 0.2 = 0.14 → 36).
        let mut im = Image::new("N", 4, 4);
        im.generated_color = [0.7, 0.1, 0.3, 0.2];
        let mut buf = im.generated_buffer().unwrap();
        buf.planes = 24;
        for bytes in [
            encode_copy(&im, &buf, PNG, 75).unwrap(),
            encode_copy(&im, &buf, JPEG, 75).unwrap(),
        ] {
            let [r, _, _] = first_rgb(&bytes);
            assert!(r.abs_diff(36) <= 3, "{r}");
        }
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

    #[test]
    fn channel_fills_follow_image_py() {
        let r = IdRef::from_key(frap_rna::IdKey::next());
        let other = IdRef::from_key(frap_rna::IdKey::next());
        let mut t = tex(Filter::Linear, Wrap::Repeat, Wrap::Repeat);
        t.image = r;
        // Metallic B + roughness G of one image: the happy path.
        let mut m = t.clone();
        m.channel = ChannelSel::B;
        let mut ro = t.clone();
        ro.channel = ChannelSel::G;
        let mut ex = ExportImage::default();
        for (tx, role) in [(&m, Role::Metallic), (&ro, Role::Roughness)] {
            match role {
                Role::Metallic => {
                    ex.fill(2, Fill::Image(tx.image, 2));
                    if !ex.is_filled(1) {
                        ex.fill(1, Fill::White);
                    }
                }
                _ => {
                    ex.fill(1, Fill::Image(tx.image, 1));
                }
            }
        }
        assert_eq!(ex.happy(), Some(r));
        assert_eq!(ex.fills.len(), 2);
        // Full copy then alpha from the same image's alpha: still happy.
        let mut ex = ExportImage::from_image(r);
        ex.fill(3, Fill::Image(r, 3));
        assert_eq!(ex.happy(), Some(r));
        // Alpha from another image: packed.
        ex.fill(3, Fill::Image(other, 3));
        assert_eq!(ex.happy(), None);
        assert_eq!(ex.images(), vec![r, other]);
        // Base colour slot: RGBA = full + alpha; an alpha-only texture fills RGB white.
        t.channel = ChannelSel::Rgba;
        let roles: Vec<Role> = slot_inputs(TextureSlot::BaseColor, &t)
            .iter()
            .map(|i| i.role)
            .collect();
        assert_eq!(roles, [Role::Full, Role::Alpha]);
        t.channel = ChannelSel::A;
        let roles: Vec<Role> = slot_inputs(TextureSlot::BaseColor, &t)
            .iter()
            .map(|i| i.role)
            .collect();
        assert_eq!(roles, [Role::Alpha]);
    }
}
