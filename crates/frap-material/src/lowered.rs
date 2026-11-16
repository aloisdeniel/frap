// SPDX-License-Identifier: GPL-3.0-or-later
//! `LoweredMaterial` (ARCHITECTURE §16.2, CONTRACT frozen at M1 W0, extended additively in M3).

use std::collections::BTreeSet;

use frap_nodes::NodeTree;
use frap_rna::{IdRef, ImageId, Report};
use frap_scene::id::{Image, Material};
use smol_str::SmolStr;

/// A factor and an optional texture multiplying it.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel<T> {
    /// Constant factor.
    pub factor: T,
    /// Texture, if any.
    pub texture: Option<TexRef>,
}

/// Which UV map a texture uses.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UvSource {
    /// A UV Map node naming a layer.
    Named(SmolStr),
    /// The active-render UV map (no UV Map node).
    ActiveRender,
    /// `TEXCOORD_n` on import.
    Index(u32),
}

/// Which channels of the image feed the socket.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChannelSel {
    /// RGB
    Rgb,
    /// RGBA
    Rgba,
    /// R (occlusion)
    R,
    /// G (roughness)
    G,
    /// B (metallic)
    B,
    /// A (alpha)
    A,
}

/// `KHR_texture_transform` (from Mapping nodes).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TexTransform {
    /// offset
    pub offset: [f32; 2],
    /// rotation (radians)
    pub rotation: f32,
    /// scale
    pub scale: [f32; 2],
}

/// Texture filter (Image Texture `interpolation`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Filter {
    /// Linear
    Linear,
    /// Closest
    Closest,
    /// Cubic (exported as Linear)
    Cubic,
}

/// Texture wrap (Image Texture `extension`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Wrap {
    /// REPEAT
    Repeat,
    /// EXTEND (clamp)
    Clamp,
    /// MIRROR
    Mirror,
}

/// Sampler state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Sampler {
    /// Magnification / minification filter.
    pub filter: Filter,
    /// S wrap.
    pub wrap_s: Wrap,
    /// T wrap.
    pub wrap_t: Wrap,
}

/// A texture reference.
#[derive(Clone, Debug, PartialEq)]
pub struct TexRef {
    /// The image.
    pub image: IdRef<ImageId>,
    /// UV source.
    pub uv: UvSource,
    /// Channels used.
    pub channel: ChannelSel,
    /// Texture transform.
    pub transform: Option<TexTransform>,
    /// Sampler.
    pub sampler: Sampler,
    /// Image colorspace is Non-Color.
    pub colorspace_is_data: bool,
}

/// Normal map (Normal Map node, tangent space).
#[derive(Clone, Debug, PartialEq)]
pub struct NormalMap {
    /// Texture.
    pub texture: TexRef,
    /// `Strength`
    pub scale: f32,
}

/// Alpha mode, from node patterns (not `blend_method`, [RNA §0.9]).
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AlphaMode {
    /// OPAQUE
    Opaque,
    /// MASK with cutoff
    Mask {
        /// `alphaCutoff`
        cutoff: f32,
    },
    /// BLEND
    Blend,
}

/// Extension parameters (clearcoat, sheen, transmission, volume, ior, specular, iridescence,
/// anisotropy, dispersion, diffuse_transmission): fields arrive additively in M3.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct MaterialExts {}

/// How a colour attribute feeds the material (glTF `COLOR_0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexColorUse {
    /// Attribute name (`None` = active colour attribute).
    pub attribute: Option<SmolStr>,
    /// Multiplies base colour (true) or is used alone.
    pub multiply: bool,
}

/// The glTF-shaped shading model shared by the exporter and the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredMaterial {
    /// baseColorFactor / baseColorTexture
    pub base_color: Channel<[f32; 4]>,
    /// metallicFactor (texture: B of the ORM image)
    pub metallic: Channel<f32>,
    /// roughnessFactor (texture: G of the ORM image)
    pub roughness: Channel<f32>,
    /// normalTexture
    pub normal: Option<NormalMap>,
    /// occlusionTexture
    pub occlusion: Option<TexRef>,
    /// emissiveFactor / emissiveTexture
    pub emissive: Channel<[f32; 3]>,
    /// KHR_materials_emissive_strength
    pub emissive_strength: f32,
    /// Opaque | Mask { cutoff } | Blend (from node patterns).
    pub alpha_mode: AlphaMode,
    /// `!use_backface_culling`
    pub double_sided: bool,
    /// KHR_materials_unlit (Light Path pattern)
    pub unlit: bool,
    /// Other extensions.
    pub ext: MaterialExts,
    /// Which colour attribute, how combined.
    pub vertex_color: Option<VertexColorUse>,
    /// Unrecognised patterns ⇒ fallback + WARNING.
    pub diagnostics: Vec<Report>,
}

/// Image metadata lookup for lowering (colorspace, packed/external), implemented by `Main`.
pub trait ImageLookup {
    /// The image datablock.
    fn image(&self, im: IdRef<ImageId>) -> Option<&Image>;
}

impl ImageLookup for frap_scene::Main {
    fn image(&self, im: IdRef<ImageId>) -> Option<&Image> {
        self.images.get(im)
    }
}

/// Unique node names while raising a graph (`Principled BSDF`, `Image Texture.001`, …).
#[derive(Clone, Debug, Default)]
pub struct NameAlloc {
    /// Names already used in the tree.
    pub taken: BTreeSet<SmolStr>,
}

/// What a pattern recogniser reads while lowering.
#[non_exhaustive]
pub struct LowerCx<'a> {
    /// The material's node tree.
    pub tree: &'a NodeTree,
    /// The material (viewport values, `use_backface_culling`, …).
    pub mat: &'a Material,
    /// Image metadata.
    pub images: &'a dyn ImageLookup,
}

impl<'a> LowerCx<'a> {
    /// Context for lowering `mat`.
    pub fn new(tree: &'a NodeTree, mat: &'a Material, images: &'a dyn ImageLookup) -> Self {
        LowerCx { tree, mat, images }
    }
}

/// What a pattern's `raise` writes into while the importer builds a node graph. Fields that
/// the patterns share (e.g. the Principled BSDF node they link into) are added by WP 1.11
/// (`#[non_exhaustive]`, additive).
#[non_exhaustive]
pub struct RaiseCx<'a> {
    /// The material whose embedded node tree is being built.
    pub into: &'a mut Material,
    /// Node names taken so far.
    pub names: &'a mut NameAlloc,
}

impl<'a> RaiseCx<'a> {
    /// Context for raising into `into`.
    pub fn new(into: &'a mut Material, names: &'a mut NameAlloc) -> Self {
        RaiseCx { into, names }
    }
}

/// One pattern recogniser: one file in `patterns/` registers one entry (ARCHITECTURE §16.2
/// extension point, M1 W0). `lower` runs every entry in ascending `order` over the same
/// [`LoweredMaterial`]; `raise` runs every entry's `raise` in the same order. Later milestones
/// add files (`clearcoat.rs`, `sheen.rs`, …), never edit `lower`/`raise`.
#[derive(Debug)]
pub struct MaterialPattern {
    /// Unique name (the file stem: `base_color`, `metallic_roughness`, …).
    pub name: &'static str,
    /// Run order (ascending).
    pub order: u16,
    /// Recognises this feature in the tree and fills its part of the output (unrecognised
    /// shapes: fallback value + a WARNING in `diagnostics`).
    pub lower: fn(&LowerCx, &mut LoweredMaterial),
    /// Adds this feature's nodes and links for the importer (`None`: nothing to raise).
    pub raise: Option<fn(&LoweredMaterial, &mut RaiseCx)>,
}

/// Every material pattern recogniser.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static MATERIAL_PATTERNS: [MaterialPattern];

/// Lowers a material's node tree by running [`MATERIAL_PATTERNS`] in `order`.
pub fn lower(tree: &NodeTree, mat: &Material, images: &dyn ImageLookup) -> LoweredMaterial {
    let _ = (tree, mat, images);
    todo!("WP 1.11: pattern recognisers")
}

/// Builds a Blender-style node graph from a lowered material (glTF importer), running every
/// [`MATERIAL_PATTERNS`] entry's `raise` in `order`.
/// Law: `lower(raise(lower(x))) == lower(x)`.
pub fn raise(lm: &LoweredMaterial, into: &mut Material, names: &mut NameAlloc) {
    let _ = (lm, into, names);
    todo!("WP 1.11: raise")
}
