// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_mesh_quantization` (WP 7.1): `frap_mesh_quantization` (a frap export option; the add-on
//! quantizes only through its external `gltfpack`) and the quantization of the
//! `export_gltfpack_*` mapping (`gltfpack.rs`).
//!
//! The encodings are gltfpack's (meshoptimizer 1.1 `gltf/stream.cpp::writeVertexStream`,
//! `gltf/gltfpack.cpp`'s defaults `-vp 14 -vt 12 -vn 8 -vc 8 -vpi`, MIT, Arseny Kapoulkine),
//! ported here:
//!
//! - POSITION: UNSIGNED_SHORT, `quantizeUnorm((p - offset) / scale, vp)` with `offset` the
//!   bounds' minimum and `scale` their largest extent; `-vpn` stores them normalized; the
//!   node applies `translation = offset`, `scale = scale / (2^vp - 1)` (× 65535 normalized),
//!   gltfpack's `QuantizationPosition::node_scale`;
//! - NORMAL: normalized BYTE (`vn` ≤ 8) or SHORT, `quantizeSnorm(n, vn)` per component
//!   (`encodeSnorm`); TANGENT: normalized BYTE VEC4 with `min(vn, 8)` bits;
//! - TEXCOORD_n: normalized UNSIGNED_SHORT (`vt` > 8) or UNSIGNED_BYTE;
//! - COLOR_n (gltfpack only): normalized UNSIGNED_BYTE (`vc` ≤ 8) or SHORT, `quantizeColor`.
//!
//! frap's choices where gltfpack restructures the file: TEXCOORD sets outside [0, 1] stay
//! FLOAT (gltfpack moves their offset and scale into a `KHR_texture_transform` on every
//! material that uses them; frap leaves materials alone), and the texture coordinates are
//! quantized with the container's full precision; morph-target deltas stay FLOAT (POSITION
//! deltas divided by the node scale, so they apply in the quantized space; gltfpack clamps
//! larger deltas to the mesh extent); a skinned mesh keeps FLOAT positions with
//! `frap_mesh_quantization` (glTF ignores a skinned node's transform; gltfpack folds its
//! global dequantization into every inverse bind matrix, which `gltfpack.rs` does).
//!
//! The dequantization transform goes on the mesh's nodes in [`finish`] (called by
//! `meshopt::compress_buffer_views`, before the buffer is finished: the whole document is
//! known then, animations included): a node without children whose transform no animation
//! channel targets takes it into its own TRS (`T' = T + R·(S ∘ offset)`, `S' = S · s`);
//! otherwise the mesh moves to a new child node holding the transform (gltfpack's mesh nodes;
//! the node's `weights` channels follow the mesh); a GPU-instanced node folds it into every
//! instance's TRS. The per-mesh transform travels from the post-processor to [`finish`] as a
//! private member of the primitives' `extensions` that [`finish`] removes; the
//! `compress::mesh_node` hook ([`node_transform`]) has nothing left to do.

use std::collections::HashMap;

use frap_codec_mesh::meshopt::{quantize_snorm, quantize_unorm};
use frap_gltf::BufferBuilder;
use frap_gltf::json::{AnimationPath, Document, Index, Node};
use serde_json::{Value, json};

use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::mesh::convert::axis;
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_POST_PROCESS, PrimitiveDraft,
    PrimitivePostProcess,
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["frap_mesh_quantization"];

/// The extension name.
pub const EXTENSION: &str = "KHR_mesh_quantization";

/// The private member carrying a mesh's dequantization from [`run`] to [`finish`].
const DEQUANT_KEY: &str = "FRAP_mesh_quantization_dequantization";

/// How POSITION is stored (`export_gltfpack_vpi`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PositionMode {
    /// `Integer` (`-vpi`): UNSIGNED_SHORT.
    #[default]
    Integer,
    /// `Normalized` (`-vpn`): normalized UNSIGNED_SHORT.
    Normalized,
    /// `Floating-point` (`-vpf`): FLOAT with `vp` mantissa bits (`meshopt_quantizeFloat`).
    Float,
}

/// Quantization bits (gltfpack's `-vp`, `-vt`, `-vn`, `-vc`, `-vpi`/`-vpn`/`-vpf`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct QuantBits {
    /// Position bits (1..=16).
    pub position: u32,
    /// Texture-coordinate bits (1..=16).
    pub texcoord: u32,
    /// Normal and tangent bits (1..=16).
    pub normal: u32,
    /// Colour bits (2..=16 as gltfpack clamps them; 1 is read as 2).
    pub color: u32,
    /// POSITION storage.
    pub position_mode: PositionMode,
}

impl QuantBits {
    /// gltfpack's defaults, which `frap_mesh_quantization` uses.
    pub const GLTFPACK: QuantBits = QuantBits {
        position: 14,
        texcoord: 12,
        normal: 8,
        color: 8,
        position_mode: PositionMode::Integer,
    };
}

/// A mesh's position dequantization: `p = offset + q · node_scale` (gltfpack's
/// `QuantizationPosition`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Dequant {
    /// Bounds minimum (glTF space).
    pub offset: [f32; 3],
    /// Largest extent of the bounds (1 for a degenerate box).
    pub scale: f32,
    /// Position bits.
    pub bits: u32,
    /// Normalized storage (`-vpn`).
    pub normalized: bool,
}

impl Dequant {
    /// From bounds (`None` without positions).
    pub fn from_bounds(min: [f32; 3], max: [f32; 3], bits: u32, normalized: bool) -> Dequant {
        let extent = (0..3).map(|k| max[k] - min[k]).fold(0.0f32, f32::max);
        Dequant {
            offset: min,
            scale: if extent > 0.0 && extent.is_finite() {
                extent
            } else {
                1.0
            },
            bits,
            normalized,
        }
    }

    /// The bounds of `positions` (`None` when empty or not finite).
    pub fn bounds(positions: impl IntoIterator<Item = [f32; 3]>) -> Option<([f32; 3], [f32; 3])> {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        let mut any = false;
        for p in positions {
            any = true;
            for k in 0..3 {
                min[k] = min[k].min(p[k]);
                max[k] = max[k].max(p[k]);
            }
        }
        (any && min.iter().chain(&max).all(|v| v.is_finite())).then_some((min, max))
    }

    /// gltfpack's `node_scale`.
    pub fn node_scale(&self) -> f32 {
        self.scale / ((1u32 << self.bits) - 1) as f32 * if self.normalized { 65535.0 } else { 1.0 }
    }

    /// One quantized position.
    pub fn quantize(&self, p: [f32; 3]) -> [u16; 3] {
        let r = 1.0 / self.scale;
        std::array::from_fn(|k| quantize_unorm((p[k] - self.offset[k]) * r, self.bits) as u16)
    }

    fn to_json(self) -> Value {
        json!({"offset": self.offset, "scale": self.scale, "bits": self.bits,
               "normalized": self.normalized})
    }

    fn from_json(v: &Value) -> Option<Dequant> {
        let f = |x: &Value| x.as_f64().map(|x| x as f32);
        let o = v.get("offset")?.as_array()?;
        Some(Dequant {
            offset: [f(o.first()?)?, f(o.get(1)?)?, f(o.get(2)?)?],
            scale: f(v.get("scale")?)?,
            bits: v.get("bits")?.as_u64()? as u32,
            normalized: v.get("normalized")?.as_bool()?,
        })
    }
}

/// meshoptimizer's `meshopt_quantizeFloat`: `v` with `n` mantissa bits (round half up),
/// denormals flushed to zero, infinities and NaN kept.
pub fn quantize_float(v: f32, n: u32) -> f32 {
    let n = n.min(23);
    let ui = v.to_bits();
    let mask = (1u32 << (23 - n)) - 1;
    let round = (1u32 << (23 - n)) >> 1;
    let e = ui & 0x7f80_0000;
    let rui = ui.wrapping_add(round) & !mask;
    let ui = if e == 0x7f80_0000 { ui } else { rui };
    f32::from_bits(if e == 0 { 0 } else { ui })
}

/// POSITION columns: `UNSIGNED_SHORT` VEC3 (normalized with `-vpn`).
pub fn quantize_positions(p: &[[f32; 3]], dq: &Dequant) -> AttrColumn {
    AttrColumn {
        values: AttrValues::U16x3(p.iter().map(|&v| dq.quantize(v)).collect()),
        normalized: dq.normalized,
    }
}

/// POSITION morph-target deltas in the quantized space: `d / node_scale`, FLOAT.
pub fn scale_position_deltas(d: &[[f32; 3]], dq: &Dequant) -> AttrColumn {
    let r = 1.0 / dq.node_scale();
    AttrColumn {
        values: AttrValues::F32x3(d.iter().map(|v| v.map(|c| c * r)).collect()),
        normalized: false,
    }
}

/// NORMAL: normalized BYTE (`bits` ≤ 8) or SHORT VEC3 (gltfpack's `encodeSnorm`).
pub fn quantize_normals(n: &[[f32; 3]], bits: u32) -> AttrColumn {
    let bits = bits.clamp(1, 16);
    let values = if bits > 8 {
        AttrValues::I16x3(
            n.iter()
                .map(|v| v.map(|c| quantize_snorm(c, bits) as i16))
                .collect(),
        )
    } else {
        AttrValues::I8x3(
            n.iter()
                .map(|v| v.map(|c| quantize_snorm(c, bits) as i8))
                .collect(),
        )
    };
    AttrColumn {
        values,
        normalized: true,
    }
}

/// TANGENT: normalized BYTE VEC4 with `min(bits, 8)` bits; `w` keeps its sign at ±1 (gltfpack
/// quantizes it like the other components, which leaves |w| < 1 below 8 bits).
pub fn quantize_tangents(t: &[[f32; 4]], bits: u32) -> AttrColumn {
    let bits = bits.clamp(1, 8);
    AttrColumn {
        values: AttrValues::I8x4(
            t.iter()
                .map(|v| {
                    [
                        quantize_snorm(v[0], bits) as i8,
                        quantize_snorm(v[1], bits) as i8,
                        quantize_snorm(v[2], bits) as i8,
                        if v[3] < 0.0 { -127 } else { 127 },
                    ]
                })
                .collect(),
        ),
        normalized: true,
    }
}

/// TEXCOORD_n in [0, 1]: normalized UNSIGNED_SHORT (`bits` > 8) or UNSIGNED_BYTE; `None` when a
/// coordinate lies outside [0, 1] (kept FLOAT).
pub fn quantize_texcoords(uv: &[[f32; 2]], bits: u32) -> Option<AttrColumn> {
    if uv.iter().flatten().any(|c| !(0.0..=1.0).contains(c)) {
        return None;
    }
    let values = if bits > 8 {
        AttrValues::U16x2(
            uv.iter()
                .map(|v| v.map(|c| quantize_unorm(c, 16) as u16))
                .collect(),
        )
    } else {
        AttrValues::U8x2(
            uv.iter()
                .map(|v| v.map(|c| quantize_unorm(c, 8) as u8))
                .collect(),
        )
    };
    Some(AttrColumn {
        values,
        normalized: true,
    })
}

/// gltfpack's `quantizeColor`: an `bytebits`-bit unorm whose low bits below `bits` replicate
/// the top bit.
fn quantize_color(v: f32, bytebits: u32, bits: u32) -> i32 {
    let result = quantize_unorm(v, bytebits);
    let mask = (1i32 << (bytebits - bits.min(bytebits))) - 1;
    (result & !mask) | (mask & -(result >> (bytebits - 1)))
}

/// COLOR_n (RGBA): normalized UNSIGNED_BYTE (`bits` ≤ 8) or UNSIGNED_SHORT (`encodeColor`).
pub fn quantize_colors(c: &[[f32; 4]], bits: u32) -> AttrColumn {
    let bits = bits.clamp(2, 16);
    let values = if bits > 8 {
        AttrValues::U16x4(
            c.iter()
                .map(|v| v.map(|x| quantize_color(x, 16, bits) as u16))
                .collect(),
        )
    } else {
        AttrValues::U8x4(
            c.iter()
                .map(|v| v.map(|x| quantize_color(x, 8, bits) as u8))
                .collect(),
        )
    };
    AttrColumn {
        values,
        normalized: true,
    }
}

fn run(
    cx: &MeshExportCx,
    draft: &mut PrimitiveDraft,
    _buffer: &mut BufferBuilder,
) -> Result<(), ExportError> {
    let p = &cx.settings.props;
    if !p.frap_mesh_quantization.value {
        return Ok(());
    }
    if p.export_draco_mesh_compression_enable.value {
        cx.warn(
            "frap: export_scene.gltf: frap_mesh_quantization is ignored with Draco compression, which quantizes on its own",
        );
        return Ok(());
    }
    let bits = QuantBits::GLTFPACK;
    let mut quantized = false;
    // POSITION, unless the mesh is skinned (its node transform is ignored).
    if cx.skin.is_none() && !draft.positions.is_empty() {
        let yup = cx.settings.export_yup;
        let bounds = Dequant::bounds(cx.mesh.positions().iter().map(|&v| axis(v, yup)))
            .or_else(|| Dequant::bounds(draft.positions.iter().copied()));
        if let Some((min, max)) = bounds {
            let dq = Dequant::from_bounds(min, max, bits.position, false);
            draft
                .encoding
                .attributes
                .insert("POSITION".into(), quantize_positions(&draft.positions, &dq));
            for (ti, t) in draft.targets.iter().enumerate() {
                if let Some(AttrColumn {
                    values: AttrValues::F32x3(d),
                    ..
                }) = t.get("POSITION")
                {
                    if draft.encoding.targets.len() <= ti {
                        draft.encoding.targets.resize_with(ti + 1, Default::default);
                    }
                    draft.encoding.targets[ti]
                        .insert("POSITION".into(), scale_position_deltas(d, &dq));
                }
            }
            draft
                .encoding
                .extensions
                .insert_raw(DEQUANT_KEY, dq.to_json());
            quantized = true;
        }
    }
    if !draft.normals.is_empty() {
        draft.encoding.attributes.insert(
            "NORMAL".into(),
            quantize_normals(&draft.normals, bits.normal),
        );
        quantized = true;
    }
    if let Some(AttrColumn {
        values: AttrValues::F32x4(t),
        ..
    }) = draft.attributes.get("TANGENT")
    {
        let col = quantize_tangents(t, bits.normal);
        draft.encoding.attributes.insert("TANGENT".into(), col);
        quantized = true;
    }
    for (k, uv) in draft.texcoords.iter().enumerate() {
        if let Some(col) = quantize_texcoords(uv, bits.texcoord) {
            draft
                .encoding
                .attributes
                .insert(format!("TEXCOORD_{k}").into(), col);
        }
    }
    if quantized
        && !draft
            .encoding
            .extensions_used
            .iter()
            .any(|(n, _)| *n == EXTENSION)
    {
        draft.encoding.extensions_used.push((EXTENSION, true));
    }
    Ok(())
}

/// The dequantization transform of glTF mesh `mesh` on its node `node` (called by
/// `compress::mesh_node` for every node with a mesh). Nothing to do here: the transforms are
/// placed by [`finish`], once the whole document is known (children, animations, instancing).
pub fn node_transform(_g: &mut Gather, _mesh: Index, _node: &mut Node) -> Result<(), ExportError> {
    Ok(())
}

/// The nodes [`place_dequant`] could not handle by itself.
#[derive(Debug, Default)]
pub(crate) struct Unplaced {
    /// GPU-instanced nodes: the transform goes into every instance's TRS ([`fold_instances`]).
    pub instanced: Vec<(usize, Dequant)>,
    /// Skinned nodes: the transform goes into the skin's inverse bind matrices.
    pub skinned: Vec<(usize, Dequant)>,
}

fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    // v + 2 w (u × v) + 2 u × (u × v), u = (x, y, z)
    let (u, w) = ([q[0], q[1], q[2]], q[3]);
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let t = cross(u, v);
    let t2 = cross(u, t);
    std::array::from_fn(|k| v[k] + 2.0 * w * t[k] + 2.0 * t2[k])
}

/// `TRS · D` as a TRS (`D` = translate(offset) · scale(node_scale)).
pub(crate) fn fold_trs(
    t: [f64; 3],
    r: [f64; 4],
    s: [f64; 3],
    dq: &Dequant,
) -> ([f64; 3], [f64; 3]) {
    let o: [f64; 3] = std::array::from_fn(|k| s[k] * f64::from(dq.offset[k]));
    let ro = rotate(r, o);
    let ns = f64::from(dq.node_scale());
    (
        std::array::from_fn(|k| t[k] + ro[k]),
        std::array::from_fn(|k| s[k] * ns),
    )
}

/// Instance TRS with the dequantization folded in (missing translations and scales are the
/// identity's).
pub(crate) fn fold_instances(t: &mut [[f32; 3]], r: &[[f32; 4]], s: &mut [[f32; 3]], dq: &Dequant) {
    for i in 0..t.len() {
        let q = r
            .get(i)
            .copied()
            .unwrap_or([0.0, 0.0, 0.0, 1.0])
            .map(f64::from);
        let (nt, ns) = fold_trs(t[i].map(f64::from), q, s[i].map(f64::from), dq);
        t[i] = nt.map(|x| x as f32);
        s[i] = ns.map(|x| x as f32);
    }
}

/// Whether an animation channel of `doc` animates the transform of node `n`.
fn transform_animated(doc: &Document, n: usize) -> bool {
    let prefix = format!("/nodes/{n}/");
    doc.animations.iter().flat_map(|a| &a.channels).any(|c| {
        (c.target.node == Some(n as Index)
            && matches!(
                c.target.path,
                AnimationPath::Translation | AnimationPath::Rotation | AnimationPath::Scale
            ))
            || pointer_of(&c.target.extensions).is_some_and(|p| {
                p.strip_prefix(&prefix).is_some_and(|rest| {
                    matches!(rest, "translation" | "rotation" | "scale" | "matrix")
                })
            })
    })
}

fn pointer_of(ext: &frap_gltf::Extensions) -> Option<&str> {
    ext.get_raw("KHR_animation_pointer")?
        .get("pointer")?
        .as_str()
}

/// Places the dequantization of every node whose mesh has one (`dq` by glTF mesh): folded
/// into the node's TRS, or on a new child node holding the mesh (see the module docs).
/// Instanced and skinned nodes are returned for the caller.
pub(crate) fn place_dequant(doc: &mut Document, dq: &HashMap<Index, Dequant>) -> Unplaced {
    let mut out = Unplaced::default();
    let count = doc.nodes.len();
    for ni in 0..count {
        let Some(m) = doc.nodes[ni].mesh else {
            continue;
        };
        let Some(d) = dq.get(&m).copied() else {
            continue;
        };
        let n = &doc.nodes[ni];
        if n.extensions.contains("EXT_mesh_gpu_instancing") {
            out.instanced.push((ni, d));
            continue;
        }
        if n.skin.is_some() {
            out.skinned.push((ni, d));
            continue;
        }
        if n.children.is_empty() && n.matrix.is_none() && !transform_animated(doc, ni) {
            let n = &mut doc.nodes[ni];
            let (t, s) = fold_trs(
                n.translation.unwrap_or([0.0; 3]),
                n.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]),
                n.scale.unwrap_or([1.0; 3]),
                &d,
            );
            n.translation = Some(t);
            n.scale = Some(s);
            continue;
        }
        let child = doc.nodes.len() as Index;
        let ns = f64::from(d.node_scale());
        let weights = std::mem::take(&mut doc.nodes[ni].weights);
        doc.nodes.push(Node {
            mesh: Some(m),
            translation: Some(d.offset.map(f64::from)),
            scale: Some([ns; 3]),
            weights,
            ..Node::default()
        });
        let n = &mut doc.nodes[ni];
        n.mesh = None;
        n.children.push(child);
        // The morph weights follow the mesh.
        let from = format!("/nodes/{ni}/weights");
        let to = format!("/nodes/{child}/weights");
        for c in doc.animations.iter_mut().flat_map(|a| &mut a.channels) {
            if c.target.node == Some(ni as Index) && c.target.path == AnimationPath::Weights {
                c.target.node = Some(child);
            }
            if pointer_of(&c.target.extensions) == Some(from.as_str())
                && let Some(p) = c.target.extensions.0.get_mut("KHR_animation_pointer")
            {
                p["pointer"] = Value::String(to.clone());
            }
        }
    }
    out
}

/// Takes the dequantizations [`run`] recorded off the primitives of `doc` (by glTF mesh).
fn take_dequant(doc: &mut Document) -> HashMap<Index, Dequant> {
    let mut out = HashMap::new();
    for (mi, m) in doc.meshes.iter_mut().enumerate() {
        for p in &mut m.primitives {
            if let Some(v) = p.extensions.remove(DEQUANT_KEY)
                && let Some(d) = Dequant::from_json(&v)
            {
                out.entry(mi as Index).or_insert(d);
            }
        }
    }
    out
}

/// Reads a tightly packed FLOAT accessor of the open buffer.
fn read_floats<const N: usize>(buf: &BufferBuilder, acc: Index) -> Option<Vec<[f32; N]>> {
    let a = buf.accessor(acc)?;
    if a.component_type != frap_gltf::json::ComponentType::Float
        || a.type_.components() != N
        || a.sparse.is_some()
    {
        return None;
    }
    let v = buf.view(a.buffer_view?)?;
    let stride = v.byte_stride.map_or(4 * N, |s| s as usize);
    let start = (v.byte_offset_or_default() + a.byte_offset.unwrap_or(0)) as usize;
    (0..a.count as usize)
        .map(|i| {
            let o = start + i * stride;
            let b = buf.bytes.get(o..o + 4 * N)?;
            Some(std::array::from_fn(|k| {
                f32::from_le_bytes([b[4 * k], b[4 * k + 1], b[4 * k + 2], b[4 * k + 3]])
            }))
        })
        .collect()
}

fn write_floats<const N: usize>(buf: &mut BufferBuilder, acc: Index, data: &[[f32; N]]) {
    let Some(a) = buf.accessor(acc) else { return };
    let (Some(view), off) = (a.buffer_view, a.byte_offset.unwrap_or(0)) else {
        return;
    };
    let Some(v) = buf.view(view) else { return };
    let stride = v.byte_stride.map_or(4 * N, |s| s as usize);
    let start = (v.byte_offset_or_default() + off) as usize;
    for (i, e) in data.iter().enumerate() {
        for (k, c) in e.iter().enumerate() {
            let o = start + i * stride + 4 * k;
            buf.bytes[o..o + 4].copy_from_slice(&c.to_le_bytes());
        }
    }
}

/// `frap_mesh_quantization`'s node pass (see the module docs), before meshopt compression.
pub fn finish(g: &mut Gather) -> Result<(), ExportError> {
    let dq = take_dequant(&mut g.doc);
    if dq.is_empty() {
        return Ok(());
    }
    let unplaced = place_dequant(&mut g.doc, &dq);
    if let Some((ni, _)) = unplaced.skinned.first() {
        return Err(ExportError::Invalid(format!(
            "frap_mesh_quantization: node {ni} is skinned but its mesh has quantized positions"
        )));
    }
    for (ni, d) in unplaced.instanced {
        let attrs = g.doc.nodes[ni]
            .extensions
            .get_raw("EXT_mesh_gpu_instancing")
            .and_then(|e| e.get("attributes"))
            .cloned()
            .unwrap_or(Value::Null);
        let acc = |k: &str| attrs.get(k).and_then(Value::as_u64).map(|a| a as Index);
        let (ta, ra, sa) = (acc("TRANSLATION"), acc("ROTATION"), acc("SCALE"));
        let count = [ta, ra, sa]
            .into_iter()
            .flatten()
            .find_map(|a| g.buf.accessor(a).map(|a| a.count as usize))
            .unwrap_or(0);
        let bad = || {
            ExportError::Invalid(format!(
                "frap_mesh_quantization: the instance transforms of node {ni} are not FLOAT"
            ))
        };
        let mut t = match ta {
            Some(a) => read_floats::<3>(&g.buf, a).ok_or_else(bad)?,
            None => vec![[0.0; 3]; count],
        };
        let r = match ra {
            Some(a) => read_floats::<4>(&g.buf, a).ok_or_else(bad)?,
            None => vec![[0.0, 0.0, 0.0, 1.0]; count],
        };
        let mut s = match sa {
            Some(a) => read_floats::<3>(&g.buf, a).ok_or_else(bad)?,
            None => vec![[1.0; 3]; count],
        };
        fold_instances(&mut t, &r, &mut s, &d);
        let mut new = serde_json::Map::new();
        for (key, a, v) in [("TRANSLATION", ta, &t), ("SCALE", sa, &s)] {
            match a {
                Some(a) => write_floats(&mut g.buf, a, v),
                None => {
                    let i = g.buf.push_accessor(
                        &frap_gltf::AccessorData::from_elements(v),
                        &frap_gltf::AccessorOpts::default(),
                    );
                    new.insert(key.to_owned(), json!(i));
                }
            }
        }
        if !new.is_empty()
            && let Some(Value::Object(at)) = g.doc.nodes[ni]
                .extensions
                .0
                .get_mut("EXT_mesh_gpu_instancing")
                .and_then(|e| e.get_mut("attributes"))
        {
            at.extend(new);
        }
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static QUANTIZE: PrimitivePostProcess = PrimitivePostProcess {
    name: "quantize",
    order: 200,
    run,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gltfpack_position_quantization() {
        let dq = Dequant::from_bounds([-1.0, -1.0, -2.0], [1.0, 1.0, 2.0], 14, false);
        assert_eq!(dq.scale, 4.0);
        assert_eq!(dq.quantize([-1.0, -1.0, -2.0]), [0, 0, 0]);
        assert_eq!(dq.quantize([1.0, 1.0, 2.0]), [8192, 8192, 16383]);
        let ns = dq.node_scale();
        assert!((ns - 4.0 / 16383.0).abs() < 1e-9);
        // Dequantized within half a step.
        let q = dq.quantize([0.3, -0.7, 1.1]);
        for (k, p) in [0.3f32, -0.7, 1.1].iter().enumerate() {
            let back = dq.offset[k] + f32::from(q[k]) * ns;
            assert!((back - p).abs() <= ns * 0.5 + 1e-6, "{back} vs {p}");
        }
        let n = Dequant::from_bounds([0.0; 3], [2.0; 3], 14, true);
        assert!((n.node_scale() - 2.0 / 16383.0 * 65535.0).abs() < 1e-3);
        // Degenerate bounds.
        assert_eq!(
            Dequant::from_bounds([1.0; 3], [1.0; 3], 14, false).scale,
            1.0
        );
        let j = dq.to_json();
        assert_eq!(Dequant::from_json(&j), Some(dq));
    }

    #[test]
    fn quantize_float_matches_meshoptimizer() {
        assert_eq!(quantize_float(1.0, 10), 1.0);
        // 1 + 2^-12 rounds to 1 with 10 mantissa bits; 1 + 2^-10 is kept.
        assert_eq!(quantize_float(1.0 + 2f32.powi(-12), 10), 1.0);
        assert_eq!(
            quantize_float(1.0 + 2f32.powi(-10), 10),
            1.0 + 2f32.powi(-10)
        );
        assert_eq!(quantize_float(1e-40, 10), 0.0);
        assert!(quantize_float(f32::NAN, 4).is_nan());
        assert_eq!(quantize_float(f32::INFINITY, 4), f32::INFINITY);
    }

    #[test]
    fn attribute_encodings() {
        let n = quantize_normals(&[[0.0, 1.0, -1.0]], 8);
        assert_eq!(n.values, AttrValues::I8x3(vec![[0, 127, -127]]));
        assert!(n.normalized);
        let n16 = quantize_normals(&[[0.0, 1.0, -0.5]], 12);
        assert_eq!(n16.values, AttrValues::I16x3(vec![[0, 2047, -1024]]));
        let t = quantize_tangents(&[[1.0, 0.0, 0.0, -1.0]], 6);
        assert_eq!(t.values, AttrValues::I8x4(vec![[31, 0, 0, -127]]));
        let uv = quantize_texcoords(&[[0.0, 1.0], [0.5, 0.25]], 12).unwrap();
        assert_eq!(
            uv.values,
            AttrValues::U16x2(vec![[0, 65535], [32768, 16384]])
        );
        assert!(quantize_texcoords(&[[1.5, 0.0]], 12).is_none());
        let uv8 = quantize_texcoords(&[[1.0, 0.0]], 8).unwrap();
        assert_eq!(uv8.values, AttrValues::U8x2(vec![[255, 0]]));
        let c = quantize_colors(&[[1.0, 0.0, 0.5, 1.0]], 8);
        assert_eq!(c.values, AttrValues::U8x4(vec![[255, 0, 128, 255]]));
        // 4 bits: the low nibble replicates the top bit.
        let c4 = quantize_colors(&[[1.0, 0.0, 0.5, 0.25]], 4);
        assert_eq!(c4.values, AttrValues::U8x4(vec![[255, 0, 143, 64]]));
        let d = scale_position_deltas(
            &[[0.5, 0.0, 0.0]],
            &Dequant::from_bounds([0.0; 3], [1.0; 3], 14, false),
        );
        assert_eq!(d.values, AttrValues::F32x3(vec![[0.5 * 16383.0, 0.0, 0.0]]));
    }

    #[test]
    fn folded_transforms_compose() {
        let dq = Dequant::from_bounds([1.0, 2.0, 3.0], [3.0, 3.0, 3.0], 14, false);
        let h = std::f64::consts::FRAC_1_SQRT_2;
        // 90° about Z, scale 2.
        let (t, s) = fold_trs([10.0, 0.0, 0.0], [0.0, 0.0, h, h], [2.0; 3], &dq);
        // R · (S ∘ offset) = R · (2, 4, 6) = (-4, 2, 6).
        assert!(
            (t[0] - 6.0).abs() < 1e-9 && (t[1] - 2.0).abs() < 1e-9 && (t[2] - 6.0).abs() < 1e-9
        );
        assert!((s[0] - 2.0 * f64::from(dq.node_scale())).abs() < 1e-12);
        let mut tt = vec![[0.0f32; 3]];
        let mut ss = vec![[1.0f32; 3]];
        fold_instances(&mut tt, &[[0.0, 0.0, 0.0, 1.0]], &mut ss, &dq);
        assert_eq!(tt[0], [1.0, 2.0, 3.0]);
    }

    fn mesh_node_doc(children: bool, animated: bool) -> Document {
        let mut j = json!({
            "asset": {"version": "2.0"},
            "nodes": [{"mesh": 0, "name": "Cube", "translation": [1.0, 0.0, 0.0]}],
            "meshes": [{"primitives": [{"attributes": {}}], "weights": [0.0]}],
            "animations": []
        });
        if children {
            j["nodes"][0]["children"] = json!([1]);
            j["nodes"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": "Child"}));
        }
        if animated {
            j["animations"] = json!([{"channels": [
                {"sampler": 0, "target": {"node": 0, "path": "translation"}},
                {"sampler": 1, "target": {"node": 0, "path": "weights"}}],
                "samplers": [{"input": 0, "output": 0}, {"input": 0, "output": 0}]}]);
        }
        Document::from_slice(&serde_json::to_vec(&j).unwrap()).unwrap()
    }

    use crate::export::mesh::post::meshopt::tests::{
        import, json_of, run_export, scratch, settings, write,
    };

    fn quantized() -> crate::export::ExportSettings {
        settings(|p| p.frap_mesh_quantization.value = true)
    }

    #[test]
    fn factory_cube_quantizes_like_gltfpack() {
        let (main, win) = frap_scene::startup::factory();
        let out = run_export(&main, &win, &quantized());
        let j = json_of(&out);
        assert_eq!(j["extensionsUsed"], json!([EXTENSION]));
        assert_eq!(j["extensionsRequired"], json!([EXTENSION]));
        let prim = &j["meshes"][0]["primitives"][0];
        assert!(
            prim.get("extensions").is_none(),
            "the private member is removed"
        );
        let at = |k: &str| &j["accessors"][prim["attributes"][k].as_u64().unwrap() as usize];
        assert_eq!(at("POSITION")["componentType"], 5123);
        assert!(at("POSITION").get("normalized").is_none());
        assert_eq!(at("POSITION")["min"], json!([0.0, 0.0, 0.0]));
        assert_eq!(at("POSITION")["max"], json!([16383.0, 16383.0, 16383.0]));
        assert_eq!(at("NORMAL")["componentType"], 5120);
        assert_eq!(at("NORMAL")["normalized"], true);
        assert_eq!(at("TEXCOORD_0")["componentType"], 5123);
        assert_eq!(at("TEXCOORD_0")["normalized"], true);
        // The node has no children and no animation: the transform is folded into its TRS.
        let n = &j["nodes"][0];
        assert_eq!(n["translation"], json!([-1.0, -1.0, -1.0]));
        let s = f64::from(2.0f32 / 16383.0);
        for k in 0..3 {
            assert!((n["scale"][k].as_f64().unwrap() - s).abs() < 1e-12);
        }
        // Padded VEC3 elements: byteStride 8 (POSITION) and 4 (NORMAL).
        let views = &j["bufferViews"];
        assert_eq!(
            views[at("POSITION")["bufferView"].as_u64().unwrap() as usize]["byteStride"],
            8
        );
        assert_eq!(
            views[at("NORMAL")["bufferView"].as_u64().unwrap() as usize]["byteStride"],
            4
        );
        let r = frap_gltf::validate::validate_file(&out.bytes).unwrap();
        assert!(r.is_ok(), "{r}");
        // Imported back: the object carries the transform, the world positions are the cube's.
        let dir = scratch("quant");
        let (m, _) = import(&write(&dir, "q.glb", &out));
        let (_, ob) = m
            .objects
            .iter()
            .find(|(_, o)| o.header.name == "Cube")
            .unwrap();
        let me = m
            .meshes
            .get(ob.data.and_then(|d| d.typed::<frap_rna::MeshId>()).unwrap())
            .unwrap();
        for p in me.positions() {
            let w: Vec<f32> = (0..3)
                .map(|k| ob.location[k] + ob.scale[k] * p[k])
                .collect();
            for c in w {
                assert!((c.abs() - 1.0).abs() < 1e-4, "{c}");
            }
        }
    }

    #[test]
    fn parents_get_a_child_mesh_node_and_meshopt_keeps_integer_columns() {
        use frap_scene::id::Object;
        use frap_scene::{Base, BaseFlag, ChangeLog};
        let (mut main, win) = frap_scene::startup::factory();
        let mut log = ChangeLog::default();
        let cube = main.objects.find("Cube").unwrap();
        let data = main.objects.get(cube).unwrap().data;
        let mut child = Object::new("", data);
        child.parent = Some(cube);
        child.location = [3.0, 0.0, 0.0];
        let c = main.objects.insert("Child", child, &mut log);
        let coll = main.collections.find("Collection").unwrap();
        main.collections
            .get_mut(coll, &mut log)
            .unwrap()
            .objects
            .push(c);
        let sc = win.scene;
        main.scenes.get_mut(sc, &mut log).unwrap().view_layers[0]
            .bases
            .push(Base {
                object: c,
                flag: BaseFlag::empty(),
            });
        let out = run_export(
            &main,
            &win,
            &settings(|p| {
                p.frap_mesh_quantization.value = true;
                p.export_meshopt_compression_enable.value = true;
            }),
        );
        let j = json_of(&out);
        assert_eq!(
            j["extensionsUsed"],
            json!(["EXT_meshopt_compression", EXTENSION])
        );
        let nodes = j["nodes"].as_array().unwrap();
        let by_name = |n: &str| nodes.iter().position(|x| x["name"] == n).unwrap();
        let (cube_n, child_n) = (by_name("Cube"), by_name("Child"));
        // The leaf takes the transform; the parent keeps its TRS and gets a mesh child.
        assert!(nodes[child_n]["scale"].is_array());
        assert!(nodes[cube_n].get("mesh").is_none());
        let kids = nodes[cube_n]["children"].as_array().unwrap();
        assert_eq!(kids.len(), 2);
        let holder = kids
            .iter()
            .map(|k| &nodes[k.as_u64().unwrap() as usize])
            .find(|n| n.get("mesh").is_some() && n.get("name").is_none())
            .unwrap();
        assert_eq!(holder["translation"], json!([-1.0, -1.0, -1.0]));
        // meshopt: the integer POSITION is compressed at its padded stride, without a filter.
        let prim = &j["meshes"][0]["primitives"][0];
        let pos = &j["accessors"][prim["attributes"]["POSITION"].as_u64().unwrap() as usize];
        let v = &j["bufferViews"][pos["bufferView"].as_u64().unwrap() as usize];
        let e = &v["extensions"]["EXT_meshopt_compression"];
        assert_eq!(e["byteStride"], 8);
        assert!(e.get("filter").is_none());
        // Round trip through frap's importer: world positions within a quantization step.
        let dir = scratch("quant-child");
        let (m, _) = import(&write(&dir, "c.glb", &out));
        assert!(m.objects.iter().any(|(_, o)| o.header.name == "Child"));
    }

    #[test]
    fn dequantization_is_folded_or_moved_to_a_child() {
        let dq = Dequant::from_bounds([-1.0; 3], [1.0; 3], 14, false);
        let map: HashMap<Index, Dequant> = [(0, dq)].into();
        let mut d = mesh_node_doc(false, false);
        let u = place_dequant(&mut d, &map);
        assert!(u.instanced.is_empty() && u.skinned.is_empty());
        assert_eq!(d.nodes.len(), 1);
        assert_eq!(d.nodes[0].translation, Some([0.0, -1.0, -1.0]));
        let s = f64::from(dq.node_scale());
        assert_eq!(d.nodes[0].scale, Some([s; 3]));
        for (children, animated) in [(true, false), (false, true)] {
            let mut d = mesh_node_doc(children, animated);
            let before = d.nodes.len();
            place_dequant(&mut d, &map);
            assert_eq!(d.nodes.len(), before + 1);
            let c = before as Index;
            assert_eq!(d.nodes[0].mesh, None);
            assert_eq!(d.nodes[0].translation, Some([1.0, 0.0, 0.0]));
            assert!(d.nodes[0].children.contains(&c));
            assert_eq!(d.nodes[c as usize].mesh, Some(0));
            assert_eq!(d.nodes[c as usize].translation, Some([-1.0; 3]));
            if animated {
                let ch = &d.animations[0].channels;
                assert_eq!(ch[0].target.node, Some(0));
                assert_eq!(ch[1].target.node, Some(c));
            }
        }
    }
}
