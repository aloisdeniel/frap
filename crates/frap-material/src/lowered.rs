// SPDX-License-Identifier: GPL-3.0-or-later
//! `LoweredMaterial` (ARCHITECTURE §16.2, CONTRACT frozen at M1 W0, extended additively in M3).

use std::collections::{BTreeMap, BTreeSet};

use frap_nodes::NodeTree;
use frap_rna::{IdRef, ImageId, Report};
use frap_scene::id::{Image, Material};
use smol_str::SmolStr;

// WP 1.11: helpers shared by the pattern recognisers (graph navigation for `lower`, graph
// building for `raise`). They name no pattern; the patterns live in `patterns/`.
pub mod build;
pub mod graph;

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

/// `KHR_materials_clearcoat` (Principled `Coat Weight` R, `Coat Roughness` G, `Coat Normal`).
#[derive(Clone, Debug, PartialEq)]
pub struct Clearcoat {
    /// `clearcoatFactor` / `clearcoatTexture`
    pub factor: Channel<f32>,
    /// `clearcoatRoughnessFactor` / `clearcoatRoughnessTexture`
    pub roughness: Channel<f32>,
    /// `clearcoatNormalTexture`
    pub normal: Option<NormalMap>,
}

/// `KHR_materials_sheen` (`Sheen Tint` RGB, `Sheen Roughness` A; enabled by `Sheen Weight`).
#[derive(Clone, Debug, PartialEq)]
pub struct Sheen {
    /// `sheenColorFactor` / `sheenColorTexture`
    pub color: Channel<[f32; 3]>,
    /// `sheenRoughnessFactor` / `sheenRoughnessTexture`
    pub roughness: Channel<f32>,
}

/// `KHR_materials_transmission` (`Transmission Weight`, R).
#[derive(Clone, Debug, PartialEq)]
pub struct Transmission {
    /// `transmissionFactor` / `transmissionTexture`
    pub factor: Channel<f32>,
}

/// `KHR_materials_volume` (glTF Material Output `Thickness` G + Volume Absorption).
#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    /// `thicknessFactor` / `thicknessTexture`
    pub thickness: Channel<f32>,
    /// `attenuationDistance` (`1 / Density`; `f32::INFINITY` when absent)
    pub attenuation_distance: f32,
    /// `attenuationColor`
    pub attenuation_color: [f32; 3],
}

/// `KHR_materials_specular` (`Specular IOR Level` × 2 in A, `Specular Tint` RGB).
#[derive(Clone, Debug, PartialEq)]
pub struct Specular {
    /// `specularFactor` / `specularTexture`
    pub factor: Channel<f32>,
    /// `specularColorFactor` / `specularColorTexture`
    pub color: Channel<[f32; 3]>,
}

/// `KHR_materials_iridescence` (`Thin Film *` sockets + glTF Material Output inputs).
#[derive(Clone, Debug, PartialEq)]
pub struct Iridescence {
    /// `iridescenceFactor` / `iridescenceTexture` (R)
    pub factor: Channel<f32>,
    /// `iridescenceIor`
    pub ior: f32,
    /// `iridescenceThicknessMinimum` (nm)
    pub thickness_min: f32,
    /// `iridescenceThicknessMaximum` (nm) / `iridescenceThicknessTexture` (G)
    pub thickness_max: Channel<f32>,
}

/// `KHR_materials_anisotropy` (`Anisotropic`, `Anisotropic Rotation` × 2π, tangent pattern).
#[derive(Clone, Debug, PartialEq)]
pub struct Anisotropy {
    /// `anisotropyStrength`
    pub strength: f32,
    /// `anisotropyRotation` (radians)
    pub rotation: f32,
    /// `anisotropyTexture` (RG direction, B strength)
    pub texture: Option<TexRef>,
}

/// `KHR_materials_diffuse_transmission` (frap option, M7).
#[derive(Clone, Debug, PartialEq)]
pub struct DiffuseTransmission {
    /// `diffuseTransmissionFactor` / `diffuseTransmissionTexture` (A)
    pub factor: Channel<f32>,
    /// `diffuseTransmissionColorFactor` / `diffuseTransmissionColorTexture` (RGB)
    pub color: Channel<[f32; 3]>,
}

/// Extension parameters, `None` when the material does not use the extension (M3 W0 fields;
/// patterns of WP 3.6 fill them, the exporter writes them, the renderer reads them in M5).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct MaterialExts {
    /// `KHR_materials_clearcoat`
    pub clearcoat: Option<Clearcoat>,
    /// `KHR_materials_sheen`
    pub sheen: Option<Sheen>,
    /// `KHR_materials_transmission`
    pub transmission: Option<Transmission>,
    /// `KHR_materials_volume`
    pub volume: Option<Volume>,
    /// `KHR_materials_ior` (`IOR`; written only with transmission, volume or specular)
    pub ior: Option<f32>,
    /// `KHR_materials_specular`
    pub specular: Option<Specular>,
    /// `KHR_materials_iridescence`
    pub iridescence: Option<Iridescence>,
    /// `KHR_materials_anisotropy`
    pub anisotropy: Option<Anisotropy>,
    /// `KHR_materials_dispersion` (`dispersion`, only with volume)
    pub dispersion: Option<f32>,
    /// `KHR_materials_diffuse_transmission`
    pub diffuse_transmission: Option<DiffuseTransmission>,
    /// `KHR_materials_pbrSpecularGlossiness` (import only: the add-on raises it and never
    /// exports it). When set, its pattern (the first raised) builds the importer's whole
    /// spec-gloss graph and leaves [`RaiseCx::bsdf`] unset, so the other patterns add nothing;
    /// the diffuse colour is [`LoweredMaterial::base_color`], and the importer sets no other
    /// extension (the add-on ignores them).
    pub pbr_specular_glossiness: Option<SpecularGlossiness>,
}

/// `KHR_materials_pbrSpecularGlossiness` as the glTF importer raises it
/// (`blender/imp/KHR_materials_pbrSpecularGlossiness.py`): `Specular Tint` from the specular
/// colour, `IOR` 1000 (F0 = the tint), `Roughness` = 1 − glossiness.
#[derive(Clone, Debug, PartialEq)]
pub struct SpecularGlossiness {
    /// `specularFactor` / RGB of `specularGlossinessTexture` (`Specular Tint`).
    pub specular: Channel<[f32; 3]>,
    /// `glossinessFactor`.
    pub glossiness_factor: f32,
    /// The material has a `specularGlossinessTexture`.
    pub has_texture: bool,
    /// The importer's baked roughness image (a packed copy of the texture's image whose alpha
    /// is 1 − glossinessFactor × A), read through its `Alpha` output; `None` when the
    /// texture has no image.
    pub roughness: Option<TexRef>,
}

/// How a colour attribute feeds the material (glTF `COLOR_0`): the add-on's `vc_info`.
///
/// The exporter's `COLOR_n` writer (MATERIAL mode) exports `attribute` when `color`; it adds
/// the alpha of `alpha_attribute` when `alpha` and the material is not OPAQUE
/// ([`LoweredMaterial::alpha_mode`], the add-on's `vc_info['alpha_mode'] != 'OPAQUE'`);
/// without `color` it exports nothing (the add-on warns "Vertex Color alpha without color").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VertexColorUse {
    /// Attribute name (`None` = active colour attribute): the one Base Color reads when
    /// `color`, else the one Alpha reads.
    pub attribute: Option<SmolStr>,
    /// Multiplies base colour (true) or is used alone.
    pub multiply: bool,
    /// `vc_info['color_type']` is set: Base Color reads `attribute`. M3 (WP 3.5).
    pub color: bool,
    /// `vc_info['alpha_type']` is set: Alpha reads `alpha_attribute`. M3 (WP 3.5).
    pub alpha: bool,
    /// `vc_info['alpha']` (`None` = active colour attribute), meaningful when `alpha`. M3
    /// (WP 3.5).
    pub alpha_attribute: Option<SmolStr>,
}

impl VertexColorUse {
    /// Base Color (and Alpha) read the same colour attribute: the active one when
    /// `attribute` is `None`.
    pub fn color_and_alpha(attribute: Option<SmolStr>, multiply: bool) -> VertexColorUse {
        VertexColorUse {
            alpha_attribute: attribute.clone(),
            attribute,
            multiply,
            color: true,
            alpha: true,
        }
    }

    /// Only Base Color reads a colour attribute.
    pub fn color_only(attribute: Option<SmolStr>, multiply: bool) -> VertexColorUse {
        VertexColorUse {
            attribute,
            multiply,
            color: true,
            alpha: false,
            alpha_attribute: None,
        }
    }

    /// Whether `COLOR_n` carries the alpha attribute (the add-on's `add_alpha` in MATERIAL
    /// mode) for a material with alpha mode `mode`.
    pub fn adds_alpha(&self, mode: AlphaMode) -> bool {
        self.color && self.alpha && mode != AlphaMode::Opaque
    }
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
    /// `occlusionTexture.strength` (a Mix with white before the glTF Material Output's
    /// `Occlusion`; 1 when absent). M3 W0.
    pub occlusion_strength: f32,
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

impl LoweredMaterial {
    /// The glTF default material (every factor at its glTF default: white base colour,
    /// metallic 1, roughness 1, no emission, OPAQUE, single-sided). `lower` starts from it;
    /// each pattern overwrites what it recognises, so an unrecognised graph exports what the
    /// Blender exporter writes for it (no factors).
    pub fn gltf_default() -> LoweredMaterial {
        LoweredMaterial {
            base_color: Channel {
                factor: [1.0; 4],
                texture: None,
            },
            metallic: Channel {
                factor: 1.0,
                texture: None,
            },
            roughness: Channel {
                factor: 1.0,
                texture: None,
            },
            normal: None,
            occlusion: None,
            occlusion_strength: 1.0,
            emissive: Channel {
                factor: [0.0; 3],
                texture: None,
            },
            emissive_strength: 1.0,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            unlit: false,
            ext: MaterialExts::default(),
            vertex_color: None,
            diagnostics: Vec::new(),
        }
    }
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
/// the patterns share (the Principled BSDF node they link into, …) were added by WP 1.11
/// (`#[non_exhaustive]`, additive).
#[non_exhaustive]
pub struct RaiseCx<'a> {
    /// The material whose embedded node tree is being built.
    pub into: &'a mut Material,
    /// Node names taken so far.
    pub names: &'a mut NameAlloc,
    /// Uid of the Principled BSDF the patterns link into.
    pub bsdf: Option<u32>,
    /// Uid of the Material Output.
    pub output: Option<u32>,
    /// The end of the alpha chain built so far (the Principled BSDF's `Alpha`, or the free
    /// input of the alpha clip nodes), set by the alpha pattern before the base-colour pattern
    /// adds the factor nodes and the texture, as the importer's `base_color()` does.
    pub base_alpha_socket: Option<frap_nodes::SocketAddr>,
    /// Upper-right corner of each layout block (`build::calc_locations`).
    pub locations: BTreeMap<&'static str, [f32; 2]>,
}

impl<'a> RaiseCx<'a> {
    /// Context for raising into `into`: `bsdf` / `output` are the first Principled BSDF and
    /// Material Output already in its tree, if any (`raise` rebuilds the graph first).
    pub fn new(into: &'a mut Material, names: &'a mut NameAlloc) -> Self {
        let first = |ty: &str| into.node_tree.first_of_type(ty).map(|n| n.uid);
        let bsdf = first("ShaderNodeBsdfPrincipled");
        let output = first("ShaderNodeOutputMaterial");
        RaiseCx {
            into,
            names,
            bsdf,
            output,
            base_alpha_socket: None,
            locations: BTreeMap::new(),
        }
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

/// Every registered pattern in run order (`order`, then `name` for a stable tie-break).
pub fn patterns_in_order() -> Vec<&'static MaterialPattern> {
    let mut v: Vec<&'static MaterialPattern> = MATERIAL_PATTERNS.iter().collect();
    v.sort_by(|a, b| a.order.cmp(&b.order).then(a.name.cmp(b.name)));
    v
}

/// Lowers a material's node tree by running [`MATERIAL_PATTERNS`] in `order`, starting from
/// [`LoweredMaterial::gltf_default`].
pub fn lower(tree: &NodeTree, mat: &Material, images: &dyn ImageLookup) -> LoweredMaterial {
    let cx = LowerCx::new(tree, mat, images);
    let mut out = LoweredMaterial::gltf_default();
    for p in patterns_in_order() {
        (p.lower)(&cx, &mut out);
    }
    out
}

/// Builds a Blender-style node graph from a lowered material (glTF importer), running every
/// [`MATERIAL_PATTERNS`] entry's `raise` in `order`. The embedded tree is rebuilt from
/// scratch (`Principled BSDF` → `Material Output`, as the importer does after
/// `node_tree.nodes.clear()`); `names` ends up holding every node name of the tree.
/// Law: `lower(raise(lower(x))) == lower(x)`.
pub fn raise(lm: &LoweredMaterial, into: &mut Material, names: &mut NameAlloc) {
    let mut cx = RaiseCx::new(into, names);
    cx.reset_graph();
    cx.locations = build::calc_locations(lm);
    for p in patterns_in_order() {
        if let Some(r) = p.raise {
            r(lm, &mut cx);
        }
    }
}

#[cfg(test)]
mod tests;
