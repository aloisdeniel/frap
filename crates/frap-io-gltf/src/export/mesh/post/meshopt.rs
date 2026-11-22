// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_meshopt_compression` / `KHR_meshopt_compression` (WP 7.1):
//! `export_meshopt_compression_enable`, `export_meshopt_extension`. The add-on compresses at
//! the buffer level (`blender/exp/exporter.py::finalize_buffer`, `io/exp/meshopt.py`): the
//! views go to a second buffer compressed, the first becomes a `fallback` buffer without data.
//! `compress.rs` calls [`compress_buffer_views`] once, before the buffer is finished; it finds
//! what each view holds from the finished document (the meshes' attributes, indices and morph
//! targets, the GPU-instancing attributes of the nodes, the animation samplers), so the
//! per-primitive post-processor of this file has nothing to do.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2 5.2.41 (Apache-2.0, The
//! glTF-Blender-IO authors): `blender/exp/accessors.py::array_to_accessor`,
//! `blender/exp/primitives.py::__gather_indices`, `blender/exp/exporter.py`
//! (`finalize_buffer`, `add_meshopt_extension`, `manage_gpu_instancing_nodes`),
//! `io/exp/buffer.py::add_and_get_view` and `io/exp/meshopt.py`:
//!
//! - compressed: the views the add-on builds through `array_to_accessor` with an attribute
//!   name (POSITION, NORMAL, TANGENT, TEXCOORD_n, JOINTS_n, WEIGHTS_n, float `COLOR_n` and
//!   `_`-attributes, dense morph targets `SK_*`), the primitive indices (mode `TRIANGLES` for
//!   triangle lists, `INDICES` otherwise), the GPU-instancing TRS, and the animation samplers
//!   the add-on's sampler modules encode themselves (`blender/exp/animation/fcurves/sampler.py`,
//!   `sampled/{object,armature,shapekeys,data}/sampler.py`): every input as `TIME`, the
//!   `location`/`scale` outputs as `POSITION`/`SCALE`, rotations as `ROTATION`, shape-key
//!   weights as `SK_ANIM` ([`anim_output`]); not compressed: the integer colours (`COLOR_n` and
//!   `_`/`KHR_` attributes as UNSIGNED_SHORT, the fake white `COLOR_0` as UNSIGNED_BYTE,
//!   written as raw `BinaryData`), sparse morph targets, the outputs of
//!   `KHR_animation_pointer` channels (`sampled/data/sampler.py` encodes only their input),
//!   inverse bind matrices and images;
//! - filters: `EXPONENTIAL` (12 bits, shared exponent per vector) for float POSITION, NORMAL,
//!   TANGENT, morph targets, instance translations and scales, and the animation times,
//!   translations, scales and weights; `QUATERNION` (8 bits) for instance and animation
//!   rotations (their accessor becomes normalized SHORT; byteStride 8, but 16 for the sampled
//!   object rotations, whose filtered quaternions fill the first half of the encoded array),
//!   none otherwise; the vertex
//!   codec version is 0 for EXT and 1 for KHR, the index codec version 1, level 2
//!   (meshoptimizer's default);
//! - layout: for a GLB the data buffer is buffer 0 (the BIN chunk) and the fallback buffer 1;
//!   for `GLTF_SEPARATE` the fallback is buffer 0 and the data (`<stem>.bin`) buffer 1; every
//!   view keeps its place; a compressed view points at the fallback buffer, at the offset its
//!   uncompressed bytes would have there (padded to 4), and its extension at the stream in the
//!   data buffer; an uncompressed view points at its bytes in the data buffer; the fallback
//!   buffer has `fallback: true`, no `uri` and the sum of those lengths;
//! - the extension is declared used and required whenever compression is on, before anything
//!   else (`__gather_gltf`).
//!
//! frap's departures, each toward a valid file: the strides and filters follow the actual
//! accessor types (the add-on hard-codes 12 bytes and `EXPONENTIAL` for POSITION/NORMAL, which
//! would be wrong for the integer columns of `frap_mesh_quantization`, and 16 bytes for any
//! non-byte weights); a view whose layout meshopt cannot encode (a stride that is not a
//! multiple of 4, UNSIGNED_BYTE indices of `frap_compat='SPEC'`) stays uncompressed;
//! `GLTF_EMBEDDED` lays the buffers out like a GLB (data first: `write.rs` embeds buffer 0);
//! `frap_compat='SPEC'` writes the sampled object rotations at the specification's byteStride
//! 8; the second half of a stride-16 rotation array is zeros (the add-on's is `np.empty`);
//! with nothing compressed no empty fallback buffer is written (the add-on writes one with
//! `byteLength` 0, which the specification forbids); and the quaternion-filtered rotation
//! view's fallback length is the filtered one (the add-on keeps the float length).
//!
//! The inverse, [`decode_document`], is shared by the importer (`import/decompress/meshopt.rs`)
//! and `export_use_gltfpack` (`export/gltfpack.rs`).

use frap_codec_mesh::meshopt::{
    self as codec, ExpMode, MeshoptCodec, MeshoptFilter, MeshoptMode, MeshoptVersion,
};
use frap_gltf::ext::ext_meshopt_compression::{
    ExtMeshoptCompression, ExtMeshoptCompressionBuffer, MESHOPT_EXTENSIONS, is_fallback_buffer,
    view_compression,
};
use frap_gltf::ext::khr_meshopt_compression::{KhrMeshoptCompression, KhrMeshoptCompressionBuffer};
use frap_gltf::json::{Accessor, AccessorType, Buffer, ComponentType, Index, Mode};
use frap_gltf::{BufferBuilder, Document};
use frap_spec_ops_io::export_scene::gltf::{ExportMeshoptExtension, FrapCompat};

use crate::export::gather::Gather;
use crate::export::mesh::ext::{
    MeshExportCx, PRIMITIVE_POST_PROCESS, PrimitiveDraft, PrimitivePostProcess,
};
use crate::export::{ExportError, ExportFormat};

/// The add-on's `EXP_FILTER_BITS`.
const EXP_FILTER_BITS: u32 = 12;
/// The add-on's `QUAT_FILTER_BITS`.
const QUAT_FILTER_BITS: u32 = 8;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_meshopt_compression_enable",
    "export_meshopt_extension",
];

fn run(
    _cx: &MeshExportCx,
    _draft: &mut PrimitiveDraft,
    _buffer: &mut BufferBuilder,
) -> Result<(), ExportError> {
    // Compression is buffer-level (`compress_buffer_views`).
    Ok(())
}

/// What a compressed view holds and how it is encoded.
#[derive(Clone, Debug, PartialEq)]
struct Plan {
    mode: MeshoptMode,
    filter: MeshoptFilter,
    stride: usize,
    count: usize,
    /// Quaternion-filtered rotation (GPU instancing): its accessor becomes normalized SHORT.
    rotation: Option<Index>,
}

/// How meshopt encodes an animation sampler's output: the add-on's sampler modules call
/// `MeshoptEncoder.encode_attribute` themselves (`blender/exp/animation/fcurves/sampler.py`,
/// `sampled/{object,armature,shapekeys}/sampler.py`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AnimOutput {
    /// `POSITION`, `SCALE` (VEC3, stride 12) and `SK_ANIM` (scalar weights, stride 4).
    Exponential,
    /// `ROTATION`: the 8-bit quaternion filter at this byteStride (8, or 16 for the sampled
    /// object path); the accessor becomes normalized SHORT.
    Quaternion {
        /// The extension's byteStride.
        stride: usize,
    },
}

impl AnimOutput {
    /// Whether the accessor becomes normalized SHORT.
    pub(crate) fn is_quaternion(self) -> bool {
        matches!(self, AnimOutput::Quaternion { .. })
    }
}

/// The animation outputs to compress, by accessor (first writer wins, as the add-on's
/// `@cached gather_accessor` keeps the first `BinaryData` of equal bytes).
#[derive(Default)]
struct AnimOutputs(std::collections::HashMap<Index, AnimOutput>);

/// The compression of an output of `origin` animating `path`, `None` without
/// `export_meshopt_compression_enable`. Every add-on module encodes `location` and `scale`
/// as `POSITION`/`SCALE` (EXPONENTIAL, stride 12) and rotations as `ROTATION` (QUATERNION,
/// stride 8; 16 on the sampled object path, `sampled/object/sampler.py`, which
/// `frap_compat='SPEC'` writes at the specification's 8); shape-key weights are `SK_ANIM`
/// (EXPONENTIAL, stride 4). Any other channel gets the add-on's WARNING and stays
/// uncompressed. The `key_blocks` channels of `fcurves/sampler.py`
/// (`export_force_sampling=False`) are `SK_ANIM` as the add-on intends: its
/// `__convert_keyframes` raises `UnboundLocalError` there (Blender 5.2.4; deviations §2).
pub(crate) fn anim_output(
    g: &mut Gather,
    origin: crate::export::animation::SamplerOrigin,
    path: &frap_gltf::json::animation::AnimationPath,
) -> Option<AnimOutput> {
    use crate::export::animation::SamplerOrigin as O;
    use frap_gltf::json::animation::AnimationPath as P;
    if !g.settings.props.export_meshopt_compression_enable.value {
        return None;
    }
    let spec = g.settings.props.frap_compat.value == FrapCompat::Spec;
    match (origin, path) {
        (_, P::Translation | P::Scale) => Some(AnimOutput::Exponential),
        (O::Object, P::Rotation) if !spec => Some(AnimOutput::Quaternion { stride: 16 }),
        (_, P::Rotation) => Some(AnimOutput::Quaternion { stride: 8 }),
        (O::ShapeKeys | O::Fcurves, P::Weights) => Some(AnimOutput::Exponential),
        _ => {
            g.warn(format!(
                "Meshopt compression for channel {} is not supported, skipping compression for this channel",
                path.as_str()
            ));
            None
        }
    }
}

/// Records that animation output accessor `acc` is compressed as `kind`.
pub(crate) fn record_anim_output(g: &mut Gather, acc: Index, kind: AnimOutput) {
    g.state::<AnimOutputs>().0.entry(acc).or_insert(kind);
}

/// The extension name `export_meshopt_extension` selects, and its vertex codec version.
pub fn extension(g: &Gather) -> (&'static str, MeshoptVersion) {
    match g.settings.props.export_meshopt_extension.value {
        ExportMeshoptExtension::KhrMeshoptCompression => {
            ("KHR_meshopt_compression", MeshoptVersion::Khr)
        }
        _ => ("EXT_meshopt_compression", MeshoptVersion::Ext),
    }
}

/// Compresses the buffer views of the export (see the module docs). Runs the node pass of
/// `frap_mesh_quantization` first (`quantize::finish`), which may add instance accessors.
pub fn compress_buffer_views(g: &mut Gather) -> Result<(), ExportError> {
    crate::export::mesh::post::quantize::finish(g)?;
    if !g.settings.props.export_meshopt_compression_enable.value {
        return Ok(());
    }
    let (name, version) = extension(g);
    let Some(meshopt) = codec::codec() else {
        g.warn(format!(
            "frap: export_scene.gltf: {}; exported without {name}",
            codec::unavailable()
        ));
        return Ok(());
    };
    // `add_meshopt_extension`, whatever the content.
    g.use_extension(name, true);
    if g.buf.views.is_empty() {
        return Ok(());
    }
    let anim = std::mem::take(&mut g.state::<AnimOutputs>().0);
    let plans = plan_views(&g.doc, &g.buf, &anim);
    if plans.iter().all(Option::is_none) {
        return Ok(());
    }
    let (data_index, fallback_index): (Index, Index) = match g.settings.export_format {
        ExportFormat::GltfSeparate => (1, 0),
        ExportFormat::Glb | ExportFormat::GltfEmbedded => (0, 1),
    };
    let old = std::mem::take(&mut g.buf.bytes);
    let mut data: Vec<u8> = Vec::with_capacity(old.len() / 2);
    let mut fallback_len: u64 = 0;
    for (vi, plan) in plans.iter().enumerate() {
        frap_math::budget::check();
        let view = &g.buf.views[vi];
        let start = view.byte_offset_or_default() as usize;
        let raw = &old[start..start + view.byte_length as usize];
        match plan {
            None => {
                let off = data.len() as u64;
                data.extend_from_slice(raw);
                pad4(&mut data);
                let view = &mut g.buf.views[vi];
                view.buffer = data_index;
                view.byte_offset = Some(off);
            }
            Some(p) => {
                let (stream, mut raw_len) = encode_view(meshopt, raw, p, version)?;
                // `manage_gpu_instancing_nodes` keeps the FLOAT data's length on the fallback
                // view of a quaternion-filtered ROTATION (its accessor becomes SHORT, so the
                // decoded data is half as long); `frap_compat='SPEC'` writes the decoded length
                // (M7 W1 integration, verified on Blender 5.2.4).
                if p.rotation.is_some() && g.settings.props.frap_compat.value != FrapCompat::Spec {
                    raw_len = raw.len();
                }
                let off = data.len() as u64;
                data.extend_from_slice(&stream);
                pad4(&mut data);
                let ext = ExtMeshoptCompression {
                    buffer: data_index,
                    byte_offset: Some(off),
                    byte_length: stream.len() as u64,
                    byte_stride: p.stride as u32,
                    count: p.count as u32,
                    mode: p.mode.identifier().to_owned(),
                    filter: (p.filter != MeshoptFilter::None)
                        .then(|| p.filter.identifier().to_owned()),
                    ..ExtMeshoptCompression::default()
                };
                let view = &mut g.buf.views[vi];
                view.buffer = fallback_index;
                view.byte_offset = Some(fallback_len);
                view.byte_length = raw_len as u64;
                fallback_len += (raw_len as u64).div_ceil(4) * 4;
                set_view_extension(&mut view.extensions, version, ext);
                if let Some(acc) = p.rotation
                    && let Some(a) = g.buf.accessor_mut(acc)
                {
                    a.component_type = ComponentType::Short;
                    a.normalized = Some(true);
                }
            }
        }
    }
    g.buf.bytes = data;
    g.buf.buffer = data_index;
    let mut fallback = Buffer {
        byte_length: fallback_len,
        ..Buffer::default()
    };
    match version {
        MeshoptVersion::Ext => fallback.extensions.set(Some(ExtMeshoptCompressionBuffer {
            fallback: Some(true),
            ..ExtMeshoptCompressionBuffer::default()
        })),
        MeshoptVersion::Khr => fallback.extensions.set(Some(KhrMeshoptCompressionBuffer {
            fallback: Some(true),
            ..KhrMeshoptCompressionBuffer::default()
        })),
    }
    // `BufferBuilder::finish` creates (or updates) buffer `data_index`.
    g.doc.buffers = if data_index == 0 {
        vec![Buffer::default(), fallback]
    } else {
        vec![fallback]
    };
    Ok(())
}

fn pad4(v: &mut Vec<u8>) {
    while !v.len().is_multiple_of(4) {
        v.push(0);
    }
}

fn set_view_extension(
    ext: &mut frap_gltf::Extensions,
    version: MeshoptVersion,
    e: ExtMeshoptCompression,
) {
    match version {
        MeshoptVersion::Ext => ext.set(Some(e)),
        MeshoptVersion::Khr => ext.set(Some(KhrMeshoptCompression::from(e))),
    }
}

fn f32s(raw: &[u8]) -> Vec<f32> {
    raw.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The compressed stream of one view and the length of its uncompressed (fallback) data.
fn encode_view(
    meshopt: &dyn MeshoptCodec,
    raw: &[u8],
    p: &Plan,
    version: MeshoptVersion,
) -> Result<(Vec<u8>, usize), ExportError> {
    let err = |e: frap_codec_mesh::CodecError| ExportError::Invalid(format!("meshopt: {e}"));
    match p.mode {
        MeshoptMode::Attributes => {
            let filtered;
            let bytes: &[u8] = match p.filter {
                MeshoptFilter::Exponential => {
                    filtered = codec::encode_filter_exp(
                        &f32s(&raw[..p.count * p.stride]),
                        p.count,
                        p.stride,
                        EXP_FILTER_BITS,
                        ExpMode::SharedVector,
                    )
                    .map_err(err)?;
                    &filtered
                }
                MeshoptFilter::Quaternion => {
                    let mut q = codec::encode_filter_quat(
                        &f32s(&raw[..p.count * 16]),
                        p.count,
                        QUAT_FILTER_BITS,
                    )
                    .map_err(err)?;
                    // At byteStride 16 (the sampled object rotations) the add-on encodes a
                    // `count * 16`-byte array whose first half `encodeFilterQuat` filled
                    // (meshoptimizer ignores the stride); its second half is `np.empty`'s,
                    // zeros here.
                    q.resize(p.count * p.stride, 0);
                    filtered = q;
                    &filtered
                }
                _ => &raw[..p.count * p.stride],
            };
            let stream = meshopt
                .encode_vertex(bytes, p.count, p.stride, version)
                .map_err(err)?;
            Ok((stream, bytes.len()))
        }
        MeshoptMode::Triangles | MeshoptMode::Indices => {
            let indices: Vec<u32> = match p.stride {
                2 => raw
                    .chunks_exact(2)
                    .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
                    .collect(),
                _ => raw
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect(),
            };
            let indices = &indices[..p.count];
            // `vertex_count = int(data.max()) + 1`, as the add-on.
            let vertex_count = indices.iter().max().map_or(0, |&m| m as usize + 1);
            let stream = meshopt
                .encode_index(indices, vertex_count, p.stride, p.mode)
                .map_err(err)?;
            Ok((stream, raw.len()))
        }
    }
}

/// Element size of an accessor in a tightly packed view.
fn element_size(a: &Accessor) -> usize {
    frap_gltf::accessor::element_size(a.component_type, a.type_)
}

/// One plan per view of `buf` (`None`: written uncompressed), from the document's meshes,
/// instancing nodes and animation samplers (`anim`: the outputs to compress).
fn plan_views(
    doc: &Document,
    buf: &BufferBuilder,
    anim: &std::collections::HashMap<Index, AnimOutput>,
) -> Vec<Option<Plan>> {
    let mut plans: Vec<Option<Plan>> = vec![None; buf.views.len()];
    // A view several accessors share (interleaved, `frap_interleave`) is compressed once,
    // without a filter.
    let mut users = vec![0usize; buf.views.len()];
    for a in &buf.accessors {
        if let Some(v) = a.buffer_view
            && let Some(u) = v
                .checked_sub(buf.view_base)
                .and_then(|i| users.get_mut(i as usize))
        {
            *u += 1;
        }
    }
    let view_of = |acc: Index| -> Option<(usize, &Accessor)> {
        let a = buf.accessor(acc)?;
        if a.sparse.is_some() {
            return None;
        }
        let v = a.buffer_view?.checked_sub(buf.view_base)? as usize;
        (v < buf.views.len()).then_some((v, a))
    };
    let attribute = |plans: &mut Vec<Option<Plan>>, acc: Index, filterable: bool| {
        let Some((v, a)) = view_of(acc) else {
            return;
        };
        let view = &buf.views[v];
        let stride = view
            .byte_stride
            .map_or_else(|| element_size(a), |s| s as usize);
        let shared = users[v] > 1;
        if shared && a.byte_offset.unwrap_or(0) != 0 {
            // Planned by the accessor at offset 0 of this interleaved view.
            return;
        }
        let float = a.component_type == ComponentType::Float;
        let filter = if filterable && float && !shared {
            MeshoptFilter::Exponential
        } else {
            MeshoptFilter::None
        };
        let count = a.count as usize;
        if stride == 0
            || !stride.is_multiple_of(4)
            || stride > 256
            || (count * stride) as u64 > view.byte_length
        {
            return;
        }
        plans[v].get_or_insert(Plan {
            mode: MeshoptMode::Attributes,
            filter,
            stride,
            count,
            rotation: None,
        });
    };
    for m in &doc.meshes {
        for p in &m.primitives {
            for (sem, &acc) in &p.attributes {
                let Some(a) = buf.accessor(acc) else { continue };
                let integer_colour = ((sem.starts_with("COLOR_")
                    || sem.starts_with('_')
                    || sem.starts_with("KHR_"))
                    && a.component_type == ComponentType::UnsignedShort)
                    || (sem.starts_with("COLOR_")
                        && a.component_type == ComponentType::UnsignedByte);
                if integer_colour {
                    continue;
                }
                let filterable = matches!(sem.as_str(), "POSITION" | "NORMAL" | "TANGENT");
                attribute(&mut plans, acc, filterable);
            }
            if let Some(acc) = p.indices
                && let Some((v, a)) = view_of(acc)
                && users[v] == 1
            {
                let stride = match a.component_type {
                    ComponentType::UnsignedShort => 2,
                    ComponentType::UnsignedInt => 4,
                    _ => 0,
                };
                let count = a.count as usize;
                let triangles = matches!(p.mode, None | Some(Mode::Triangles));
                let mode = if triangles && count.is_multiple_of(3) {
                    MeshoptMode::Triangles
                } else {
                    MeshoptMode::Indices
                };
                if stride != 0 && (count * stride) as u64 <= buf.views[v].byte_length {
                    plans[v].get_or_insert(Plan {
                        mode,
                        filter: MeshoptFilter::None,
                        stride,
                        count,
                        rotation: None,
                    });
                }
            }
            for t in &p.targets {
                for &acc in t.values() {
                    attribute(&mut plans, acc, true);
                }
            }
        }
    }
    // `manage_gpu_instancing_nodes` (WP 7.5's `EXT_mesh_gpu_instancing`).
    for n in &doc.nodes {
        let Some(attrs) = n
            .extensions
            .get_raw("EXT_mesh_gpu_instancing")
            .and_then(|e| e.get("attributes"))
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        for (sem, acc) in attrs {
            let Some(acc) = acc.as_u64().map(|a| a as Index) else {
                continue;
            };
            match sem.as_str() {
                "TRANSLATION" | "SCALE" => attribute(&mut plans, acc, true),
                "ROTATION" => {
                    if let Some((v, a)) = view_of(acc)
                        && users[v] == 1
                        && a.component_type == ComponentType::Float
                        && a.type_ == AccessorType::Vec4
                        && buf.views[v].byte_stride.is_none()
                    {
                        plans[v].get_or_insert(Plan {
                            mode: MeshoptMode::Attributes,
                            filter: MeshoptFilter::Quaternion,
                            stride: 8,
                            count: a.count as usize,
                            rotation: Some(acc),
                        });
                    }
                }
                _ => attribute(&mut plans, acc, false),
            }
        }
    }
    // The animation samplers: every add-on sampler module encodes its input as `TIME`
    // (EXPONENTIAL, stride 4), and its outputs as `anim` records them.
    let float_view = |acc: Index, ty: AccessorType| -> Option<(usize, &Accessor)> {
        let (v, a) = view_of(acc)?;
        (users[v] == 1
            && a.component_type == ComponentType::Float
            && a.type_ == ty
            && buf.views[v].byte_stride.is_none())
        .then_some((v, a))
    };
    for an in &doc.animations {
        for s in &an.samplers {
            if let Some((v, a)) = float_view(s.input, AccessorType::Scalar) {
                plans[v].get_or_insert(Plan {
                    mode: MeshoptMode::Attributes,
                    filter: MeshoptFilter::Exponential,
                    stride: 4,
                    count: a.count as usize,
                    rotation: None,
                });
            }
            let Some(&kind) = anim.get(&s.output) else {
                continue;
            };
            let Some(ty) = buf.accessor(s.output).map(|a| a.type_) else {
                continue;
            };
            let Some((v, a)) = float_view(s.output, ty) else {
                continue;
            };
            let count = a.count as usize;
            let plan = match kind {
                AnimOutput::Exponential => Plan {
                    mode: MeshoptMode::Attributes,
                    filter: MeshoptFilter::Exponential,
                    stride: element_size(a),
                    count,
                    rotation: None,
                },
                AnimOutput::Quaternion { stride } if ty == AccessorType::Vec4 => Plan {
                    mode: MeshoptMode::Attributes,
                    filter: MeshoptFilter::Quaternion,
                    stride,
                    count,
                    rotation: Some(s.output),
                },
                AnimOutput::Quaternion { .. } => continue,
            };
            plans[v].get_or_insert(plan);
        }
    }
    plans
}

/// Why [`decode_document`] failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshoptDecodeError {
    /// No codec in this build and no fallback data to read instead.
    Unavailable(String),
    /// Corrupt or inconsistent data.
    Data(String),
}

/// What [`decode_document`] did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Views decoded.
    pub views: usize,
    /// Without a codec, the extension whose views were read from their fallback data.
    pub fallback_used: Option<&'static str>,
}

/// Decodes every meshopt view of `doc` into one new buffer appended to `doc.buffers` and
/// `buffers` (no `uri`), re-points the views at it and removes their extension (the
/// importer's `decompress/meshopt.rs` documents the rules). Without a codec, views whose
/// fallback data is present keep it (the extension is removed); others fail.
pub fn decode_document(
    doc: &mut Document,
    buffers: &mut Vec<Vec<u8>>,
) -> Result<Decoded, MeshoptDecodeError> {
    let mut out = Decoded::default();
    let mut data: Vec<u8> = Vec::new();
    let new_buffer = Index::try_from(doc.buffers.len())
        .map_err(|_| MeshoptDecodeError::Data("too many buffers".into()))?;
    for vi in 0..doc.buffer_views.len() {
        let (name, ext) = match view_compression(&doc.buffer_views[vi].extensions) {
            Ok(Some(c)) => c,
            Ok(None) => continue,
            Err(e) => return Err(MeshoptDecodeError::Data(format!("bufferView {vi}: {e}"))),
        };
        frap_math::budget::check_now();
        let Some(meshopt) = codec::codec() else {
            let bv = &doc.buffer_views[vi];
            let fallback = doc
                .buffers
                .get(bv.buffer as usize)
                .is_some_and(|b| is_fallback_buffer(&b.extensions));
            let present = buffers.get(bv.buffer as usize).is_some_and(|b| {
                b.len() as u64 >= bv.byte_offset_or_default().saturating_add(bv.byte_length)
            });
            if fallback || !present {
                return Err(MeshoptDecodeError::Unavailable(format!(
                    "{name}: {} (bufferView {vi} has no fallback data)",
                    codec::unavailable()
                )));
            }
            for n in MESHOPT_EXTENSIONS {
                doc.buffer_views[vi].extensions.remove(n);
            }
            out.fallback_used = Some(name);
            continue;
        };
        let bad = |m: String| MeshoptDecodeError::Data(format!("bufferView {vi} ({name}): {m}"));
        let mode = MeshoptMode::from_identifier(&ext.mode)
            .ok_or_else(|| bad(format!("unknown mode {:?}", ext.mode)))?;
        let filter = match ext.filter.as_deref() {
            None => MeshoptFilter::None,
            Some(f) => MeshoptFilter::from_identifier(f)
                .ok_or_else(|| bad(format!("unknown filter {f:?}")))?,
        };
        let src = buffers
            .get(ext.buffer as usize)
            .ok_or_else(|| bad(format!("buffer {} does not exist", ext.buffer)))?;
        let start = usize::try_from(ext.byte_offset.unwrap_or(0)).unwrap_or(usize::MAX);
        let len = usize::try_from(ext.byte_length).unwrap_or(usize::MAX);
        let stream = start
            .checked_add(len)
            .and_then(|end| src.get(start..end))
            .ok_or_else(|| {
                bad(format!(
                    "the compressed stream (offset {start}, length {len}) exceeds buffer {} ({} bytes)",
                    ext.buffer,
                    src.len()
                ))
            })?;
        let bytes = meshopt
            .decode(
                stream,
                ext.count as usize,
                ext.byte_stride as usize,
                mode,
                filter,
            )
            .map_err(|e| bad(e.to_string()))?;
        // A QUATERNION view of byteStride 16 (the add-on's sampled object rotations) holds
        // its quaternions in the first `count * 8` bytes (`codec::filter_stride`): the
        // decoded view is those, tightly packed, as Blender's decoder reads them.
        let element = codec::filter_stride(filter, ext.byte_stride as usize);
        let bytes = if mode == MeshoptMode::Attributes && element != ext.byte_stride as usize {
            &bytes[..ext.count as usize * element]
        } else {
            &bytes[..]
        };
        pad4(&mut data);
        let offset = data.len() as u64;
        data.extend_from_slice(bytes);
        let bv = &mut doc.buffer_views[vi];
        bv.buffer = new_buffer;
        bv.byte_offset = Some(offset);
        bv.byte_length = bytes.len() as u64;
        if mode == MeshoptMode::Attributes && bv.byte_stride.is_none() {
            bv.byte_stride = Some(element as u32);
        }
        for n in MESHOPT_EXTENSIONS {
            bv.extensions.remove(n);
        }
        out.views += 1;
    }
    if out.views > 0 {
        doc.buffers.push(Buffer {
            byte_length: data.len() as u64,
            ..Buffer::default()
        });
        buffers.push(data);
    }
    Ok(out)
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static MESHOPT: PrimitivePostProcess = PrimitivePostProcess {
    name: "meshopt",
    order: 300,
    run,
};

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    use frap_scene::{Context, Main, Preferences, WindowState};
    use frap_spec_ops_io::export_scene::ExportSceneGltf;
    use serde_json::{Value as J, json};

    use super::*;
    use crate::export::{ExportOutput, ExportSettings, export};

    use frap_rna::IdRef;
    use frap_scene::generated::plain::View3DShading;
    use frap_scene::id::Scene;
    use frap_scene::{ChangeLog, LayerCollection, LayerCollectionPath, ViewLayer, VirtualView3D};

    use crate::import::{ImportSettings, import_file};

    /// The repository root.
    pub(crate) fn repo() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The Blender-written meshopt files (`blender_meshopt.py make`).
    pub(crate) fn fixtures() -> PathBuf {
        repo().join("tests/golden/cases/gltf/meshopt/import_blender.inputs")
    }

    /// `read_factory_settings(use_empty=True)`: one scene with one view layer.
    pub(crate) fn empty_state() -> (Main, WindowState) {
        let mut main = Main::default();
        let mut log = ChangeLog::default();
        let vl = ViewLayer {
            name: "ViewLayer".into(),
            bases: Vec::new(),
            active: None,
            layer_collection: LayerCollection::new(None),
            active_layer_collection: LayerCollectionPath::default(),
        };
        let scene = main
            .scenes
            .insert("Scene", Scene::new("", vec![vl]), &mut log);
        let win = WindowState {
            scene,
            view_layer: "ViewLayer".into(),
            view3d: VirtualView3D {
                view_matrix: frap_math::glam::Mat4::IDENTITY,
                shading: View3DShading::default(),
            },
        };
        (main, win)
    }

    fn r(v: &[f32]) -> Vec<J> {
        v.iter()
            .map(|&x| json!(((f64::from(x) * 1e6).round() / 1e6) + 0.0))
            .collect()
    }

    /// The state `blender_meshopt.py`'s `dump_state` writes, from frap's `Main`.
    pub(crate) fn dump(main: &Main) -> J {
        let mut objs = serde_json::Map::new();
        let mut obs: Vec<_> = main.objects.iter().collect();
        obs.sort_by(|a, b| a.1.header.name.cmp(&b.1.header.name));
        for (_, o) in obs {
            let mut d = serde_json::Map::new();
            d.insert("type".into(), json!(o.object_type()));
            d.insert(
                "parent".into(),
                o.parent
                    .and_then(|p| main.objects.get(p))
                    .map_or(J::Null, |p| json!(p.header.name.as_str())),
            );
            d.insert("location".into(), J::Array(r(&o.location)));
            d.insert(
                "rotation_quaternion".into(),
                J::Array(r(&o.rotation_quaternion)),
            );
            d.insert("scale".into(), J::Array(r(&o.scale)));
            if let Some(me) = o
                .data
                .and_then(|d| d.typed::<frap_rna::MeshId>())
                .and_then(|m: IdRef<frap_rna::MeshId>| main.meshes.get(m))
            {
                d.insert("mesh".into(), json!(me.header.name.as_str()));
                let co: Vec<f32> = me.positions().iter().flatten().copied().collect();
                d.insert("co".into(), J::Array(r(&co)));
                d.insert("corner_verts".into(), json!(me.corner_verts()));
                let off = me.face_offsets();
                let sizes: Vec<i32> = off.windows(2).map(|w| w[1] - w[0]).collect();
                d.insert("face_sizes".into(), json!(sizes));
                d.insert("edges".into(), json!(me.edges()));
                let uv: Vec<f32> = me
                    .uv_maps()
                    .first()
                    .map(|(_, d)| d.iter().flatten().copied().collect())
                    .unwrap_or_default();
                d.insert("uv".into(), J::Array(r(&uv)));
                if let Some(k) = me.shape_keys.and_then(|k| main.shape_keys.get(k)) {
                    d.insert(
                        "shape_keys".into(),
                        J::Object(
                            k.key_blocks
                                .iter()
                                .map(|b| {
                                    let c: Vec<f32> = b.data.iter().flatten().copied().collect();
                                    (b.name.to_string(), J::Array(r(&c)))
                                })
                                .collect(),
                        ),
                    );
                }
                if !me.vertex_group_names.is_empty() {
                    let names = &me.vertex_group_names;
                    let w: Vec<J> = (0..me.verts_num())
                        .map(|v| {
                            let mut g: Vec<(String, f64)> = me
                                .deform_verts()
                                .and_then(|d| d.get(v))
                                .map(|dv| {
                                    dv.weights
                                        .iter()
                                        .map(|&(g, w)| {
                                            (
                                                names[g as usize].to_string(),
                                                (f64::from(w) * 1e5).round() / 1e5,
                                            )
                                        })
                                        .collect()
                                })
                                .unwrap_or_default();
                            g.sort_by(|a, b| a.partial_cmp(b).unwrap());
                            json!(
                                g.into_iter()
                                    .map(|(n, w)| json!([n, w]))
                                    .collect::<Vec<_>>()
                            )
                        })
                        .collect();
                    d.insert("weights".into(), J::Array(w));
                }
            }
            objs.insert(o.header.name.to_string(), J::Object(d));
        }
        J::Object(objs)
    }

    /// `a` equals `b` with floats within `tol`; the first difference otherwise.
    pub(crate) fn diff(path: &str, a: &J, b: &J, tol: f64) -> Option<String> {
        match (a, b) {
            (J::Number(x), J::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                ((x - y).abs() > tol).then(|| format!("{path}: {x} vs {y}"))
            }
            (J::Array(x), J::Array(y)) => {
                if x.len() != y.len() {
                    return Some(format!("{path}: {} vs {} elements", x.len(), y.len()));
                }
                x.iter()
                    .zip(y)
                    .enumerate()
                    .find_map(|(i, (x, y))| diff(&format!("{path}[{i}]"), x, y, tol))
            }
            (J::Object(x), J::Object(y)) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                keys.into_iter().find_map(|k| match (x.get(k), y.get(k)) {
                    (Some(x), Some(y)) => diff(&format!("{path}.{k}"), x, y, tol),
                    _ => Some(format!("{path}.{k}: only on one side")),
                })
            }
            _ => (a != b).then(|| format!("{path}: {a} vs {b}")),
        }
    }

    /// Imports `path` into an empty state; also gives the window.
    pub(crate) fn import_with_window(path: &Path) -> (Main, WindowState) {
        let (mut main, win) = empty_state();
        let mut log = ChangeLog::default();
        import_file(&mut main, &win, path, &ImportSettings::default(), &mut log)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        (main, win)
    }

    /// Imports `path` into an empty state.
    pub(crate) fn import(path: &Path) -> (Main, Vec<frap_rna::Report>) {
        let (mut main, win) = empty_state();
        let mut log = ChangeLog::default();
        let r = import_file(&mut main, &win, path, &ImportSettings::default(), &mut log)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        (main, r.reports)
    }

    /// Settings with the props `f` sets (file stem `out`).
    pub(crate) fn settings(f: impl FnOnce(&mut ExportSceneGltf)) -> ExportSettings {
        let mut p = (*ExportSettings::default().props).clone();
        f(&mut p);
        let mut s = ExportSettings::from_props(&p);
        s.props = std::sync::Arc::new(p);
        s.generator = "frap test".into();
        s.file_stem = "out".into();
        s
    }

    pub(crate) fn run_export(main: &Main, win: &WindowState, s: &ExportSettings) -> ExportOutput {
        let prefs = Preferences::default();
        let ctx = Context {
            main,
            win,
            prefs: &prefs,
        };
        export(main, &ctx, s).unwrap()
    }

    /// A fresh scratch directory.
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("frap-wp71-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Writes an export as the operator does (main file + `files` next to it).
    pub(crate) fn write(dir: &Path, name: &str, out: &ExportOutput) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, &out.bytes).unwrap();
        for (rel, bytes) in &out.files {
            let f = dir.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, bytes).unwrap();
        }
        p
    }

    pub(crate) fn json_of(out: &ExportOutput) -> J {
        let glb = frap_gltf::read(&out.bytes).unwrap();
        serde_json::to_value(&glb.document).unwrap()
    }

    fn meshopt(ext: ExportMeshoptExtension, format: &str) -> ExportSettings {
        settings(|p| {
            p.export_meshopt_compression_enable.value = true;
            p.export_meshopt_extension.value = ext;
            p.export_format.value = format.into();
        })
    }

    #[test]
    fn factory_cube_has_blenders_layout() {
        // Blender 5.2.4, factory scene, `export_meshopt_compression_enable=True` (GLB): the
        // fallback buffer 1 (840 bytes, the uncompressed size), compressed views in buffer 0.
        let (main, win) = frap_scene::startup::factory();
        for (ext, name) in [
            (
                ExportMeshoptExtension::ExtMeshoptCompression,
                "EXT_meshopt_compression",
            ),
            (
                ExportMeshoptExtension::KhrMeshoptCompression,
                "KHR_meshopt_compression",
            ),
        ] {
            let out = run_export(&main, &win, &meshopt(ext, "GLB"));
            let j = json_of(&out);
            assert_eq!(j["extensionsUsed"], json!([name]));
            assert_eq!(j["extensionsRequired"], json!([name]));
            assert_eq!(
                j["buffers"][1],
                json!({"byteLength": 840, "extensions": {name: {"fallback": true}}})
            );
            let views = j["bufferViews"].as_array().unwrap();
            let want = [
                (288, 0, 12, 24, "ATTRIBUTES", Some("EXPONENTIAL")),
                (288, 288, 12, 24, "ATTRIBUTES", Some("EXPONENTIAL")),
                (192, 576, 8, 24, "ATTRIBUTES", None),
                (72, 768, 2, 36, "TRIANGLES", None),
            ];
            assert_eq!(views.len(), 4);
            let mut stream_end = 0;
            for (v, (len, off, stride, count, mode, filter)) in views.iter().zip(want) {
                assert_eq!(v["buffer"], 1);
                assert_eq!(v["byteLength"], len);
                assert_eq!(v["byteOffset"], off);
                let e = &v["extensions"][name];
                assert_eq!(e["buffer"], 0);
                assert_eq!(e["byteStride"], stride);
                assert_eq!(e["count"], count);
                assert_eq!(e["mode"], mode);
                assert_eq!(e.get("filter").and_then(J::as_str), filter);
                assert_eq!(e["byteOffset"].as_u64().unwrap(), stream_end);
                stream_end += e["byteLength"].as_u64().unwrap().div_ceil(4) * 4;
            }
            assert_eq!(j["buffers"][0]["byteLength"].as_u64().unwrap(), stream_end);
            let r = frap_gltf::validate::validate_file(&out.bytes).unwrap();
            assert!(r.is_ok(), "{r}");
        }
    }

    #[test]
    fn separate_files_put_the_fallback_first() {
        let (main, win) = frap_scene::startup::factory();
        let out = run_export(
            &main,
            &win,
            &meshopt(
                ExportMeshoptExtension::KhrMeshoptCompression,
                "GLTF_SEPARATE",
            ),
        );
        let j = json_of(&out);
        assert_eq!(
            j["buffers"][0],
            json!({"byteLength": 840, "extensions": {"KHR_meshopt_compression": {"fallback": true}}})
        );
        assert_eq!(j["buffers"][1]["uri"], "out.bin");
        assert_eq!(out.files[0].0, "out.bin");
        assert_eq!(
            j["buffers"][1]["byteLength"].as_u64().unwrap(),
            out.files[0].1.len() as u64
        );
        for v in j["bufferViews"].as_array().unwrap() {
            assert_eq!(v["buffer"], 0);
            assert_eq!(v["extensions"]["KHR_meshopt_compression"]["buffer"], 1);
        }
    }

    #[test]
    fn wp_7_1_props_are_supported() {
        // No "not supported yet" WARNING for them any more (deviations §2).
        for p in [
            "export_meshopt_compression_enable",
            "export_meshopt_extension",
            "frap_mesh_quantization",
            "export_use_gltfpack",
            "export_gltfpack_tc",
            "export_gltfpack_tq",
            "export_gltfpack_si",
            "export_gltfpack_sa",
            "export_gltfpack_slb",
            "export_gltfpack_vp",
            "export_gltfpack_vt",
            "export_gltfpack_vn",
            "export_gltfpack_vc",
            "export_gltfpack_vpi",
            "export_gltfpack_noq",
            "export_gltfpack_kn",
        ] {
            assert!(crate::export::is_supported(p), "{p}");
        }
    }

    #[test]
    fn without_compression_nothing_changes() {
        let (main, win) = frap_scene::startup::factory();
        let plain = run_export(&main, &win, &settings(|_| {}));
        let j = json_of(&plain);
        assert!(j.get("extensionsUsed").is_none());
        assert_eq!(j["buffers"], json!([{"byteLength": 840}]));
    }

    /// The layout of each compressed animation accessor: (mode, filter, byteStride, count,
    /// componentType, normalized), by (node name, path, input/output).
    fn anim_layout(j: &J, name: &str) -> Vec<(String, String, J)> {
        let mut out = Vec::new();
        for a in j["animations"].as_array().unwrap() {
            for c in a["channels"].as_array().unwrap() {
                let node = j["nodes"][c["target"]["node"].as_u64().unwrap() as usize]["name"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let s = &a["samplers"][c["sampler"].as_u64().unwrap() as usize];
                for k in ["input", "output"] {
                    let acc = &j["accessors"][s[k].as_u64().unwrap() as usize];
                    let v = &j["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
                    let e = &v["extensions"][name];
                    out.push((
                        node.clone(),
                        format!("{} {k}", c["target"]["path"].as_str().unwrap()),
                        json!([
                            e["filter"],
                            e["byteStride"],
                            acc["componentType"],
                            acc["normalized"]
                        ]),
                    ));
                }
            }
        }
        out
    }

    /// `export_meshopt_compression_enable` with animation (M7 stabilization): Blender's
    /// `anim_ext.glb` (sampled object TRS, bone TRS, shape-key weights) re-exported by frap
    /// compresses every sampler as the add-on's sampler modules do; `frap_compat='SPEC'`
    /// writes the object rotation at byteStride 8 and validates; the fcurves path
    /// (`export_force_sampling=False`) writes its rotations at 8 and the weights as SK_ANIM.
    #[test]
    fn animation_samplers_are_compressed() {
        let dir = fixtures().join("../import_blender_anim.inputs");
        let (main, win) = import_with_window(&dir.join("anim_ext.glb"));
        let name = "EXT_meshopt_compression";
        let ex = |f: fn(&mut ExportSceneGltf)| {
            let out = run_export(
                &main,
                &win,
                &settings(|p| {
                    p.export_meshopt_compression_enable.value = true;
                    f(p);
                }),
            );
            (json_of(&out), out)
        };
        let row = |l: &[(String, String, J)], node: &str, what: &str| -> J {
            l.iter()
                .find(|(n, w, _)| n == node && w == what)
                .unwrap_or_else(|| panic!("{node} {what}: {l:?}"))
                .2
                .clone()
        };
        let (j, _) = ex(|_| {});
        let l = anim_layout(&j, name);
        assert_eq!(
            row(&l, "Cube", "rotation input"),
            json!(["EXPONENTIAL", 4, 5126, null])
        );
        assert_eq!(
            row(&l, "Cube", "translation output"),
            json!(["EXPONENTIAL", 12, 5126, null])
        );
        assert_eq!(
            row(&l, "Cube", "scale output"),
            json!(["EXPONENTIAL", 12, 5126, null])
        );
        assert_eq!(
            row(&l, "Cube", "rotation output"),
            json!(["QUATERNION", 16, 5122, true])
        );
        assert_eq!(
            row(&l, "Cube", "weights output"),
            json!(["EXPONENTIAL", 4, 5126, null])
        );
        assert_eq!(
            row(&l, "Bone", "rotation output"),
            json!(["QUATERNION", 8, 5122, true])
        );
        // Re-imported, the animation is Blender's (lossy only within the filters' bits).
        let (j, out) = ex(|p| p.frap_compat.value = FrapCompat::Spec);
        let l = anim_layout(&j, name);
        assert_eq!(
            row(&l, "Cube", "rotation output"),
            json!(["QUATERNION", 8, 5122, true])
        );
        let r = frap_gltf::validate::validate_file(&out.bytes).unwrap();
        assert!(r.is_ok(), "{r}");
        assert!(!r.has("MESHOPT_QUATERNION_STRIDE"), "{r}");
        let (j, _) = ex(|p| p.export_force_sampling.value = false);
        let l = anim_layout(&j, name);
        assert_eq!(
            row(&l, "Cube", "rotation output"),
            json!(["QUATERNION", 8, 5122, true])
        );
        assert_eq!(
            row(&l, "Cube", "weights output"),
            json!(["EXPONENTIAL", 4, 5126, null])
        );
        // frap decodes its own stride-16 rotations as Blender's.
        let sd = scratch("anim");
        let (_, out) = ex(|_| {});
        let f = write(&sd, "anim.glb", &out);
        let (m, reports) = import(&f);
        assert!(
            !reports.iter().any(|r| r.message.contains("meshopt")),
            "{reports:?}"
        );
        assert!(m.actions.iter().count() >= 2);
    }

    /// frap's own round trip of the Blender fixture scene (skin, shape keys, loose edges,
    /// tangents): export compressed, import, compare with the import of the uncompressed
    /// export (meshopt is lossless but for the 12-bit exponential filter of POSITION, NORMAL,
    /// TANGENT and morph targets).
    #[test]
    fn compressed_round_trip_equals_the_plain_one() {
        let (main, win) = import_with_window(&fixtures().join("ext.glb"));
        let dir = scratch("roundtrip");
        let base = settings(|p| {
            p.export_tangents.value = true;
            p.use_mesh_edges.value = true;
        });
        let plain = write(&dir, "plain.glb", &run_export(&main, &win, &base));
        let (pm, _) = import(&plain);
        let want = dump(&pm);
        for (ext, format, name) in [
            (
                ExportMeshoptExtension::ExtMeshoptCompression,
                "GLB",
                "ext.glb",
            ),
            (
                ExportMeshoptExtension::KhrMeshoptCompression,
                "GLB",
                "khr.glb",
            ),
            (
                ExportMeshoptExtension::ExtMeshoptCompression,
                "GLTF_SEPARATE",
                "ext.gltf",
            ),
            (
                ExportMeshoptExtension::KhrMeshoptCompression,
                "GLTF_SEPARATE",
                "khr.gltf",
            ),
        ] {
            let s = settings(|p| {
                p.export_tangents.value = true;
                p.use_mesh_edges.value = true;
                p.export_meshopt_compression_enable.value = true;
                p.export_meshopt_extension.value = ext;
                p.export_format.value = format.into();
            });
            let out = run_export(&main, &win, &s);
            let f = write(&dir, name, &out);
            let (m, _) = import(&f);
            // Exponential filter: 12-bit mantissa per vector, the fixture's coordinates are
            // below 4, so the error is below 4 / 2^12.
            if let Some(d) = diff(name, &dump(&m), &want, 1e-3) {
                panic!("{d}");
            }
        }
    }

    /// Blender 5.2, as `frap-golden` finds it (`$BLENDER_BIN`, `blender` on `PATH`, the usual
    /// macOS install).
    pub(crate) fn blender() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("BLENDER_BIN") {
            return Some(PathBuf::from(p));
        }
        if let Some(path) = std::env::var_os("PATH") {
            for d in std::env::split_paths(&path) {
                let p = d.join("blender");
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let mac = home.join("Applications/Blender-5.2.app/Contents/MacOS/Blender");
        mac.is_file().then_some(mac)
    }

    /// Blender's import dump (`blender_meshopt.py dump`) of `files`.
    pub(crate) fn blender_dump(dir: &Path, files: &[PathBuf]) -> J {
        let exe = blender().expect("live: needs Blender 5.2 ($BLENDER_BIN)");
        let script = repo().join("tests/cli/m7/meshopt/blender_meshopt.py");
        let out = dir.join("blender.json");
        let st = std::process::Command::new(exe)
            .args([
                "-b",
                "--factory-startup",
                "--python-exit-code",
                "1",
                "--python",
            ])
            .arg(&script)
            .arg("--")
            .arg("dump")
            .arg(&out)
            .args(files)
            .output()
            .unwrap();
        assert!(
            st.status.success(),
            "Blender failed: {}",
            String::from_utf8_lossy(&st.stderr)
        );
        let j: J = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(j["blender"], "5.2.4 LTS");
        j
    }

    /// The acceptance of WP 7.1: Blender imports every frap-compressed file (both meshopt
    /// variants, GLB and `GLTF_SEPARATE`, `frap_mesh_quantization` with and without meshopt,
    /// and the `export_use_gltfpack` output with quantization and simplification) exactly as
    /// frap imports it.
    #[test]
    #[ignore = "live: needs Blender 5.2 (cargo test -p frap-io-gltf -- --ignored)"]
    fn blender_decodes_frap_compressed_files() {
        let (main, win) = import_with_window(&fixtures().join("ext.glb"));
        let dir = scratch("live");
        let mut files = Vec::new();
        let variants: [(&str, &str, Option<ExportMeshoptExtension>, bool); 6] = [
            (
                "ext.glb",
                "GLB",
                Some(ExportMeshoptExtension::ExtMeshoptCompression),
                false,
            ),
            (
                "khr.glb",
                "GLB",
                Some(ExportMeshoptExtension::KhrMeshoptCompression),
                false,
            ),
            (
                "ext_sep.gltf",
                "GLTF_SEPARATE",
                Some(ExportMeshoptExtension::ExtMeshoptCompression),
                false,
            ),
            (
                "khr_sep.gltf",
                "GLTF_SEPARATE",
                Some(ExportMeshoptExtension::KhrMeshoptCompression),
                false,
            ),
            ("quant.glb", "GLB", None, true),
            (
                "quant_khr.gltf",
                "GLTF_SEPARATE",
                Some(ExportMeshoptExtension::KhrMeshoptCompression),
                true,
            ),
        ];
        for (name, format, ext, quantize) in variants {
            let s = settings(|p| {
                p.export_tangents.value = true;
                p.use_mesh_edges.value = true;
                p.export_format.value = format.into();
                if let Some(e) = ext {
                    p.export_meshopt_compression_enable.value = true;
                    p.export_meshopt_extension.value = e;
                }
                p.frap_mesh_quantization.value = quantize;
            });
            let mut s = s;
            s.file_stem = name.split('.').next().unwrap().into();
            files.push(write(&dir, name, &run_export(&main, &win, &s)));
        }
        let s = settings(|p| {
            p.export_use_gltfpack.value = true;
            p.export_gltfpack_noq.value = false;
            p.export_gltfpack_si.value = 0.5;
            p.export_gltfpack_sa.value = true;
            p.export_gltfpack_tc.value = false;
        });
        let mut s = s;
        s.file_stem = "pack".into();
        let out = run_export(&main, &win, &s);
        write(&dir, "pack.glb", &out);
        files.push(dir.join("gltfpacked/pack.glb"));
        let b = blender_dump(&dir, &files);
        for f in &files {
            let name = f.file_name().unwrap().to_str().unwrap();
            assert_eq!(b[name]["result"], json!(["FINISHED"]), "{name}");
            let (m, _) = import(f);
            if let Some(d) = diff(name, &dump(&m), &b[name]["objects"], 2e-5) {
                panic!("Blender's import of frap's {name} differs from frap's: {d}");
            }
        }
    }
}
