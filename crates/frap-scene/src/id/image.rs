// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Image` (§5.6). WP 3.2 owns the bindings and the image buffer model.
//!
//! Blender keeps an `ImBuf` cache per image: loaded lazily from the source (packed bytes, the
//! file, or the generator), changed in place by `pixels` and `scale()`, written by `pack()` and
//! `save()`, and freed by `reload()` and by changes of the generator settings or the source.
//! frap keeps the same model with one difference: the buffer is stored in the datablock
//! ([`Image::buffer`]) only once it holds something the source cannot reproduce (it was
//! modified, or it was saved with a different precision); otherwise pixels are derived from the
//! source on demand by [`Image::acquire`] (packed bytes and files are decoded with the decoder
//! `frap-ops-data` registers, then converted with the image's `colorspace_settings` and
//! `alpha_mode` as Blender's loader does; generated images are generated). Nothing caches them:
//! every `acquire` decodes or generates again, so the RNA reads that do not need pixels
//! (`size`, `depth`, `channels`, `is_float`, `has_data`) use [`Image::info`] instead, which
//! reads the generator settings, the stored buffer or the file's header only.
//!
//! Limits: every buffer is checked against the statement's budget before it is allocated
//! (`--max-elements` × `frap_math::budget::BUFFER_BYTES_PER_ELEMENT` bytes, `LIMIT_EXCEEDED`),
//! and generating, decoding, converting and scaling are cancellation points of
//! `--stmt-timeout`.
//!
//! Verified on 5.2.4 (`-b --factory-startup`):
//! - `generated_width/height` clamp to 1..=65536, `generated_color` components to ≥ 0; changing
//!   any generator setting frees the buffer (`IMA_SIGNAL_FREE`): modified pixels are lost and
//!   the image is generated again with the new settings;
//! - `filepath` frees the buffer (`IMA_SIGNAL_RELOAD`), `filepath_raw` does not;
//! - `source` frees the buffer (`IMA_SIGNAL_SRC_CHANGE`); switching to `GENERATED` clears the
//!   file path;
//! - `alpha_mode` frees the buffer of an image that is not GENERATED, `colorspace_settings`
//!   (`name`, `is_data`) the buffer of any image (`IMA_SIGNAL_COLORMANAGE`). frap drops a
//!   stored buffer when the colour space settings no longer match the ones it was loaded with
//!   ([`Image::live_buffer`]): writing the *same* colour space again keeps modified pixels,
//!   where Blender loses them;
//! - `size` is the buffer's size (`(0, 0)` when no pixels can be loaded), `type` is `UV_TEST`
//!   for a generated image and `IMAGE` once it was packed or saved.

pub mod generate;
pub mod imbuf;
pub mod scale;

use std::sync::Arc;

use frap_anim::AnimData;
use frap_rna::*;
use smol_str::SmolStr;

use crate::generated::plain::ColorManagedInputColorspaceSettings;
use generate::GenParams;
pub use imbuf::{ImBuf, ImBufData};

/// What an image's buffer is, or would be once loaded, without its pixels (`Image.size`,
/// `depth`, `channels`, `is_float`, `has_data`, `file_format`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImInfo {
    /// Width (`ibuf->x`).
    pub x: u32,
    /// Height (`ibuf->y`).
    pub y: u32,
    /// Colour mode bits (`ibuf->planes`, see [`ImBuf::planes`]).
    pub planes: u8,
    /// A float buffer (16-bit PNG, OpenEXR, HDR, float generated images).
    pub float: bool,
    /// `ibuf->ftype` as an `Image.file_format` identifier.
    pub ftype: SmolStr,
}

impl ImInfo {
    /// The description of a buffer.
    pub fn of(b: &ImBuf) -> ImInfo {
        ImInfo {
            x: b.x,
            y: b.y,
            planes: b.planes,
            float: b.is_float(),
            ftype: b.ftype.clone(),
        }
    }

    /// `Image.depth` (see [`ImBuf::depth`]).
    pub fn depth(&self) -> i32 {
        imbuf::depth_of(self.planes, self.float)
    }
}

/// Encoded (file) bytes of a packed image (`image.packed_file`), as read or embedded in a GLB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedImage {
    /// The encoded file (PNG, JPEG, WebP, …).
    pub bytes: Vec<u8>,
    /// MIME type ("image/png").
    pub mime: SmolStr,
}

/// Image.
#[derive(Clone, Debug)]
pub struct Image {
    /// ID header.
    pub header: IdHeader,
    /// `Image.source` ("FILE", "GENERATED", "VIEWER", …)
    pub source: SmolStr,
    /// `Image.filepath` (as given; `//` is relative to `bpy.data.filepath`)
    pub filepath: String,
    /// `Image.packed_file` (shared bytes, never copied by a COW clone)
    pub packed_file: Option<Arc<EncodedImage>>,
    /// `Image.packed_files[0].filepath` (`ImagePackedFile.filepath`): the file the packed bytes
    /// came from (`''` for an image packed from memory without a path).
    pub packed_filepath: String,
    /// `Image.file_format` ("PNG")
    pub file_format: SmolStr,
    /// `Image.generated_type` ("BLANK", "UV_GRID", "COLOR_GRID")
    pub generated_type: SmolStr,
    /// `Image.generated_width`
    pub generated_width: i32,
    /// `Image.generated_height`
    pub generated_height: i32,
    /// `Image.generated_color` (linear RGBA)
    pub generated_color: [f32; 4],
    /// `Image.use_generated_float`
    pub use_generated_float: bool,
    /// `ImageTile.gen_depth`: the colour depth of the generated buffer (24 RGB, 32 RGBA), from
    /// the `alpha` argument of `images.new` / `image.new`; reported by `Image.depth`.
    pub generated_depth: u8,
    /// `Image.colorspace_settings`
    pub colorspace_settings: ColorManagedInputColorspaceSettings,
    /// `Image.alpha_mode` ("STRAIGHT", "PREMUL", "CHANNEL_PACKED", "NONE")
    pub alpha_mode: SmolStr,
    /// `Image.size` of a FILE image, read from the file when it was loaded (`[0, 0]` if it
    /// could not be read). Generated images and images with a buffer report the buffer's size.
    pub size: [i32; 2],
    /// The image buffer when it differs from what the source gives (see the module
    /// documentation); `None` when pixels come from the source. Not saved in `.frap` files
    /// (Blender does not save unpacked pixel edits in `.blend` files either).
    pub buffer: Option<Arc<ImBuf>>,
    /// `Image.animation_data`
    pub animation_data: Option<AnimData>,
}

impl Image {
    /// A generated BLANK image with the values of `bpy.data.images.new(name, w, h)` (verified
    /// on 5.2.4: black, alpha 1, sRGB, STRAIGHT, depth 24).
    pub fn new(name: &str, width: i32, height: i32) -> Image {
        Image {
            header: IdHeader::new(name),
            source: "GENERATED".into(),
            filepath: String::new(),
            packed_file: None,
            packed_filepath: String::new(),
            file_format: "PNG".into(),
            generated_type: "BLANK".into(),
            generated_width: width,
            generated_height: height,
            generated_color: [0.0, 0.0, 0.0, 1.0],
            use_generated_float: false,
            generated_depth: 24,
            colorspace_settings: ColorManagedInputColorspaceSettings::default(),
            alpha_mode: "STRAIGHT".into(),
            size: [width, height],
            buffer: None,
            animation_data: None,
        }
    }

    /// `BKE_image_is_dirty` (`Image.is_dirty`).
    pub fn is_dirty(&self) -> bool {
        self.live_buffer().is_some_and(|b| b.dirty)
    }

    /// The stored buffer while it is valid: a buffer loaded with other colour space settings
    /// than the image's current ones counts as freed (`IMA_SIGNAL_COLORMANAGE`).
    pub fn live_buffer(&self) -> Option<&Arc<ImBuf>> {
        self.buffer.as_ref().filter(|b| {
            b.colorspace
                .as_ref()
                .is_none_or(|c| *c == self.colorspace_settings)
        })
    }

    /// `BKE_image_free_buffers`: drops the in-memory buffer (modified pixels are lost).
    pub fn free_buffers(&mut self) {
        self.buffer = None;
    }

    /// The colour space is a data space (`IMB_colormanagement_space_name_is_data`).
    pub fn is_data_colorspace(&self) -> bool {
        self.colorspace_settings.is_data || self.colorspace_settings.name == "Non-Color"
    }

    /// The buffer of a GENERATED image (`add_ibuf_for_tile`); `None` for other sources.
    pub fn generated_buffer(&self) -> Option<ImBuf> {
        (self.source == "GENERATED").then(|| generate::generate(&self.gen_params()))
    }

    /// The description of the generated buffer (what [`Self::generated_buffer`] gives).
    fn generated_info(&self) -> ImInfo {
        ImInfo {
            x: self.generated_width.max(1) as u32,
            y: self.generated_height.max(1) as u32,
            planes: self.generated_depth,
            float: self.use_generated_float,
            ftype: SmolStr::new_static("PNG"),
        }
    }

    /// The first tile is generated (`IMA_GEN_TILE`): a tiled image made by
    /// `images.new(tiled=True)` / `image.new(tiled=True)` until it is saved or packed.
    fn tile_is_generated(&self) -> bool {
        self.source == "TILED" && self.packed_file.is_none() && self.filepath.is_empty()
    }

    /// `rna_Image_alpha_mode_update`: frees the buffer (`IMA_SIGNAL_COLORMANAGE`) unless the
    /// image is GENERATED ("it doesn't impact generated images").
    fn set_alpha_mode(&mut self, v: SmolStr) {
        self.alpha_mode = v;
        if self.source != "GENERATED" {
            self.free_buffers();
        }
    }

    /// The generator settings of the first tile.
    fn gen_params(&self) -> GenParams<'_> {
        GenParams {
            kind: &self.generated_type,
            width: self.generated_width,
            height: self.generated_height,
            float: self.use_generated_float,
            depth: self.generated_depth,
            color: self.generated_color,
            is_data: self.is_data_colorspace(),
        }
    }

    /// `Image.size` without decoding: the buffer's size, the generator's for a GENERATED
    /// image, else the size read when the file was loaded.
    pub fn known_size(&self) -> [i32; 2] {
        if let Some(b) = self.live_buffer() {
            return [b.x as i32, b.y as i32];
        }
        if self.source == "GENERATED" {
            return [self.generated_width.max(1), self.generated_height.max(1)];
        }
        self.size
    }

    /// `Image.filepath` setter: `rna_Image_reload_update` (`IMA_SIGNAL_RELOAD`) frees the
    /// buffer.
    fn set_filepath(&mut self, v: String) {
        self.filepath = v;
        self.free_buffers();
    }

    /// `rna_Image_source_set` (`IMA_SIGNAL_SRC_CHANGE`).
    fn set_source(&mut self, v: SmolStr) {
        if v == self.source {
            return;
        }
        if v == "GENERATED" {
            if self.generated_width == 0 || self.generated_height == 0 {
                let [w, h] = self.known_size();
                self.generated_width = w;
                self.generated_height = h;
            }
            // "Ensure original image path wouldn't be used when saving generated image."
            self.filepath.clear();
        }
        self.source = v;
        self.free_buffers();
    }
}

/// A file decoder: encoded bytes → image buffer (bottom row first, four channels, straight
/// samples in the file's colour space: [`Image::acquire`] applies the image's colour space and
/// alpha mode), `Err` with the reason when the bytes are not a readable image. frap-scene has
/// no codecs (layer rule): `frap-ops-data` registers the `frap-imbuf` decoder here (one entry
/// is used, the first).
pub type ImageDecoder = fn(&[u8]) -> Result<ImBuf, String>;

/// Registered image decoders (see [`ImageDecoder`]).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static IMAGE_DECODERS: [ImageDecoder];

/// A header reader: what [`ImageDecoder`] would give, without decoding the pixels.
/// `frap-ops-data` registers the `frap-imbuf` one (the first entry is used).
pub type ImageProbe = fn(&[u8]) -> Result<ImInfo, String>;

/// Registered header readers (see [`ImageProbe`]).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static IMAGE_PROBES: [ImageProbe];

/// Reads the header of encoded image bytes (decodes them when no header reader is linked).
pub fn probe_image(bytes: &[u8]) -> Result<ImInfo, String> {
    match IMAGE_PROBES.first() {
        Some(p) => p(bytes),
        None => decode_raw(bytes).map(|b| ImInfo::of(&b)),
    }
}

/// Decodes encoded image bytes with the registered decoder: straight samples in the file's
/// colour space (see [`ImageDecoder`]). The size read from the header is checked against the
/// statement's budget first (`LIMIT_EXCEEDED` instead of allocating).
pub fn decode_image(bytes: &[u8]) -> Result<ImBuf, String> {
    if !IMAGE_PROBES.is_empty() {
        let info = probe_image(bytes)?;
        frap_math::budget::check_buffer(
            imbuf::BUFFER,
            imbuf::buffer_bytes(info.x, info.y, info.float),
        );
    }
    decode_raw(bytes)
}

fn decode_raw(bytes: &[u8]) -> Result<ImBuf, String> {
    match IMAGE_DECODERS.first() {
        Some(d) => d(bytes),
        None => Err("no image decoder is linked".into()),
    }
}

/// Reads the header of an image file: the first MiB (enough for the headers of every format
/// frap reads), the whole file when that is not enough.
fn probe_file(path: &std::path::Path) -> Option<ImInfo> {
    use std::io::Read;
    const HEAD: u64 = 1 << 20;
    let mut head = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(HEAD)
        .read_to_end(&mut head)
        .ok()?;
    match probe_image(&head) {
        Ok(i) => Some(i),
        Err(_) if head.len() as u64 == HEAD => probe_image(&std::fs::read(path).ok()?).ok(),
        Err(_) => None,
    }
}

/// Where relative image paths resolve: `bpy.data.filepath` (`//` is relative to its folder)
/// and the session's working directory (other relative paths, and `//` in an unsaved file).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathBase {
    /// `bpy.data.filepath` when saved.
    pub blend: Option<String>,
    /// The session's working directory (`HostRead::cwd`); `None`: the process's.
    pub cwd: Option<std::path::PathBuf>,
}

impl PathBase {
    /// The base of a host's session.
    pub fn of(h: &dyn crate::HostRead) -> PathBase {
        PathBase {
            blend: h.main().filepath.clone(),
            cwd: h.cwd().map(std::path::Path::to_path_buf),
        }
    }

    /// `BLI_path_abs(path, bpy.data.filepath)`, the path Blender prints in reports: `//` is
    /// replaced by the saved file's folder, or dropped for an unsaved file.
    pub fn abs(&self, filepath: &str) -> String {
        use std::path::Path;
        match filepath.strip_prefix("//") {
            Some(rel) => match self.blend.as_deref().and_then(|b| Path::new(b).parent()) {
                Some(dir) => dir.join(rel).to_string_lossy().into_owned(),
                None => rel.to_owned(),
            },
            None => filepath.to_owned(),
        }
    }

    /// The file system path of `filepath`: [`Self::abs`], then relative to the working
    /// directory.
    pub fn file(&self, filepath: &str) -> std::path::PathBuf {
        let abs = std::path::PathBuf::from(self.abs(filepath));
        match &self.cwd {
            Some(cwd) if abs.is_relative() => cwd.join(abs),
            _ => abs,
        }
    }
}

/// Why an image file could not be loaded (`BKE_image_load`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The file cannot be opened: `strerror(errno)` ("No such file or directory").
    Io(String),
}

impl Image {
    /// `BKE_image_acquire_ibuf`: the image's pixels (the buffer, the packed bytes, the file or
    /// the generator); `None` when they cannot be loaded (missing or unreadable file, a VIEWER
    /// image). Relative paths resolve against `base`. Decoded files get the image's colour
    /// space and alpha mode ([`ImBuf::handle_colorspace_and_alpha`]); the result carries the
    /// colour space settings it was made with ([`ImBuf::colorspace`]).
    ///
    /// Within a statement, a buffer larger than the budget unwinds with `LIMIT_EXCEEDED`.
    pub fn acquire(&self, base: &PathBase) -> Option<ImBuf> {
        if let Some(b) = self.live_buffer() {
            return Some((**b).clone());
        }
        let mut buf = match self.source.as_str() {
            // A generated image ignores its packed file (a packed image switched back to
            // GENERATED keeps it, verified on 5.2.4).
            "GENERATED" => self.generated_buffer()?,
            "TILED" if self.tile_is_generated() => generate::generate(&self.gen_params()),
            "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if self.packed_file.is_some() => {
                let p = self.packed_file.as_ref().expect("invariant: checked");
                self.file_buffer(decode_image(&p.bytes).ok()?)
            }
            "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if !self.filepath.is_empty() => {
                let bytes = std::fs::read(base.file(&self.filepath)).ok()?;
                self.file_buffer(decode_image(&bytes).ok()?)
            }
            _ => return None,
        };
        buf.colorspace = Some(self.colorspace_settings.clone());
        Some(buf)
    }

    /// A decoded file converted with the image's colour space and alpha mode.
    fn file_buffer(&self, mut buf: ImBuf) -> ImBuf {
        buf.handle_colorspace_and_alpha(
            &self.colorspace_settings.name,
            self.is_data_colorspace(),
            &self.alpha_mode,
        );
        buf
    }

    /// What [`Self::acquire`] would give, without building the pixels: the stored buffer, the
    /// generator settings, or the header of the packed bytes or the file. `None` when no
    /// pixels can be loaded (`Image.has_data` false).
    pub fn info(&self, base: &PathBase) -> Option<ImInfo> {
        if let Some(b) = self.live_buffer() {
            return Some(ImInfo::of(b));
        }
        match self.source.as_str() {
            "GENERATED" => Some(self.generated_info()),
            "TILED" if self.tile_is_generated() => Some(self.generated_info()),
            "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if self.packed_file.is_some() => {
                let p = self.packed_file.as_ref().expect("invariant: checked");
                probe_image(&p.bytes).ok()
            }
            "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if !self.filepath.is_empty() => {
                probe_file(&base.file(&self.filepath))
            }
            _ => None,
        }
    }

    /// `BKE_image_load` without the datablock insertion: a FILE image named after the file,
    /// with `filepath` as given and the colour space, alpha mode, file format and size read
    /// from the file (`image_init_color_management`). Fails only when the file cannot be
    /// opened: an unreadable file still gives an image (Blender: size `(0, 0)`, no data).
    pub fn load(filepath: &str, base: &PathBase) -> Result<Image, LoadError> {
        let abs = base.file(filepath);
        let bytes = match std::fs::read(&abs) {
            Ok(b) => b,
            // A directory opens (`BLI_open` succeeds on it) but has no image data.
            Err(_) if abs.is_dir() => Vec::new(),
            Err(e) => return Err(LoadError::Io(io_message(&e))),
        };
        let name = basename(filepath);
        let mut im = Image::new(&name, 0, 0);
        im.source = SmolStr::new_static("FILE");
        im.filepath = filepath.to_owned();
        // `imagetile_alloc` defaults.
        im.generated_type = SmolStr::new_static("UV_GRID");
        im.generated_width = 1024;
        im.generated_height = 1024;
        im.generated_color = [0.0; 4];
        im.generated_depth = 0;
        im.size = [0, 0];
        // `file_format` of an image without a buffer (`IMB_FTYPE_NONE`).
        im.file_format = SmolStr::new_static("TARGA");
        if ["exr", "cin", "dpx", "hdr"]
            .iter()
            .any(|e| has_extension(filepath, e))
        {
            im.alpha_mode = SmolStr::new_static("PREMUL");
        }
        if let Ok(b) = probe_image(&bytes) {
            im.size = [b.x as i32, b.y as i32];
            im.file_format = b.ftype.clone();
            if b.float && matches!(b.ftype.as_str(), "OPEN_EXR" | "HDR") {
                im.colorspace_settings.name = SmolStr::new_static("Linear Rec.709");
            }
        }
        Ok(im)
    }
}

/// `BLI_path_basename`
fn basename(p: &str) -> String {
    p.rsplit(['/', '\\']).next().unwrap_or(p).to_owned()
}

/// Case-insensitive extension test (`BLI_path_extension_check`).
fn has_extension(p: &str, ext: &str) -> bool {
    p.rsplit_once('.')
        .is_some_and(|(_, e)| e.eq_ignore_ascii_case(ext))
}

/// `strerror(errno)` wording for the common cases.
pub fn io_message(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "No such file or directory".into(),
        std::io::ErrorKind::PermissionDenied => "Permission denied".into(),
        _ => {
            let s = e.to_string();
            // Rust appends " (os error N)".
            s.split(" (os error").next().unwrap_or(&s).to_owned()
        }
    }
}

/// `Image.type` (`ima->type`): what a source makes of the image.
fn image_type(i: &Image) -> &'static str {
    match (i.source.as_str(), i.header.name.as_str()) {
        ("GENERATED", _) => "UV_TEST",
        ("VIEWER", "Render Result") => "RENDER_RESULT",
        ("VIEWER", _) => "COMPOSITING",
        _ => "IMAGE",
    }
}

crate::id::id_rna! { Image as frap_spec_rna::IMAGE, "IMAGE" {
    filepath => get(|i| i.filepath.clone()) set(|i, v: String| i.set_filepath(v)),
    filepath_raw => get(|i| i.filepath.clone()) set(|i, v: String| i.filepath = v),
    file_format => file_format,
    source => get(|i| i.source.clone()) set(|i, v: SmolStr| i.set_source(v)),
    type => readonly(|i| image_type(i)),
    packed_file => hook,
    packed_files => hook_coll,
    is_stereo_3d => readonly(|_i| false),
    is_multiview => readonly(|_i| false),
    is_dirty => readonly(|i| i.is_dirty()),
    generated_type => get(|i| i.generated_type.clone()) set(|i, v: SmolStr| {
        i.generated_type = v;
        i.free_buffers();
    }),
    generated_width => get(|i| i.generated_width) set(|i, v: i32| {
        i.generated_width = v.clamp(1, 65536);
        i.free_buffers();
    }),
    generated_height => get(|i| i.generated_height) set(|i, v: i32| {
        i.generated_height = v.clamp(1, 65536);
        i.free_buffers();
    }),
    use_generated_float => get(|i| i.use_generated_float) set(|i, v: bool| {
        i.use_generated_float = v;
        i.free_buffers();
    }),
    generated_color => get(|i| i.generated_color) set(|i, v: [f32; 4]| {
        i.generated_color = v.map(|c| c.max(0.0));
        i.free_buffers();
    }) (tag = Color),
    has_data => hook,
    depth => hook,
    size => hook,
    frame_duration => readonly(|_i| 1),
    pixels => hook,
    channels => hook,
    is_float => hook,
    colorspace_settings => struct(colorspace_settings),
    alpha_mode => get(|i| i.alpha_mode.clone()) set(|i, v: SmolStr| i.set_alpha_mode(v)),
}}

impl IdType for Image {
    type Kind = ImageId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_changes_free_the_buffer() {
        let mut i = Image::new("I", 2, 2);
        let mut b = i.generated_buffer().unwrap();
        b.set_pixels(&[1.0; 16]);
        i.buffer = Some(Arc::new(b));
        assert!(i.is_dirty());
        i.set_source("GENERATED".into());
        assert!(i.is_dirty(), "same source: nothing happens");
        i.set_filepath("x.png".into());
        assert!(!i.is_dirty());
    }

    #[test]
    fn path_base() {
        let unsaved = PathBase {
            blend: None,
            cwd: Some("/work".into()),
        };
        // Verified on 5.2.4: `unpack()` of an unsaved file writes `//textures/x.png` to
        // `textures/x.png` in the working directory, and reports print the path without `//`.
        assert_eq!(unsaved.abs("//textures/x.png"), "textures/x.png");
        assert_eq!(
            unsaved.file("//textures/x.png"),
            std::path::PathBuf::from("/work/textures/x.png")
        );
        assert_eq!(unsaved.abs("sub/x.png"), "sub/x.png");
        assert_eq!(
            unsaved.file("/abs/x.png"),
            std::path::PathBuf::from("/abs/x.png")
        );
        let saved = PathBase {
            blend: Some("/proj/scene.frap".into()),
            cwd: Some("/work".into()),
        };
        assert_eq!(saved.abs("//tex/x.png"), "/proj/tex/x.png");
        assert_eq!(saved.file("x.png"), std::path::PathBuf::from("/work/x.png"));
    }

    #[test]
    fn load_errors_and_directories() {
        let base = PathBase::default();
        assert_eq!(
            Image::load("/nonexistent-frap-dir/x.png", &base).unwrap_err(),
            LoadError::Io("No such file or directory".into())
        );
        // A directory opens but has no data (verified on 5.2.4: named after the folder,
        // size (0, 0)).
        let dir = std::env::temp_dir();
        let d = dir.to_string_lossy().trim_end_matches('/').to_owned();
        let im = Image::load(&d, &base).unwrap();
        assert_eq!(im.header.name, basename(&d));
        assert_eq!((im.source.as_str(), im.size), ("FILE", [0, 0]));
        assert_eq!(im.file_format, "TARGA");
        assert!(im.acquire(&base).is_none());
    }

    #[test]
    fn tiled_and_generated_buffers() {
        let mut i = Image::new("T", 3, 2);
        i.source = "TILED".into();
        let b = i.acquire(&PathBase::default()).unwrap();
        assert_eq!((b.x, b.y, b.depth()), (3, 2, 24));
        // A GENERATED image ignores a packed file it kept.
        let mut g = Image::new("G", 2, 2);
        g.packed_file = Some(Arc::new(EncodedImage {
            bytes: vec![1, 2, 3],
            mime: "image/png".into(),
        }));
        assert_eq!(g.acquire(&PathBase::default()).unwrap().x, 2);
        assert_eq!(image_type(&g), "UV_TEST");
    }

    #[test]
    fn colour_space_changes_drop_the_buffer() {
        let base = PathBase::default();
        let mut i = Image::new("I", 2, 1);
        let mut b = i.acquire(&base).unwrap();
        assert_eq!(b.colorspace.as_ref(), Some(&i.colorspace_settings));
        b.set_pixels(&[0.5; 8]);
        i.buffer = Some(Arc::new(b));
        // GENERATED: `alpha_mode` keeps the buffer (verified on 5.2.4: still dirty).
        i.set_alpha_mode("PREMUL".into());
        assert!(i.is_dirty());
        // The colour space frees it (5.2.4: not dirty, pixels generated again).
        i.colorspace_settings.name = "Non-Color".into();
        assert!(!i.is_dirty());
        assert_eq!(i.acquire(&base).unwrap().pixels()[0], 0.0);
        assert_eq!(i.info(&base).unwrap().x, 2);
        // Other sources: `alpha_mode` frees the buffer.
        let mut f = Image::new("F", 1, 1);
        f.source = "FILE".into();
        let mut b = ImBuf::new(1, 1, 32, false);
        b.dirty = true;
        f.buffer = Some(Arc::new(b));
        assert!(f.is_dirty());
        f.set_alpha_mode("STRAIGHT".into());
        assert!(!f.is_dirty());
    }

    #[test]
    fn info_without_pixels() {
        let base = PathBase::default();
        let mut i = Image::new("G", 7, 3);
        i.use_generated_float = true;
        i.generated_depth = 32;
        let info = i.info(&base).unwrap();
        assert_eq!(
            (info.x, info.y, info.depth(), info.float),
            (7, 3, 128, true)
        );
        let b = i.acquire(&base).unwrap();
        assert_eq!(ImInfo::of(&b), info);
        i.source = "FILE".into();
        i.filepath = "/nonexistent-frap-dir/x.png".into();
        assert!(i.info(&base).is_none());
        i.source = "VIEWER".into();
        assert!(i.info(&base).is_none());
    }

    #[test]
    fn known_size() {
        let mut i = Image::new("I", 4, 2);
        assert_eq!(i.known_size(), [4, 2]);
        i.generated_width = 7;
        assert_eq!(i.known_size(), [7, 2]);
        i.source = "FILE".into();
        i.size = [3, 3];
        assert_eq!(i.known_size(), [3, 3]);
        i.set_source("GENERATED".into());
        assert_eq!(i.known_size(), [7, 2]);
        assert_eq!(i.filepath, "");
    }
}
