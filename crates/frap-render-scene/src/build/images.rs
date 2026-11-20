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

#[cfg(test)]
mod tests {
    use frap_nodes::{NodeTreeType, PropBag};
    use frap_scene::ChangeLog;
    use smol_str::SmolStr;

    use super::*;

    fn node(uid: u32, bl_idname: &'static str) -> Node {
        Node {
            uid,
            name: bl_idname.into(),
            label: String::new(),
            bl_idname,
            location: [0.0, 0.0],
            width: 140.0,
            parent: None,
            mute: false,
            hide: false,
            inputs: vec![],
            outputs: vec![],
            props: PropBag::default(),
        }
    }

    fn enum_prop(n: &mut Node, name: &'static str, v: &str) {
        n.props.values.insert(name, Value::Enum(SmolStr::new(v)));
    }

    #[test]
    fn imbuf_rows_flip_and_floats_unpremultiply() {
        let mut b = ImBuf::new(1, 2, 32, true);
        // Bottom row first: the bottom pixel is half-transparent premultiplied red.
        b.data = ImBufData::Float(vec![0.5, 0.0, 0.0, 0.5, 0.0, 1.0, 0.0, 1.0]);
        let d = from_imbuf(&b, true);
        assert_eq!((d.width, d.height, d.channels), (1, 2, 4));
        assert_eq!(
            d.pixels,
            PixelData::F32(vec![0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.5])
        );
        // Channel-packed or data floats are kept as stored.
        let d = from_imbuf(&b, false);
        assert_eq!(
            d.pixels,
            PixelData::F32(vec![0.0, 1.0, 0.0, 1.0, 0.5, 0.0, 0.0, 0.5])
        );
        let mut im = Image::new("", 1, 1);
        assert!(float_premultiplied(&im));
        im.alpha_mode = "CHANNEL_PACKED".into();
        assert!(!float_premultiplied(&im));
        // Byte buffers keep their bytes, rows flipped.
        let mut b = ImBuf::new(1, 2, 32, false);
        b.data = ImBufData::Byte(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            from_imbuf(&b, true).pixels,
            PixelData::U8(vec![5, 6, 7, 8, 1, 2, 3, 4])
        );
    }

    #[test]
    fn generated_images_and_session_relative_paths() {
        // A generated image reads through `Image::acquire` (its generator).
        let mut im = Image::new("", 4, 2);
        im.generated_color = [0.25, 0.5, 0.75, 1.0];
        let d = acquire(&im, &PathBase::default()).expect("generated");
        assert_eq!((d.width, d.height), (4, 2));
        // A relative file path resolves against the session's directory, not the process's.
        let dir = std::env::temp_dir().join(format!("frap_wp55_images_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = frap_imbuf::encode(
            &DecodedImage {
                width: 2,
                height: 1,
                channels: 4,
                pixels: PixelData::U8(vec![255, 0, 0, 255, 0, 0, 255, 255]),
            },
            &frap_imbuf::EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
        )
        .unwrap();
        std::fs::write(dir.join("tex.png"), png).unwrap();
        let mut f = Image::new("", 0, 0);
        f.source = "FILE".into();
        f.filepath = "tex.png".into();
        let base = PathBase {
            blend: None,
            cwd: Some(dir.clone()),
        };
        let d = acquire(&f, &base).expect("relative to the session cwd");
        assert_eq!((d.width, d.height), (2, 1));
        let mut main = Main::default();
        let r = main.images.insert("Tex", f, &mut ChangeLog::default());
        let mut imgs = Images::new(base);
        let k = imgs.key(&main, r);
        assert_eq!((imgs.map[&k].width, imgs.map[&k].height), (2, 1));
        assert_eq!(imgs.ids()[&r.key()], k);
        // Missing: magenta.
        let mut imgs = Images::new(PathBase {
            blend: None,
            cwd: Some(dir.join("nowhere")),
        });
        let k = imgs.key(&main, r);
        assert_eq!(*imgs.map[&k], missing());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workbench_texture_is_the_first_texture_node() {
        let mut main = Main::default();
        let mut im = Image::new("", 2, 2);
        im.alpha_mode = "STRAIGHT".into();
        let img = main.images.insert("Grid", im, &mut ChangeLog::default());
        let mut tree = NodeTree::new("Shader Nodetree", NodeTreeType::Shader, true);
        tree.nodes.push(node(1, "ShaderNodeBsdfPrincipled"));
        // Texture Coordinate is an input node, not a texture.
        tree.nodes.push(node(2, "ShaderNodeTexCoord"));
        let mut tex = node(3, "ShaderNodeTexImage");
        tex.props.values.insert("image", Value::Id(Some(img.any())));
        enum_prop(&mut tex, "interpolation", "Closest");
        enum_prop(&mut tex, "extension", "MIRROR");
        tree.nodes.push(tex);
        let mut imgs = Images::default();
        assert_eq!(active_texture(&main, &tree).map(|n| n.uid), Some(3));
        let t = imgs.workbench_texture(&main, &tree).expect("image");
        assert_eq!(
            (t.linear, t.extension.as_str(), t.alpha_cutoff, t.is_data),
            (false, "MIRROR", true, false)
        );
        assert_eq!(imgs.map.len(), 1);
        assert_eq!(imgs.ids()[&img.key()], t.image);
        // A procedural texture before it is the active one: no image.
        let mut noise = node(4, "ShaderNodeTexNoise");
        noise.uid = 4;
        tree.nodes.insert(1, noise);
        assert_eq!(active_texture(&main, &tree).map(|n| n.uid), Some(4));
        assert!(imgs.workbench_texture(&main, &tree).is_none());
        // An Image Texture without an image: none; a NONE alpha has no cutoff.
        tree.nodes.remove(1);
        tree.nodes[2].props.values.insert("image", Value::Id(None));
        assert!(imgs.workbench_texture(&main, &tree).is_none());
        tree.nodes[2]
            .props
            .values
            .insert("image", Value::Id(Some(img.any())));
        main.images
            .get_mut(img, &mut ChangeLog::default())
            .unwrap()
            .alpha_mode = "NONE".into();
        let t = Images::default().workbench_texture(&main, &tree).unwrap();
        assert!(!t.alpha_cutoff);
        // Environment Texture: EXTEND (its interpolation still Closest).
        tree.nodes[2].bl_idname = "ShaderNodeTexEnvironment";
        let t = Images::default().workbench_texture(&main, &tree).unwrap();
        assert_eq!((t.extension.as_str(), t.linear), ("EXTEND", false));
        // Inside a node group when the tree has no texture node.
        let group_tree = tree.clone();
        let g = main
            .node_groups
            .insert("Group", group_tree, &mut ChangeLog::default());
        let mut outer = NodeTree::new("Shader Nodetree", NodeTreeType::Shader, true);
        let mut gn = node(9, "ShaderNodeGroup");
        gn.props
            .values
            .insert("node_tree", Value::Id(Some(g.any())));
        outer.nodes.push(gn);
        assert_eq!(active_texture(&main, &outer).map(|n| n.uid), Some(3));
    }

    #[test]
    fn principled_values_the_lowered_material_drops() {
        use frap_nodes::{Link, Socket, SocketType, SocketValue};
        let socket = |name: &str, ty: SocketType, v: SocketValue| Socket {
            name: name.into(),
            identifier: name.into(),
            ty,
            enabled: true,
            hide_value: false,
            default_value: v,
        };
        let mut tree = NodeTree::new("Shader Nodetree", NodeTreeType::Shader, true);
        let mut p = node(1, "ShaderNodeBsdfPrincipled");
        p.inputs = vec![
            socket("Sheen Weight", SocketType::Value, SocketValue::Float(0.25)),
            socket("Coat IOR", SocketType::Value, SocketValue::Float(1.8)),
            socket(
                "Coat Tint",
                SocketType::Rgba,
                SocketValue::Color([0.9, 0.8, 0.7, 1.0]),
            ),
        ];
        tree.nodes.push(p);
        let mut s = crate::types::RMaterialSettings::default();
        assert_eq!(
            (s.sheen_weight, s.coat_ior, s.coat_tint),
            (1.0, 1.5, [1.0; 3])
        );
        super::super::principled_values(&tree, &mut s);
        assert_eq!(
            (s.sheen_weight, s.coat_ior, s.coat_tint),
            (0.25, 1.8, [0.9, 0.8, 0.7])
        );
        // A linked socket keeps the default.
        let mut v = node(2, "ShaderNodeValue");
        v.outputs = vec![socket("Value", SocketType::Value, SocketValue::Float(0.5))];
        tree.nodes.push(v);
        tree.links.push(Link {
            from_node: 2,
            from_socket: "Value".into(),
            to_node: 1,
            to_socket: "Sheen Weight".into(),
        });
        let mut s = crate::types::RMaterialSettings::default();
        super::super::principled_values(&tree, &mut s);
        assert_eq!((s.sheen_weight, s.coat_ior), (1.0, 1.8));
    }
}
