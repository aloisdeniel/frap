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
//! - MIME type (`__gather_mime_type`): a texture feeding Alpha is PNG (WebP with
//!   `export_image_format='WEBP'` or when its image is a WebP file); `AUTO` keeps JPEG and WebP
//!   sources (by file name, or packed bytes without one) as they are and writes everything
//!   else (packed images included) PNG; `JPEG` writes JPEG, `WEBP` WebP. Name
//!   (`__gather_name`): the common file's base name when every source image has the same
//!   `.png`/`.jpg`/`.jpeg` path, else the source image names without extension joined by `-`
//!   (`Image` when empty).
//! - WebP (`EXT_texture_webp`, WP 7.3): texture.py's `__gather_source` and
//!   `__gather_extensions`, see [`crate::export::images_webp`]. An image re-encoded as WebP
//!   is saved as `Image.save(quality=export_image_quality)` does: lossless at quality 100,
//!   else lossy (libwebp, method 4). A build without the `webp-lossy` feature writes it
//!   lossless with a WARNING.
//! - KTX2 (`KHR_texture_basisu`, frap's `frap_texture_format='KTX2'`, WP 7.3): see
//!   [`crate::export::images_ktx2`].
//! - images are shared when bytes, MIME type and name are equal (`__make_image` is cached
//!   on the buffer view, compared by bytes); `GLB`/`GLTF_EMBEDDED` store the bytes in a
//!   buffer view, `GLTF_SEPARATE` writes `<texture dir>/<name><ext>`, every image gathered
//!   taking a `-N`-suffixed name in turn (io/exp/image_data.py, `set_adjusted_name`; an image
//!   that is shared, or that the WebP rules replace, consumes a name without writing a file,
//!   as the add-on does). The images of a texture enter `images` in the add-on's traversal
//!   order: the extensions' sources before the texture's own `source`.
//!
//! Image pixels come from `Image::acquire` (WP 3.2): pixel edits, `Image.scale()`, generated
//! patterns and colour spaces are honoured. Limits: JPEG and WebP re-encoding use frap's
//! encoders (not byte-identical to Blender's; WebP lossless keeps the samples).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use frap_gltf::Extension;
use frap_gltf::ext::khr_texture_transform::KhrTextureTransform;
use frap_gltf::json::{
    Image as GImage, Index, MagFilter, MinFilter, Sampler, Texture, TextureInfo, WrapMode,
};
use frap_imbuf::basis::BasisOptions;
use frap_imbuf::{DecodedImage, EncodeFormat, ImageError, PixelData};
use frap_material::{ChannelSel, Filter, TexRef, Wrap};
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
/// WebP (`EXT_texture_webp`).
pub const WEBP: &str = "image/webp";
/// KTX2 (`KHR_texture_basisu`).
pub const KTX2: &str = frap_imbuf::ktx2::MIME;

/// What an exported image is encoded as (`ExportImage.file_format`, plus frap's KTX2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Target {
    /// `PNG`
    Png,
    /// `JPEG`
    Jpeg,
    /// `WEBP`
    Webp,
    /// KTX2 with Basis Universal supercompression.
    Ktx2(BasisOptions),
}

impl Target {
    /// `ExportImage.encode`'s `file_format` of a MIME type (PNG for anything else).
    pub(crate) fn from_mime(mime: &str) -> Target {
        match mime {
            JPEG => Target::Jpeg,
            WEBP => Target::Webp,
            _ => Target::Png,
        }
    }

    /// Whether `bytes` (an unmodified file's) can be written as they are (`__encode_from_image`'s
    /// magic-number check).
    fn accepts(self, bytes: &[u8]) -> bool {
        match self {
            Target::Png => is_png(bytes),
            Target::Jpeg => is_jpeg(bytes),
            Target::Webp => is_webp(bytes),
            // A KTX2 file is copied only when it keeps every rule of KHR_texture_basisu for
            // the texture's colour space; another (an odd size, a wrong transfer function, a
            // raw format) is re-encoded from its decoded pixels.
            Target::Ktx2(o) => {
                use frap_gltf::ext::khr_texture_basisu::{Usage, ktx2_issues};
                let usage = if o.srgb { Usage::Colour } else { Usage::Data };
                frap_imbuf::ktx2::is_ktx2(bytes) && ktx2_issues(bytes, Some(usage)).is_empty()
            }
        }
    }
}

/// An image gathered for a texture but not yet in `images` (`gltf2_io.Image` before the
/// traversal numbers it): its file name is taken when it is gathered.
#[derive(Clone, Debug)]
pub(crate) struct PendingImage {
    pub bytes: Arc<[u8]>,
    pub mime: &'static str,
    pub name: String,
    /// `GLTF_SEPARATE`: the URI path relative to the glTF file.
    rel: Option<String>,
}

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
    /// The `k`-th unused Image Texture of a material (`export_unused_textures`, WP 8.12,
    /// `unused.rs`).
    Additional(usize),
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
    /// `Alpha` fed by an image's colour output (`Channel.RGB2BW`, WP 8.12): its grey value
    /// → A, as [`Role::Alpha`] otherwise.
    AlphaBw,
    /// `Anisotropic` of a grayscale anisotropy texture (WP 8.12): its grey value → B.
    AnisotropyStrength,
    /// `Anisotropic Rotation` of a grayscale anisotropy texture (WP 8.12): the direction of its
    /// grey value × 2π → R, G.
    AnisotropyRotation,
    /// Another socket with a fixed destination channel (`Coat Weight` → R, `Thickness` → G,
    /// `Specular IOR Level` → A, …).
    Channel(u8),
}

impl Role {
    /// The `Alpha` socket ([`Role::Alpha`] or [`Role::AlphaBw`]).
    pub fn is_alpha(self) -> bool {
        matches!(self, Role::Alpha | Role::AlphaBw)
    }
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
    /// The grey value of an image's colour (`FillImageRGB2BW`, WP 8.12).
    ImageBw(IdRef<ImageId>),
    /// All ones.
    White,
}

/// `ExportImage`: channel fills in insertion order (a refill keeps the position).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct ExportImage {
    fills: Vec<(u8, Fill)>,
    /// `set_calc(grayscale_anisotropy_calculation)` with the stored images (WP 8.12):
    /// `(anisotropy, anisotropic_rotation)`, `None` for a stored value (an unlinked socket).
    anisotropy: Option<AnisotropyImages>,
}

/// The stored images of a grayscale anisotropy bake: `(anisotropy, anisotropic_rotation)`.
type AnisotropyImages = (Option<IdRef<ImageId>>, Option<IdRef<ImageId>>);

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
            anisotropy: None,
        }
    }

    /// The images used, in fill order, once each.
    fn images(&self) -> Vec<IdRef<ImageId>> {
        let mut v = Vec::new();
        for (_, f) in &self.fills {
            if let Fill::Image(i, _) | Fill::ImageBw(i) = f
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
    /// Encoded bytes per image description and target (`None`: cannot be written).
    encoded: HashMap<(ExportImage, Target), Option<Arc<[u8]>>>,
    /// glTF images by (bytes, MIME type, name).
    images: HashMap<(Arc<[u8]>, &'static str, String), Index>,
    textures: HashMap<TextureUse, Option<Index>>,
    file_names: Vec<String>,
    /// Names of the images a texture's image was made from (the add-on's
    /// `export_settings['exported_images']`, WP 8.12: `export_unused_images` skips them).
    exported: std::collections::HashSet<String>,
}

impl ImageCache {
    /// Whether a texture already wrote (part of) image `r` (`img.name in exported_images`).
    pub(crate) fn was_exported(&self, main: &frap_scene::Main, r: IdRef<ImageId>) -> bool {
        main.images
            .get(r)
            .is_some_and(|im| self.exported.contains(im.header.name.as_str()))
    }
}

/// `__get_mime_type_of_image` for an image written whole (`export_unused_images`, WP 8.12):
/// a 4-channel image is PNG, WebP for a WebP file; otherwise `export_image_format` decides
/// (`AUTO`: JPEG and WebP files keep their format, `JPEG`: JPEG), and `WEBP` and `NONE` fall
/// off the end of the add-on's function: no MIME type (the data is encoded as PNG). Only an
/// image without a buffer (a missing file, 0 channels) has fewer than 4 channels in frap.
pub(crate) fn whole_image_mime(
    settings: &ExportSettings,
    im: &Image,
    four: bool,
) -> Option<&'static str> {
    if four {
        return Some(if is_webp_image(im) { WEBP } else { PNG });
    }
    match settings.export_image_format {
        ExportImageFormat::Auto if is_jpeg_image(im) => Some(JPEG),
        ExportImageFormat::Auto if is_webp_image(im) => Some(WEBP),
        ExportImageFormat::Auto => Some(PNG),
        ExportImageFormat::Jpeg => Some(JPEG),
        _ => None,
    }
}

/// `Image.channels` (`ibuf->channels`): 4 for every byte buffer Blender allocates (generated,
/// PNG, JPEG, …) and for frap's float buffers, which are RGBA; 0 when the image has no
/// buffer (a missing file).
pub(crate) fn image_channels(g: &Gather, im: &Image) -> u8 {
    let (w, h) = image_size(g, im);
    if w == 0 || h == 0 { 0 } else { 4 }
}

/// `get_gltf_image_from_blender_image` + `__traverse` (WP 8.12): image `r` written whole as
/// `mime` (`None`: encoded as PNG, no `mimeType`), named as the add-on names it; nothing when
/// it cannot be encoded. An image without a buffer (a missing file: `Image.channels` 0) gives
/// `ExportImage.from_blender_image` no fill, so the add-on writes its fill-less encode, a 1×1
/// white pixel, named `Image` (`__gather_name` of no image; verified on 5.2.4).
pub(crate) fn write_whole_image(
    g: &mut Gather,
    r: IdRef<ImageId>,
    mime: Option<&'static str>,
) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let Some(im) = main.images.get(r) else {
        return Ok(None);
    };
    let (ex, name) = if image_channels(g, im) == 0 {
        (ExportImage::default(), "Image".to_owned())
    } else {
        g.images.exported.insert(im.header.name.to_string());
        (ExportImage::from_image(r), image_name(im))
    };
    let Some(bytes) = encode(g, &ex, Target::from_mime(mime.unwrap_or(PNG)))? else {
        g.warn(format!(
            "image '{name}': Image data is empty, not exporting image"
        ));
        return Ok(None);
    };
    let p = make_image(g, bytes, mime.unwrap_or(PNG), name);
    let i = push_image_unshared(g, p);
    if mime.is_none()
        && let Some(img) = g.doc.images.get_mut(i as usize)
    {
        img.mime_type = None;
    }
    Ok(Some(i))
}

/// `tex_coord` moved to its own file (M7 W0, decision log M7W0-4: WP 7.7's Attribute-node
/// texCoord, while WP 7.3 owns this file).
pub use crate::export::tex_coord::tex_coord;

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

/// `__gather_mime_type`: `happy` is the image written as is (`export_image.blender_image()`:
/// `None` when channels are packed), `alpha` whether a socket of the texture is an Alpha
/// socket.
fn mime_for(settings: &ExportSettings, happy: Option<&Image>, alpha: bool) -> &'static str {
    let webp_format = settings.export_image_format == ExportImageFormat::Webp;
    if alpha {
        // "force png or webp if Alpha contained so we can export alpha"
        return if webp_format || happy.is_some_and(is_webp_image) {
            WEBP
        } else {
            PNG
        };
    }
    match settings.export_image_format {
        ExportImageFormat::Webp => WEBP,
        _ if settings.wants_jpeg() => JPEG,
        _ if happy.is_some_and(is_jpeg_image) => JPEG,
        _ if happy.is_some_and(is_webp_image) => WEBP,
        _ => PNG,
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

/// How a target is encoded: a `frap_imbuf` format, or the Basis Universal encoder.
#[derive(Clone, Debug, PartialEq)]
enum Format {
    Image(EncodeFormat),
    Ktx2(BasisOptions),
}

impl Format {
    /// `_encode_temp_image`: `save(quality=export_image_quality)` for JPEG and WebP (WebP
    /// lossless at 100, `format_webp.cc`), 16-bit PNG for a float buffer.
    fn of(target: Target, float: bool, quality: u8) -> Format {
        Format::Image(match target {
            Target::Png => EncodeFormat::Png {
                bit_depth: if float { 16 } else { 8 },
                compression: 15,
            },
            Target::Jpeg => EncodeFormat::Jpeg { quality },
            Target::Webp => EncodeFormat::WebP {
                lossless: quality >= 100,
                quality,
            },
            Target::Ktx2(o) => return Format::Ktx2(o),
        })
    }
}

/// Encoded bytes; `lossless_fallback`: a lossy WebP was written lossless (no `webp-lossy`).
#[derive(Debug)]
struct Encoded {
    bytes: Vec<u8>,
    lossless_fallback: bool,
}

/// Encodes `img`; a lossy WebP in a build without the `webp-lossy` feature is written
/// lossless instead (the caller warns).
fn encode_pixels(img: &DecodedImage, format: &Format) -> Result<Encoded, ImageError> {
    let done = |bytes| Encoded {
        bytes,
        lossless_fallback: false,
    };
    match format {
        Format::Ktx2(o) => frap_imbuf::basis::encode_ktx2_with(img, o).map(done),
        Format::Image(f @ EncodeFormat::WebP { quality, .. }) => match frap_imbuf::encode(img, f) {
            Err(ImageError::Unsupported(m)) if m.contains("webp-lossy") => {
                let lossless = EncodeFormat::WebP {
                    lossless: true,
                    quality: *quality,
                };
                Ok(Encoded {
                    bytes: frap_imbuf::encode(img, &lossless)?,
                    lossless_fallback: true,
                })
            }
            r => r.map(done),
        },
        Format::Image(f) => frap_imbuf::encode(img, f).map(done),
    }
}

/// `Image.save()` of a copy of `im` with pixels `buf` as `target` (`_encode_temp_image` after
/// `make_temp_image_copy`), following `BKE_image_save` as WP 3.2 ports it: the buffer's
/// colour mode (RGB for 24 planes, and for an RGBA buffer whose alpha is opaque), 8-bit PNG
/// for a byte buffer and 16-bit for a float one, a float buffer converted from scene linear
/// to the image's colour space (`sRGB` for a generated image) and unpremultiplied.
fn save_copy(g: &Gather, im: &Image, buf: &ImBuf, target: Target) -> Result<Encoded, ImageError> {
    encode_copy(im, buf, target, g.settings.export_image_quality)
}

/// [`save_copy`] with the JPEG / WebP quality given. Alpha is composited over black
/// (`IMB_alpha_under_color`) only when the buffer's own colour mode has no alpha (24 planes,
/// or an RGBA buffer whose alpha is opaque); a JPEG written from an RGBA buffer drops the
/// alpha and keeps the straight colour (verified on 5.2.4: a generated image of colour
/// (0.7, 0.1, 0.3, 0.2) exports as (0.702, 0.106, 0.306) with `export_image_format='JPEG'`).
/// WebP and KTX2 are 8-bit: a float buffer is converted as for an 8-bit PNG.
///
/// A greyscale buffer (8 planes: a 1-channel PNG or JPEG file) cannot be saved as WebP:
/// Blender's OpenImageIO writer refuses it ("webp does not support 1-channel images", then
/// `Image.save` raises "Could not write image: internal error, see console"), so the add-on
/// logs "Error while saving image" and gets no bytes (verified on 5.2.4: no WebP image, and
/// with `export_image_format='WEBP'` a texture without `source`).
fn encode_copy(
    im: &Image,
    buf: &ImBuf,
    target: Target,
    quality: u8,
) -> Result<Encoded, ImageError> {
    if target == Target::Webp && buf.planes == 8 {
        return Err(ImageError::Encode(WEBP_GREY_ERROR.into()));
    }
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
    if target == Target::Jpeg {
        channels = channels.min(3);
    }
    let format = Format::of(target, float, quality);
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
                Format::Image(EncodeFormat::Png { bit_depth: 16, .. }) => {
                    (false, convert || is_data)
                }
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
    let img = DecodedImage::new(buf.x, buf.y, 4, pixels)?.with_channels(channels)?;
    encode_pixels(&img, &format)
}

/// Why a greyscale (8-plane) buffer cannot be saved as WebP (`Image.save`'s error after
/// OpenImageIO's).
pub(crate) const WEBP_GREY_ERROR: &str = "Error: Could not write image: internal error, see console (webp does not support \
     1-channel images)";

/// The NOT_IMPLEMENTED error of `frap_texture_format='KTX2'` in a build without the encoder.
pub(crate) fn ktx2_encoder_missing() -> ExportError {
    ExportError::NotImplemented(
        "frap_texture_format='KTX2' needs frap built with the `basisu-encode` feature \
         (the Basis Universal encoder)"
            .into(),
    )
}

/// The outcome of an encode for `name`: the bytes, `None` with a WARNING when the image cannot
/// be written, the [`ktx2_encoder_missing`] error when KTX2 cannot be encoded at all.
fn finish_encode(
    g: &mut Gather,
    name: &str,
    target: Target,
    r: Result<Encoded, ImageError>,
) -> Result<Option<Vec<u8>>, ExportError> {
    match r {
        Ok(e) => {
            if e.lossless_fallback {
                g.warn(format!(
                    "image '{name}': lossy WebP needs frap built with the `webp-lossy` \
                     feature; written as lossless WebP"
                ));
            }
            Ok(Some(e.bytes))
        }
        Err(ImageError::Unsupported(_)) if matches!(target, Target::Ktx2(_)) => {
            Err(ktx2_encoder_missing())
        }
        Err(ImageError::Encode(m)) if m == WEBP_GREY_ERROR => {
            // `_encode_temp_image`: "Error while saving image: %s" % e (then b'').
            g.warn(format!("Error while saving image: image '{name}': {m}"));
            Ok(None)
        }
        Err(e) => {
            // `_encode_temp_image`: "Error while saving image: %s" (then b'').
            g.warn(format!("Error while saving image: image '{name}': {e}"));
            Ok(None)
        }
    }
}

/// `__encode_from_image`: the file bytes of an unmodified FILE/SEQUENCE image whose magic
/// number matches `target`, else its pixels re-encoded.
fn encode_happy(
    g: &mut Gather,
    im: &Image,
    target: Target,
) -> Result<Option<Vec<u8>>, ExportError> {
    let name = im.header.name.clone();
    let mut data = None;
    if is_passthrough_source(im) && !im.is_dirty() {
        if let Some(p) = &im.packed_file {
            data = Some(p.bytes.clone());
        } else if !im.filepath.is_empty() {
            data = std::fs::read(resolve(g, &im.filepath)).ok();
        }
    }
    if let Some(b) = &data
        && target.accepts(b)
    {
        return Ok(data);
    }
    let Some(buf) = acquire(g, im) else {
        g.warn(format!(
            "Error while saving image: image '{name}' has no data"
        ));
        return Ok(None);
    };
    let r = save_copy(g, im, &buf, target);
    finish_encode(g, &name, target, r)
}

/// `__encode_unhappy`: the channels packed into a new image (see the module documentation).
fn encode_unhappy(
    g: &mut Gather,
    ex: &ExportImage,
    target: Target,
) -> Result<Option<Vec<u8>>, ExportError> {
    let main = g.main;
    if let Some((strength, rotation)) = ex.anisotropy {
        return encode_anisotropy(g, strength, rotation, target);
    }
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
            let Some(im) = main.images.get(*r) else {
                return Ok(None);
            };
            let Some(mut buf) = acquire(g, im) else {
                return Ok(None);
            };
            if (buf.x, buf.y) != (w, h) {
                buf.scale(w, h);
            }
            let px = buf.pixels();
            // `FillImageRGB2BW`: `R * 0.2989 + G * 0.5870 + B * 0.1140` in float32, times the
            // alpha when `alpha_mode` is STRAIGHT or PREMUL.
            let times_alpha = matches!(im.alpha_mode.as_str(), "STRAIGHT" | "PREMUL");
            for (dst, f) in &ex.fills {
                match f {
                    Fill::Image(i, src) if i == r => {
                        let (d, s) = (usize::from(*dst), usize::from(*src));
                        for (o, p) in out.chunks_exact_mut(4).zip(px.chunks_exact(4)) {
                            o[d] = p[s];
                        }
                    }
                    Fill::ImageBw(i) if i == r => {
                        let d = usize::from(*dst);
                        for (o, p) in out.chunks_exact_mut(4).zip(px.chunks_exact(4)) {
                            let mut v = p[0] * 0.2989f32 + p[1] * 0.5870f32 + p[2] * 0.1140f32;
                            if times_alpha {
                                v *= p[3];
                            }
                            o[d] = v;
                        }
                    }
                    _ => {}
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
    if target == Target::Jpeg {
        channels = 3;
    }
    let format = Format::of(target, false, g.settings.export_image_quality);
    if channels == 3 && alpha {
        bytes.chunks_exact_mut(4).for_each(under_black_byte);
    }
    let img =
        DecodedImage::new(w, h, 4, PixelData::U8(bytes)).and_then(|i| i.with_channels(channels));
    let r = img.and_then(|i| encode_pixels(&i, &format));
    let names: Vec<&Image> = images.iter().filter_map(|r| main.images.get(*r)).collect();
    let name = images_name(&names);
    finish_encode(g, &name, target, r)
}

/// `grayscale_anisotropy_calculation` (WP 8.12), ported from blender/scripts/addons_core/
/// io_scene_gltf2/blender/exp/material/extensions/anisotropy.py (io_scene_gltf2 5.2.41,
/// Apache-2.0, The glTF-Blender-IO authors): the size of the largest image, each image
/// (scaled to it) as its grey value `0.2989 R + 0.5870 G + 0.1140 B` (an unlinked socket: 1);
/// B = the strength, R, G = `(cos, sin)` of the rotation × 2π, normalised, then `(x + 1) / 2`;
/// A = 1. In `float32` as the add-on's numpy arrays, except that an unlinked rotation is the
/// integer 1 (`np.full(…, 1)`), so its angle `2π` and the direction are in double precision.
/// Written without alpha (`images.new(alpha=False)`: no Alpha fill), named `Image`.
fn encode_anisotropy(
    g: &mut Gather,
    strength: Option<IdRef<ImageId>>,
    rotation: Option<IdRef<ImageId>>,
    target: Target,
) -> Result<Option<Vec<u8>>, ExportError> {
    let main = g.main;
    let images: Vec<IdRef<ImageId>> = [strength, rotation].into_iter().flatten().collect();
    let (w, h) = if images.is_empty() {
        (1, 1)
    } else {
        let sizes: Vec<(u32, u32)> = images
            .iter()
            .map(|r| main.images.get(*r).map_or((0, 0), |im| image_size(g, im)))
            .collect();
        (
            sizes.iter().map(|s| s.0).max().unwrap_or(1),
            sizes.iter().map(|s| s.1).max().unwrap_or(1),
        )
    };
    frap_math::budget::check_buffer(
        frap_scene::id::imbuf::BUFFER,
        frap_scene::id::imbuf::buffer_bytes(w, h, true),
    );
    let n = w as usize * h as usize;
    let gray = |g: &Gather, r: Option<IdRef<ImageId>>| -> Option<Option<Vec<f32>>> {
        let Some(r) = r else { return Some(None) };
        let im = main.images.get(r)?;
        let mut buf = acquire(g, im)?;
        if (buf.x, buf.y) != (w, h) {
            buf.scale(w, h);
        }
        let px = buf.pixels();
        Some(Some(
            px.chunks_exact(4)
                .map(|p| 0.2989f32 * p[0] + 0.5870f32 * p[1] + 0.1140f32 * p[2])
                .collect(),
        ))
    };
    let Some(s) = gray(g, strength) else {
        return Ok(None);
    };
    let Some(rt) = gray(g, rotation) else {
        return Ok(None);
    };
    let mut out = vec![0.0f32; n * 4];
    for (k, o) in out.chunks_exact_mut(4).enumerate() {
        o[3] = 1.0;
        o[2] = s.as_ref().map_or(1.0, |v| v[k]);
        let (x, y) = match &rt {
            Some(v) => {
                let a = v[k] * 2.0f32 * std::f32::consts::PI;
                let (c, sn) = (a.cos(), a.sin());
                let norm = (c * c + sn * sn).sqrt();
                ((c / norm + 1.0) / 2.0, (sn / norm + 1.0) / 2.0)
            }
            None => {
                let a = 2.0 * std::f64::consts::PI;
                let (c, sn) = (a.cos(), a.sin());
                let norm = (c * c + sn * sn).sqrt();
                (
                    ((c / norm + 1.0) / 2.0) as f32,
                    ((sn / norm + 1.0) / 2.0) as f32,
                )
            }
        };
        o[0] = x;
        o[1] = y;
    }
    let mut bytes: Vec<u8> = out.iter().map(|v| unit_to_u8(*v)).collect();
    bytes = ImBuf::flip_rows(&bytes, w as usize * 4);
    let format = Format::of(target, false, g.settings.export_image_quality);
    let img = DecodedImage::new(w, h, 4, PixelData::U8(bytes)).and_then(|i| i.with_channels(3));
    let r = img.and_then(|i| encode_pixels(&i, &format));
    finish_encode(g, "Image", target, r)
}

/// The encoded bytes of `ex` as `target` (cached per description and target; `None` with a
/// WARNING when the image cannot be written).
pub(crate) fn encode(
    g: &mut Gather,
    ex: &ExportImage,
    target: Target,
) -> Result<Option<Arc<[u8]>>, ExportError> {
    let key = (ex.clone(), target);
    if let Some(b) = g.images.encoded.get(&key) {
        return Ok(b.clone());
    }
    let main = g.main;
    let b = match ex.happy() {
        Some(r) => match main.images.get(r) {
            Some(im) => encode_happy(g, im, target)?,
            None => None,
        },
        None => encode_unhappy(g, ex, target)?,
    };
    let b: Option<Arc<[u8]>> = b.filter(|b| !b.is_empty()).map(Arc::from);
    g.images.encoded.insert(key, b.clone());
    Ok(b)
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
            Role::Alpha | Role::AlphaBw => {
                if i.role == Role::AlphaBw {
                    ex.fill(3, Fill::ImageBw(img));
                } else {
                    ex.fill(3, Fill::Image(img, src(i.tex, 3)));
                }
                for c in 0..3 {
                    if !ex.is_filled(c) {
                        ex.fill(c, Fill::White);
                    }
                }
            }
            Role::Channel(dst) => ex.fill(dst, Fill::Image(img, src(i.tex, dst))),
            Role::AnisotropyStrength => {
                ex.anisotropy.get_or_insert((None, None)).0 = Some(img);
            }
            Role::AnisotropyRotation => {
                ex.anisotropy.get_or_insert((None, None)).1 = Some(img);
            }
        }
    }
    // "Check that we don't have some empty channels".
    let main = g.main;
    let mut empty = Vec::new();
    for (_, f) in &ex.fills {
        if let Fill::Image(r, _) | Fill::ImageBw(r) = f
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
            .retain(|(_, f)| !matches!(f, Fill::Image(i, _) | Fill::ImageBw(i) if *i == r));
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
    let i = gather_new_texture(g, inputs, usage.1)?;
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

/// Whether a texture of `inputs` gets an image (`gather_texture_info(...) is not None`, the
/// ORM double check of `__gather_orm_texture`, WP 8.12): its fills are not empty and encode.
pub fn texture_writable(g: &mut Gather, inputs: &[TexInput]) -> Result<bool, ExportError> {
    if g.settings.props.export_keep_originals.value {
        return Ok(true);
    }
    let ex = compose(g, inputs);
    if ex.fills.is_empty() {
        return Ok(false);
    }
    let happy = ex.happy().and_then(|r| g.main.images.get(r));
    let alpha = inputs.iter().any(|i| i.role.is_alpha());
    let mime = mime_for(g.settings, happy, alpha);
    Ok(encode(g, &ex, Target::from_mime(mime))?.is_some())
}

/// The WARNING of a texture gathered from more than one socket (`__gather_sampler`).
pub const SAMPLER_OF_FIRST_NODE: &str = "More than one shader node tex image used for a texture. \
     The resulting glTF sampler will behave like the first shader node tex image.";

fn gather_new_texture(
    g: &mut Gather,
    inputs: &[TexInput],
    slot: TextureSlot,
) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let Some(first) = inputs.first() else {
        return Ok(None);
    };
    // `texture.py` `__gather_sampler`: a texture gathered from several sockets (metallic and
    // roughness, base colour and alpha, an ORM image), even when one image node feeds them
    // all (verified on 5.2.4).
    if inputs.len() > 1 {
        g.warn(SAMPLER_OF_FIRST_NODE);
    }
    if g.settings.props.export_keep_originals.value {
        return gather_original_texture(g, inputs);
    }
    let ex = compose(g, inputs);
    if ex.fills.is_empty() && ex.anisotropy.is_none() {
        return Ok(None);
    }
    // `exported_images[name] = 1 / 2` (`encode_image.py`): fully or partly used.
    for r in ex.images() {
        if let Some(im) = main.images.get(r) {
            g.images.exported.insert(im.header.name.to_string());
        }
    }
    let happy = ex.happy().and_then(|r| main.images.get(r));
    let alpha = inputs.iter().any(|i| i.role.is_alpha());
    let mime = mime_for(g.settings, happy, alpha);
    let names: Vec<&Image> = ex
        .images()
        .iter()
        .filter_map(|r| main.images.get(*r))
        .collect();
    let name = images_name(&names);
    // `image.gather_image`. Encode first: an image that cannot be written leaves no sampler
    // behind (`gather_texture`: "most viewers can't handle missing source properties")…
    let webp_format = g.settings.export_image_format == ExportImageFormat::Webp;
    let source = match encode(g, &ex, Target::from_mime(mime))? {
        Some(bytes) => Some(make_image(g, bytes, mime, name.clone())),
        None => {
            g.warn(format!(
                "image '{name}': Image data is empty, not exporting image"
            ));
            if !webp_format {
                return Ok(None);
            }
            None
        }
    };
    // `texture.py`: `__gather_source` and `__gather_extensions` (EXT_texture_webp), then frap's
    // KHR_texture_basisu.
    let (source, webp) = match source {
        Some(s) => crate::export::images_webp::gather(g, &ex, s)?,
        None => (None, None),
    };
    // …except with `export_image_format='WEBP'`: `__gather_extensions` sets the extension's
    // `source` to the missing image (`None`, dropped when written) and removes the texture's,
    // so the texture is written with its sampler, no image, and EXT_texture_webp used and
    // required (verified on 5.2.4 with a greyscale PNG: `{"extensions": {}, "sampler": 0}`).
    if webp_format && webp.is_none() {
        g.use_extension(frap_gltf::ext::ext_texture_webp::ExtTextureWebp::NAME, true);
    }
    let basisu = crate::export::images_ktx2::gather(g, &ex, slot, first.tex, name)?;
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
    // The traversal numbers the images: the texture's `extensions` before its `source`.
    let mut texture = Texture {
        sampler: Some(sampler_index),
        ..Texture::default()
    };
    if let Some((img, required)) = webp {
        let i = push_image(g, img);
        texture
            .extensions
            .set(Some(frap_gltf::ext::ext_texture_webp::ExtTextureWebp {
                source: Some(i),
                ..Default::default()
            }));
        g.use_extension(
            frap_gltf::ext::ext_texture_webp::ExtTextureWebp::NAME,
            required,
        );
    }
    if let Some(img) = basisu {
        let i = push_image(g, img);
        texture
            .extensions
            .set(Some(frap_gltf::ext::khr_texture_basisu::KhrTextureBasisu {
                source: Some(i),
                ..Default::default()
            }));
        // Required when the texture has no core `source` to fall back on (the extension's
        // specification: `source` may be omitted only when it is required).
        g.use_extension(
            frap_gltf::ext::khr_texture_basisu::KhrTextureBasisu::NAME,
            source.is_none(),
        );
        if source.is_none() {
            // M8 W0 (decision log M8W0-7): a required KHR_texture_basisu shuts out every
            // reader without KTX2 support, Blender 5.2 included (D-133), so say so.
            g.warn(KTX2_REQUIRED_WARNING);
        }
    }
    texture.source = source.map(|s| push_image(g, s));
    g.doc.textures.push(texture);
    Ok(Some((g.doc.textures.len() - 1) as Index))
}

/// `os.path.relpath(path, start)` on absolute, lexically normalised paths (`None` without a
/// common root, as on Windows across drives), with `/` separators.
fn relpath(path: &std::path::Path, start: &std::path::Path) -> Option<String> {
    use std::path::Component;
    fn norm(p: &std::path::Path) -> Vec<Component<'_>> {
        let mut out: Vec<Component> = Vec::new();
        for c in p.components() {
            match c {
                Component::CurDir => {}
                Component::ParentDir => {
                    if matches!(out.last(), Some(Component::Normal(_))) {
                        out.pop();
                    }
                }
                c => out.push(c),
            }
        }
        out
    }
    let (p, s) = (norm(path), norm(start));
    if p.first() != s.first() {
        return None;
    }
    let common = p.iter().zip(&s).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_owned(); s.len() - common];
    parts.extend(
        p[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    Some(if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    })
}

/// `export_keep_originals` (WP 8.12), ported from blender/scripts/addons_core/io_scene_gltf2/
/// blender/exp/material/image.py (`__get_image_data_mapping`: `ExportImage.from_original` of
/// each socket's image, the last one kept; `__gather_mime_type`: PNG (WebP) for an Alpha
/// socket, else the original file's format with `AUTO`; `__gather_name`: the image's name;
/// `__gather_original_uri`: the file's path relative to the exported file's directory, the
/// image dropped when the file does not exist; `__gather_buffer_view`: outside
/// `GLTF_SEPARATE`, the encode of the fill-less `ExportImage`, a 1×1 white pixel (written next
/// to the URI)) and texture.py (`__gather_extensions`: a WebP original goes into
/// `EXT_texture_webp`, required, without a core `source`), io_scene_gltf2 5.2.41, Apache-2.0,
/// The glTF-Blender-IO authors. A texture without an image is not written.
fn gather_original_texture(
    g: &mut Gather,
    inputs: &[TexInput],
) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let (Some(first), Some(last)) = (inputs.first(), inputs.last()) else {
        return Ok(None);
    };
    let Some(orig) = main.images.get(last.tex.image) else {
        return Ok(None);
    };
    let alpha = inputs.iter().any(|i| i.role.is_alpha());
    let mime = if alpha {
        if g.settings.export_image_format == ExportImageFormat::Webp {
            WEBP
        } else {
            PNG
        }
    } else {
        match g.settings.export_image_format {
            ExportImageFormat::Webp => WEBP,
            ExportImageFormat::Jpeg => JPEG,
            _ if is_jpeg_image(orig) => JPEG,
            _ if is_webp_image(orig) => WEBP,
            _ => PNG,
        }
    };
    let file = (!orig.filepath.is_empty()).then(|| resolve(g, &orig.filepath));
    let start = match (&g.settings.out_dir, &g.settings.cwd) {
        (Some(d), _) => d.clone(),
        (None, Some(c)) => c.clone(),
        (None, None) => std::env::current_dir().unwrap_or_default(),
    };
    let uri = file
        .filter(|f| f.exists())
        .and_then(|f| relpath(&f, &start))
        .map(|r| path_to_uri(&r));
    let Some(uri) = uri else {
        return Ok(None);
    };
    let name = orig.header.name.to_string();
    let bytes = if g.settings.export_format == ExportFormat::GltfSeparate {
        None
    } else {
        encode(g, &ExportImage::default(), Target::from_mime(mime))?
    };
    let key = (
        bytes.clone().unwrap_or_else(|| Arc::from(Vec::new())),
        mime,
        format!("{name}\u{0}{uri}"),
    );
    let image = match g.images.images.get(&key) {
        Some(i) => *i,
        None => {
            let img = GImage {
                mime_type: Some(mime.to_owned()),
                name: Some(name),
                uri: Some(uri),
                buffer_view: bytes.map(|b| g.buf.push_view(&b, 4, None, None)),
                ..GImage::default()
            };
            g.doc.images.push(img);
            let i = (g.doc.images.len() - 1) as Index;
            g.images.images.insert(key, i);
            i
        }
    };
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
    let mut texture = Texture {
        sampler: Some(sampler_index),
        ..Texture::default()
    };
    if mime == WEBP {
        texture
            .extensions
            .set(Some(frap_gltf::ext::ext_texture_webp::ExtTextureWebp {
                source: Some(image),
                ..Default::default()
            }));
        g.use_extension(frap_gltf::ext::ext_texture_webp::ExtTextureWebp::NAME, true);
    } else {
        texture.source = Some(image);
    }
    g.doc.textures.push(texture);
    Ok(Some((g.doc.textures.len() - 1) as Index))
}

/// The WARNING of an export that makes `KHR_texture_basisu` required (M8 W0, decision log
/// M8W0-7): once per export.
pub const KTX2_REQUIRED_WARNING: &str = "frap: KHR_texture_basisu is required in this file (a \
     texture has no PNG, JPEG or WebP image to fall back on, e.g. export_image_format='WEBP' \
     without export_image_webp_fallback): Blender 5.2 and other readers without KTX2 support \
     cannot load it (deviation D-133)";

/// An image gathered from encoded `bytes`: `GLTF_SEPARATE` takes its file name now
/// (`set_real_uri`: every gathered image takes a name, written or not).
pub(crate) fn make_image(
    g: &mut Gather,
    bytes: Arc<[u8]>,
    mime: &'static str,
    name: String,
) -> PendingImage {
    let separate = g.settings.export_format == ExportFormat::GltfSeparate;
    let rel = separate.then(|| {
        // `ImageData.file_extension` (`.ktx2` for frap's KTX2).
        let ext = match mime {
            JPEG => ".jpg",
            WEBP => ".webp",
            KTX2 => ".ktx2",
            _ => ".png",
        };
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
    PendingImage {
        bytes,
        mime,
        name,
        rel,
    }
}

/// The image index of a gathered image (`__make_image`, shared by bytes, MIME type and name;
/// a shared image keeps the first one's file).
fn push_image(g: &mut Gather, p: PendingImage) -> Index {
    let key = (p.bytes.clone(), p.mime, p.name.clone());
    if let Some(i) = g.images.images.get(&key) {
        return *i;
    }
    let i = push_image_unshared(g, p);
    g.images.images.insert(key, i);
    i
}

/// A new glTF image, never shared: `traverse_additional_images` gathers each `bpy.data`
/// image on its own, and its `Image` (its own file or buffer view) equals no other in
/// `__append_unique_and_get_index` (verified on 5.2.4: two images with the same pixels and
/// name are written twice, `paint.png` and `paint-1.png`).
fn push_image_unshared(g: &mut Gather, p: PendingImage) -> Index {
    let mut img = GImage {
        mime_type: Some(p.mime.to_owned()),
        name: Some(p.name),
        ..GImage::default()
    };
    match p.rel {
        Some(rel) => {
            img.uri = Some(path_to_uri(&rel));
            g.files.push((rel, p.bytes.to_vec()));
        }
        None => img.buffer_view = Some(g.buf.push_view(&p.bytes, 4, None, None)),
    }
    g.doc.images.push(img);
    (g.doc.images.len() - 1) as Index
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
    use frap_material::{Sampler as LSampler, TexTransform, UvSource};
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
        let enc = |im: &Image, buf: &ImBuf, t| encode_copy(im, buf, t, 75).unwrap().bytes;
        let jpeg = enc(&im, &buf, Target::Jpeg);
        assert!(is_jpeg(&jpeg));
        let [r, g, b] = first_rgb(&jpeg);
        for (got, want) in [(r, 179u8), (g, 27), (b, 78)] {
            assert!(got.abs_diff(want) <= 3, "({r}, {g}, {b})");
        }
        // PNG keeps the alpha channel and the straight colour.
        let png = enc(&im, &buf, Target::Png);
        let d = frap_imbuf::decode(&png).unwrap();
        assert_eq!(d.channels, 4);
        // So does WebP (lossless at quality 100: the PNG's samples).
        let webp = encode_copy(&im, &buf, Target::Webp, 100).unwrap().bytes;
        assert!(is_webp(&webp) && webp.get(12..16) == Some(b"VP8L"));
        assert_eq!(frap_imbuf::decode(&webp).unwrap(), d);
        // A 24-plane buffer whose pixels carry alpha (`pixels = …` on an image made without
        // alpha) is composited over black, as Blender does (0.7 × 0.2 = 0.14 → 36).
        let mut im = Image::new("N", 4, 4);
        im.generated_color = [0.7, 0.1, 0.3, 0.2];
        let mut buf = im.generated_buffer().unwrap();
        buf.planes = 24;
        for bytes in [
            enc(&im, &buf, Target::Png),
            enc(&im, &buf, Target::Jpeg),
            enc(&im, &buf, Target::Webp),
        ] {
            let [r, _, _] = first_rgb(&bytes);
            assert!(r.abs_diff(36) <= 3, "{r}");
        }
    }

    #[test]
    fn webp_quality_selects_lossless_or_lossy() {
        let mut im = Image::new("Q", 8, 8);
        im.generated_color = [0.2, 0.4, 0.6, 1.0];
        let buf = im.generated_buffer().unwrap();
        let lossless = encode_copy(&im, &buf, Target::Webp, 100).unwrap();
        assert!(!lossless.lossless_fallback);
        assert_eq!(lossless.bytes.get(12..16), Some(&b"VP8L"[..]));
        // Below 100: lossy (libwebp), or lossless with the fallback flag without the feature.
        let lossy = encode_copy(&im, &buf, Target::Webp, 75).unwrap();
        let chunk = lossy.bytes.get(12..16).unwrap();
        if lossy.lossless_fallback {
            assert_eq!(chunk, b"VP8L");
        } else {
            assert_eq!(chunk, b"VP8 ");
        }
    }

    #[test]
    fn targets_accept_their_own_magic_numbers() {
        assert_eq!(Target::from_mime(PNG), Target::Png);
        assert_eq!(Target::from_mime(JPEG), Target::Jpeg);
        assert_eq!(Target::from_mime(WEBP), Target::Webp);
        assert!(Target::Png.accepts(b"\x89PNG\r\n"));
        assert!(Target::Webp.accepts(b"RIFF\0\0\0\0WEBPVP8L"));
        assert!(!Target::Webp.accepts(b"\x89PNG\r\n"));
        // A KTX2 file is copied only when it keeps the KHR_texture_basisu rules for the
        // texture's colour space (a UASTC sRGB 32x32 grid).
        let grid: &[u8] =
            include_bytes!("../../../../tests/golden/fixtures/gltf/textures/ktx2/grid_uastc.ktx2");
        let k = Target::Ktx2(BasisOptions::new(frap_imbuf::basis::BasisMode::Etc1s));
        assert!(k.accepts(grid));
        assert!(!k.accepts(&frap_imbuf::ktx2::IDENTIFIER));
        assert!(!k.accepts(b"\x89PNG"));
        let linear =
            Target::Ktx2(BasisOptions::new(frap_imbuf::basis::BasisMode::Etc1s).with_srgb(false));
        assert!(!linear.accepts(grid), "sRGB data for a non-colour texture");
        let mut odd = grid.to_vec();
        odd[20..24].copy_from_slice(&30u32.to_le_bytes());
        assert!(!k.accepts(&odd), "pixelWidth 30");
    }

    /// A greyscale (8-plane) buffer cannot be written as WebP (Blender 5.2.4: OpenImageIO
    /// "webp does not support 1-channel images"); PNG, JPEG and grey-alpha WebP can.
    #[test]
    fn grey_buffers_are_not_written_as_webp() {
        let mut im = Image::new("G", 4, 4);
        im.generated_color = [0.5, 0.5, 0.5, 1.0];
        let mut buf = im.generated_buffer().unwrap();
        buf.planes = 8;
        let e = encode_copy(&im, &buf, Target::Webp, 75).unwrap_err();
        assert_eq!(e, ImageError::Encode(WEBP_GREY_ERROR.into()));
        assert!(is_png(
            &encode_copy(&im, &buf, Target::Png, 75).unwrap().bytes
        ));
        assert!(is_jpeg(
            &encode_copy(&im, &buf, Target::Jpeg, 75).unwrap().bytes
        ));
        buf.planes = 16;
        assert!(is_webp(
            &encode_copy(&im, &buf, Target::Webp, 75).unwrap().bytes
        ));
    }

    #[test]
    fn webp_mime_rules() {
        // `__gather_mime_type`: WEBP format writes WebP, alpha included; AUTO keeps WebP files.
        let webp = ExportSettings {
            export_image_format: ExportImageFormat::Webp,
            ..ExportSettings::default()
        };
        let png_file = Image::new("P", 1, 1);
        assert_eq!(mime_type(&webp, &png_file, ChannelSel::Rgb), WEBP);
        assert_eq!(mime_type(&webp, &png_file, ChannelSel::Rgba), WEBP);
        let auto = ExportSettings::default();
        let mut w = Image::new("W", 1, 1);
        w.source = "FILE".into();
        w.filepath = "//t/w.WEBP".into();
        assert_eq!(mime_type(&auto, &w, ChannelSel::Rgb), WEBP);
        assert_eq!(mime_type(&auto, &w, ChannelSel::Rgba), WEBP);
        // JPEG format: JPEG even for a WebP file; alpha: the WebP file stays WebP.
        let jpeg = ExportSettings {
            export_image_format: ExportImageFormat::Jpeg,
            ..ExportSettings::default()
        };
        assert_eq!(mime_type(&jpeg, &w, ChannelSel::Rgb), JPEG);
        assert_eq!(mime_type(&jpeg, &w, ChannelSel::Rgba), WEBP);
        // A packed WebP without a file path, by its magic number.
        w.filepath.clear();
        w.packed_file = Some(Arc::new(EncodedImage {
            bytes: b"RIFF\0\0\0\0WEBPVP8L".to_vec(),
            mime: "image/webp".into(),
        }));
        assert_eq!(mime_type(&auto, &w, ChannelSel::Rgb), WEBP);
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
