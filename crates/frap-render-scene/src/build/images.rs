// SPDX-License-Identifier: GPL-3.0-or-later
//! Images the render needs (material textures, environment textures, the Workbench TEXTURE
//! image), read once per build and keyed by `(IdKey, generation)`.
//!
//! Pixels come from [`Image::acquire`] (`BKE_image_acquire_ibuf`, M5+M6 W2, WP 5.5): the
//! image's buffer when it has one (pixel edits, `Image.scale()`), else its packed bytes, its file
//! or its generator (`UV_GRID`, `COLOR_GRID`, `BLANK`), converted with its colour space and alpha
//! mode. Relative paths resolve against the session ([`PathBase`]: `//` is relative to the
//! directory of `bpy.data.filepath`, other relative paths to the session's working directory,
//! not the process's). An image that cannot be loaded renders as Blender's missing-texture
//! magenta (1, 0, 1, 1).
//!
//! [`DecodedImage`] conventions of the result: rows top first (file order; an [`ImBuf`] is bottom
//! first), four channels, straight alpha; byte pixels in the image's own colour space (the
//! engines decode sRGB unless the texture is Non-Color), float pixels scene linear.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use frap_imbuf::{DecodedImage, PixelData};
use frap_nodes::{Node, NodeTree};
use frap_rna::{IdKey, IdRef, ImageId, NodeTreeId, Value};
use frap_scene::Main;
use frap_scene::id::{ImBuf, ImBufData, Image, PathBase};
use indexmap::IndexMap;

use super::keys::image_key;
use crate::types::{ImageKey, RWorkbenchTexture};

/// Collects decoded images while the scene is built.
#[derive(Default)]
pub struct Images {
    /// Decoded images in first-use order.
    pub map: IndexMap<ImageKey, Arc<DecodedImage>>,
    by_id: HashMap<IdKey, ImageKey>,
    base: PathBase,
}

impl Images {
    /// An empty collection whose relative paths resolve against `base` (the session's,
    /// `PathBase::of(host)`; the default resolves against the process directory).
    pub fn new(base: PathBase) -> Images {
        Images {
            base,
            ..Images::default()
        }
    }

    /// Datablock → key of every collected image (`RenderScene::image_ids`).
    pub fn ids(&self) -> IndexMap<IdKey, ImageKey> {
        let mut v: Vec<(IdKey, ImageKey)> = self.by_id.iter().map(|(a, b)| (*a, *b)).collect();
        v.sort_by_key(|(_, k)| self.map.get_index_of(k));
        v.into_iter().collect()
    }
}

/// Blender's missing-texture colour.
pub fn missing() -> DecodedImage {
    DecodedImage {
        width: 1,
        height: 1,
        channels: 4,
        pixels: PixelData::F32(vec![1.0, 0.0, 1.0, 1.0]),
    }
}

/// Where an image file is read from without a session (the legacy rule of [`decode`]): `//`
/// relative to the directory of `blend_path`, other relative paths to the process directory.
pub fn resolve_path(filepath: &str, blend_path: Option<&str>) -> PathBuf {
    if let Some(rel) = filepath.strip_prefix("//") {
        let base = blend_path
            .and_then(|p| Path::new(p).parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        return base.join(rel);
    }
    PathBuf::from(filepath)
}

/// Decodes `im` without `Image::acquire`: the packed bytes or the file as they are, a
/// GENERATED image filled with `generated_color` (linear). [`acquire`] falls back to it when
/// no image decoder is linked into the process (`frap-scene`'s `IMAGE_DECODERS` is empty in
/// unit tests of crates below `frap-ops-data`).
pub fn decode(im: &Image, blend_path: Option<&str>) -> Option<DecodedImage> {
    if let Some(p) = &im.packed_file {
        return frap_imbuf::decode(&p.bytes).ok();
    }
    match im.source.as_str() {
        "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if !im.filepath.is_empty() => {
            frap_imbuf::load(&resolve_path(&im.filepath, blend_path)).ok()
        }
        "GENERATED" => {
            let (w, h) = (
                im.generated_width.max(1) as u32,
                im.generated_height.max(1) as u32,
            );
            let n = (w as usize) * (h as usize);
            let mut px = Vec::with_capacity(n * 4);
            for _ in 0..n {
                px.extend_from_slice(&im.generated_color);
            }
            Some(DecodedImage {
                width: w,
                height: h,
                channels: 4,
                pixels: PixelData::F32(px),
            })
        }
        _ => None,
    }
}

/// The pixels of `im` as [`Image::acquire`] gives them (see the module documentation); `None`
/// when they cannot be loaded. Without a linked image decoder (unit tests of the crates below
/// `frap-ops-data`), files and packed bytes are read with [`decode`] instead (relative paths
/// still resolve against `base`).
pub fn acquire(im: &Image, base: &PathBase) -> Option<DecodedImage> {
    if let Some(b) = im.acquire(base) {
        return Some(from_imbuf(&b, float_premultiplied(im)));
    }
    if !frap_scene::id::IMAGE_DECODERS.is_empty() {
        return None;
    }
    let mut copy = im.clone();
    if im.packed_file.is_none() && !im.filepath.is_empty() {
        copy.filepath = base.file(&im.filepath).to_string_lossy().into_owned();
    }
    decode(&copy, base.blend.as_deref())
}

/// Whether `Image::acquire` leaves a float buffer of `im` premultiplied
/// (`imb_handle_colorspace_and_alpha`: every float buffer but those of a data colour space or a
/// `CHANNEL_PACKED` alpha).
pub fn float_premultiplied(im: &Image) -> bool {
    !(im.is_data_colorspace() || im.alpha_mode == "CHANNEL_PACKED")
}

/// An [`ImBuf`] (bottom row first) as a [`DecodedImage`] (top row first, straight alpha:
/// a float buffer stored `premultiplied` is divided by its alpha where it is neither 0 nor 1,
/// as `IMB_unpremultiply_rect_float`).
pub fn from_imbuf(b: &ImBuf, premultiplied: bool) -> DecodedImage {
    let row = b.x as usize * 4;
    let pixels = match &b.data {
        ImBufData::Byte(v) => PixelData::U8(ImBuf::flip_rows(v, row)),
        ImBufData::Float(v) => {
            let mut f = ImBuf::flip_rows(v, row);
            if premultiplied {
                for p in f.chunks_exact_mut(4) {
                    let a = p[3];
                    if a != 0.0 && a != 1.0 {
                        let inv = 1.0 / a;
                        p[0] *= inv;
                        p[1] *= inv;
                        p[2] *= inv;
                    }
                }
            }
            PixelData::F32(f)
        }
    };
    DecodedImage {
        width: b.x,
        height: b.y,
        channels: 4,
        pixels,
    }
}

impl Images {
    /// The key of `r`, decoding it on first use.
    pub fn key(&mut self, main: &Main, r: IdRef<ImageId>) -> ImageKey {
        if let Some(k) = self.by_id.get(&r.key()) {
            return *k;
        }
        let (k, img) = match main.images.get(r) {
            Some(im) => (
                image_key(r.key(), &im.header),
                acquire(im, &self.base).unwrap_or_else(missing),
            ),
            // A dangling reference: a stable key per IdKey, magenta pixels.
            None => (ImageKey(r.key().get() ^ 0xdead_beef_0000_0000), missing()),
        };
        self.by_id.insert(r.key(), k);
        self.map.entry(k).or_insert_with(|| Arc::new(img));
        k
    }

    /// The Workbench TEXTURE source of a material with node tree `tree` (`MaterialTexture`,
    /// `workbench_materials.cc`, Blender 5.2.4, GPL-2.0-or-later): the image of the tree's
    /// active texture node ([`active_texture`]) when it is an Image or Environment Texture
    /// with an image, keyed (and read) here. `None`: Workbench draws the material colour.
    pub fn workbench_texture(&mut self, main: &Main, tree: &NodeTree) -> Option<RWorkbenchTexture> {
        let node = active_texture(main, tree)?;
        let (linear, extension) = match node.bl_idname {
            "ShaderNodeTexImage" => (
                node.prop_str("interpolation") != "Closest",
                match node.prop_str("extension") {
                    e @ ("REPEAT" | "MIRROR" | "CLIP") => e,
                    _ => "EXTEND",
                },
            ),
            "ShaderNodeTexEnvironment" => (node.prop_str("interpolation") != "Closest", "EXTEND"),
            _ => return None,
        };
        let r: IdRef<ImageId> = match node.prop("image") {
            Some(Value::Id(Some(id))) => IdRef::from_any(*id)?,
            _ => return None,
        };
        let im = main.images.get(r)?;
        let alpha_cutoff = !matches!(im.alpha_mode.as_str(), "NONE" | "CHANNEL_PACKED");
        let is_data = im.is_data_colorspace();
        let image = self.key(main, r);
        let mut t = RWorkbenchTexture::new(image, linear, extension, alpha_cutoff);
        t.is_data = is_data;
        Some(t)
    }
}

/// Whether `node` is of the texture class (`NODE_CLASS_TEXTURE`: the `ShaderNodeTex*` nodes,
/// Texture Coordinate excepted).
fn is_texture_class(node: &Node) -> bool {
    node.bl_idname.starts_with("ShaderNodeTex") && node.bl_idname != "ShaderNodeTexCoord"
}

/// `node_get_active(ntree, NODE_ACTIVE_TEXTURE)` (`node_shader_util.cc`): the node flagged as
/// the active texture, else the first node of the texture class, else (none at this level) the
/// first one inside a node group. frap stores no `NODE_ACTIVE_TEXTURE` flag (`Node.show_texture`
/// reads False and `nodes.active` is not bound), so no node is ever flagged: the first texture
/// node in node order decides, as in Blender for a tree whose texture nodes were all added with
/// `nodes.new()`.
pub fn active_texture<'a>(main: &'a Main, tree: &'a NodeTree) -> Option<&'a Node> {
    active_texture_depth(main, tree, 0)
}

fn active_texture_depth<'a>(main: &'a Main, tree: &'a NodeTree, depth: u32) -> Option<&'a Node> {
    if let Some(n) = tree.nodes.iter().find(|n| is_texture_class(n)) {
        return Some(n);
    }
    if depth > 16 {
        return None;
    }
    tree.nodes
        .iter()
        .filter(|n| n.bl_idname == "ShaderNodeGroup")
        .filter_map(|n| match n.prop("node_tree") {
            Some(Value::Id(Some(id))) => IdRef::<NodeTreeId>::from_any(*id),
            _ => None,
        })
        .filter_map(|r| main.node_groups.get(r))
        .find_map(|g| active_texture_depth(main, g, depth + 1))
}
