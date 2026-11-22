// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshoptCodec` (ARCHITECTURE §17.2): the buffer-view codec of `EXT_meshopt_compression` and
//! `KHR_meshopt_compression` (vertex codec version 0 / 1), with the attribute filters.
//!
//! CONTRACT (M7 W0, decision log M7W0-5): the trait, [`MeshoptMode`], [`MeshoptFilter`],
//! [`MeshoptVersion`] and [`codec`]. WP 7.1 owns this file: the meshoptimizer backend (feature
//! `meshopt`), filter encoding helpers, and any additive items. The add-on's encoder is
//! `io/exp/meshopt.py` over Blender's `bf_intern_meshopt_bridge` (meshoptimizer), so a
//! meshoptimizer backend gives the add-on's bitstreams (Blender 5.2.4 bundles meshoptimizer
//! 1.1, the `meshopt =0.7.0` crate 1.3: same formats, encoder output may differ by version;
//! the one filter whose output differs, `meshopt_encodeFilterExp`, is ported from 1.1, see
//! [`encode_filter_exp`]).
//!
//! WP 7.1 additions (all behind the `meshopt` feature but [`encode_filter_exp`]; without it
//! they answer [`CodecError::Unavailable`]):
//! - the attribute filter encoders the add-on calls ([`encode_filter_exp`] with
//!   [`ExpMode`], [`encode_filter_quat`], [`encode_filter_oct`], [`encode_filter_color`]);
//! - the mesh optimisers `export_use_gltfpack` maps onto (gltfpack is a meshoptimizer tool):
//!   [`simplify`], [`simplify_sloppy`], [`simplify_points`], [`optimize_vertex_cache`],
//!   [`optimize_vertex_fetch_remap`];
//! - [`quantize_unorm`] / [`quantize_snorm`] (meshoptimizer's inline header functions, pure
//!   Rust, always available).
//!
//! Every FFI call validates its parameters first: meshoptimizer `assert`s on invalid sizes,
//! which would abort the process in a debug build of the C++ code.

use crate::error::CodecError;

/// `mode` of a compressed buffer view.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MeshoptMode {
    /// `ATTRIBUTES`: vertex data (`byteStride` bytes per element).
    Attributes,
    /// `TRIANGLES`: triangle-list indices.
    Triangles,
    /// `INDICES`: any other index sequence.
    Indices,
}

impl MeshoptMode {
    /// The glTF identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            MeshoptMode::Attributes => "ATTRIBUTES",
            MeshoptMode::Triangles => "TRIANGLES",
            MeshoptMode::Indices => "INDICES",
        }
    }

    /// Parses a glTF identifier (WP 7.1).
    pub fn from_identifier(s: &str) -> Option<Self> {
        Some(match s {
            "ATTRIBUTES" => MeshoptMode::Attributes,
            "TRIANGLES" => MeshoptMode::Triangles,
            "INDICES" => MeshoptMode::Indices,
            _ => return None,
        })
    }
}

/// `filter` of a compressed buffer view (applied after decoding).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MeshoptFilter {
    /// `NONE`
    #[default]
    None,
    /// `OCTAHEDRAL` (normals, tangents)
    Octahedral,
    /// `QUATERNION` (rotations)
    Quaternion,
    /// `EXPONENTIAL` (floats with a shared exponent)
    Exponential,
    /// `COLOR` (`KHR_meshopt_compression` only)
    Color,
}

impl MeshoptFilter {
    /// The glTF identifier.
    pub fn identifier(self) -> &'static str {
        match self {
            MeshoptFilter::None => "NONE",
            MeshoptFilter::Octahedral => "OCTAHEDRAL",
            MeshoptFilter::Quaternion => "QUATERNION",
            MeshoptFilter::Exponential => "EXPONENTIAL",
            MeshoptFilter::Color => "COLOR",
        }
    }

    /// Parses a glTF identifier (WP 7.1).
    pub fn from_identifier(s: &str) -> Option<Self> {
        Some(match s {
            "NONE" => MeshoptFilter::None,
            "OCTAHEDRAL" => MeshoptFilter::Octahedral,
            "QUATERNION" => MeshoptFilter::Quaternion,
            "EXPONENTIAL" => MeshoptFilter::Exponential,
            "COLOR" => MeshoptFilter::Color,
            _ => return None,
        })
    }
}

/// Vertex-codec bitstream version: 0 for `EXT_meshopt_compression`, 1 for
/// `KHR_meshopt_compression` (the add-on's `encodeVertexVersion`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum MeshoptVersion {
    /// `EXT_meshopt_compression`
    #[default]
    Ext,
    /// `KHR_meshopt_compression`
    Khr,
}

impl MeshoptVersion {
    /// The vertex codec version number (`meshopt_encodeVertexVersion`).
    pub fn vertex_version(self) -> i32 {
        match self {
            MeshoptVersion::Ext => 0,
            MeshoptVersion::Khr => 1,
        }
    }
}

/// The meshopt buffer-view codec.
pub trait MeshoptCodec: Send + Sync {
    /// Encodes `count` vertices of `stride` bytes each (`data.len() == count * stride`).
    fn encode_vertex(
        &self,
        data: &[u8],
        count: usize,
        stride: usize,
        version: MeshoptVersion,
    ) -> Result<Vec<u8>, CodecError>;

    /// Encodes indices of `index_size` (2 or 4) bytes; `mode` is `Triangles` or `Indices`.
    fn encode_index(
        &self,
        indices: &[u32],
        vertex_count: usize,
        index_size: usize,
        mode: MeshoptMode,
    ) -> Result<Vec<u8>, CodecError>;

    /// Decodes a compressed view into `count * stride` bytes and applies `filter`.
    fn decode(
        &self,
        data: &[u8],
        count: usize,
        stride: usize,
        mode: MeshoptMode,
        filter: MeshoptFilter,
    ) -> Result<Vec<u8>, CodecError>;
}

/// The codec of this build, `None` without a meshopt backend (feature `meshopt`).
pub fn codec() -> Option<&'static dyn MeshoptCodec> {
    #[cfg(feature = "meshopt")]
    {
        Some(&backend::Meshoptimizer)
    }
    #[cfg(not(feature = "meshopt"))]
    {
        None
    }
}

/// Largest decoded view frap allocates (1 GiB): a corrupt `count` must not exhaust memory.
pub const MAX_DECODED_BYTES: usize = 1 << 30;

/// Exponent sharing of [`encode_filter_exp`] (`meshopt_EncodeExpMode`; the add-on's
/// `ENCODE_EXP_*` constants).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ExpMode {
    /// One exponent per component (`meshopt_EncodeExpSeparate`).
    Separate,
    /// One exponent per vector (`meshopt_EncodeExpSharedVector`): the add-on's choice.
    #[default]
    SharedVector,
    /// One exponent per component across all vectors (`meshopt_EncodeExpSharedComponent`).
    SharedComponent,
    /// Separate exponents clamped to 0 (`meshopt_EncodeExpClamped`).
    Clamped,
}

/// meshoptimizer's `meshopt_quantizeUnorm`: `v` clamped to [0, 1] as an `n`-bit unorm.
pub fn quantize_unorm(v: f32, n: u32) -> i32 {
    let scale = ((1u32 << n) - 1) as f32;
    let v = if v >= 0.0 { v } else { 0.0 };
    let v = if v <= 1.0 { v } else { 1.0 };
    (v * scale + 0.5) as i32
}

/// meshoptimizer's `meshopt_quantizeSnorm`: `v` clamped to [-1, 1] as an `n`-bit snorm.
pub fn quantize_snorm(v: f32, n: u32) -> i32 {
    let scale = ((1u32 << (n - 1)) - 1) as f32;
    let round = if v >= 0.0 { 0.5 } else { -0.5 };
    let v = if v >= -1.0 { v } else { -1.0 };
    let v = if v <= 1.0 { v } else { 1.0 };
    (v * scale + round) as i32
}

fn encode_err(m: impl Into<String>) -> CodecError {
    CodecError::Encode(m.into())
}

/// Checks the shared layout rule of the vertex codec and the filters: `stride` a non-zero
/// multiple of 4, at most 256.
fn check_vertex_stride(stride: usize) -> Result<(), String> {
    if stride == 0 || !stride.is_multiple_of(4) || stride > 256 {
        Err(format!(
            "meshopt: byteStride {stride} is not a multiple of 4 between 4 and 256"
        ))
    } else {
        Ok(())
    }
}

/// The element size a filter of a `stride`-byte view applies to: `stride`, except for the
/// add-on's `QUATERNION` views of byteStride 16 (`sampled/object/sampler.py`, the object
/// rotations of `export_force_sampling`), whose quaternions are 8-byte elements packed in the
/// first `count * 8` bytes: release meshoptimizer's `meshopt_decodeFilterQuat` (Blender's
/// bridge) ignores the stride and reads them there, and Blender 5.2.4 imports these files.
pub fn filter_stride(filter: MeshoptFilter, stride: usize) -> usize {
    if filter == MeshoptFilter::Quaternion && stride == 16 {
        8
    } else {
        stride
    }
}

/// Checks that a filter can be applied to elements of `stride` bytes (the spec's rules).
pub fn check_filter_stride(filter: MeshoptFilter, stride: usize) -> Result<(), String> {
    let ok = match filter {
        MeshoptFilter::None => true,
        MeshoptFilter::Octahedral | MeshoptFilter::Color => stride == 4 || stride == 8,
        MeshoptFilter::Quaternion => stride == 8,
        MeshoptFilter::Exponential => stride > 0 && stride.is_multiple_of(4) && stride <= 256,
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "meshopt: filter {} cannot apply to a byteStride of {stride}",
            filter.identifier()
        ))
    }
}

/// `meshopt_encodeFilterExp` of meshoptimizer 1.1, the version Blender 5.2.4 bundles:
/// `values` holds `count` elements of `stride / 4` floats; returns `count * stride` bytes
/// (one `(exponent << 24) | mantissa` word per float, `bits` of mantissa, 1..=24). Pure Rust,
/// always available.
///
/// Ported from meshoptimizer v1.1 `src/vertexfilter.cpp` (MIT, Arseny Kapoulkine): 1.3 (the
/// `meshopt` crate's) changed `optlog2(0)` from 0 to the minimum exponent, so a zero component
/// no longer raises a shared exponent, and clamps the rounded mantissa; with 1.1 the vector
/// `(0, 0.0171, 0)` gets the exponent of 0.5 (Blender 5.2.4's bridge, probed through ctypes).
/// Non-finite values encode as 1.1 does on arm64 and x86-64 SSE (`int(NaN)` is 0 in Rust; an
/// infinite product saturates).
pub fn encode_filter_exp(
    values: &[f32],
    count: usize,
    stride: usize,
    bits: u32,
    mode: ExpMode,
) -> Result<Vec<u8>, CodecError> {
    check_vertex_stride(stride).map_err(encode_err)?;
    if !(1..=24).contains(&bits) {
        return Err(encode_err(format!(
            "meshopt: {bits} exponential filter bits"
        )));
    }
    if values.len() != count * (stride / 4) {
        return Err(encode_err(format!(
            "meshopt: {} floats for {count} elements of {stride} bytes",
            values.len()
        )));
    }
    Ok(encode_filter_exp_v1_1(
        values,
        count,
        stride / 4,
        bits as i32,
        mode,
    ))
}

/// meshoptimizer 1.1's `optlog2`: `frexp`'s exponent, 0 for 0.
fn optlog2_v1_1(v: f32) -> i32 {
    if v == 0.0 {
        0
    } else {
        ((v.to_bits() >> 23) & 0xff) as i32 - 127 + 1
    }
}

/// meshoptimizer's `optexp2`: `2^e` as a float built from its exponent bits.
fn optexp2(e: i32) -> f32 {
    f32::from_bits(((e + 127) as u32) << 23)
}

/// The body of meshoptimizer 1.1's `meshopt_encodeFilterExp`.
fn encode_filter_exp_v1_1(
    data: &[f32],
    count: usize,
    stride_float: usize,
    bits: i32,
    mode: ExpMode,
) -> Vec<u8> {
    const MIN_EXP: i32 = -100;
    let mut component_exp = vec![MIN_EXP; stride_float];
    if mode == ExpMode::SharedComponent {
        for v in data.chunks_exact(stride_float).take(count) {
            for (c, &x) in component_exp.iter_mut().zip(v) {
                *c = (*c).max(optlog2_v1_1(x));
            }
        }
    }
    let mut out = Vec::with_capacity(count * stride_float * 4);
    for v in data.chunks_exact(stride_float).take(count) {
        let mut vector_exp = MIN_EXP;
        match mode {
            ExpMode::SharedVector => {
                for &x in v {
                    vector_exp = vector_exp.max(optlog2_v1_1(x));
                }
            }
            ExpMode::Separate => {
                for (c, &x) in component_exp.iter_mut().zip(v) {
                    *c = optlog2_v1_1(x).max(MIN_EXP);
                }
            }
            ExpMode::Clamped => {
                for (c, &x) in component_exp.iter_mut().zip(v) {
                    *c = optlog2_v1_1(x).max(0);
                }
            }
            ExpMode::SharedComponent => {}
        }
        for (j, &x) in v.iter().enumerate() {
            let exp = if mode == ExpMode::SharedVector {
                vector_exp
            } else {
                component_exp[j]
            } - (bits - 1);
            // `int(v[j] * optexp2(-exp) + (v[j] >= 0 ? 0.5f : -0.5f))`, in f32.
            let m = (x * optexp2(-exp) + if x >= 0.0 { 0.5 } else { -0.5 }) as i32;
            let d = ((m as u32) & ((1 << 24) - 1)) | ((exp as u32) << 24);
            out.extend_from_slice(&d.to_le_bytes());
        }
    }
    out
}

/// `meshopt_encodeFilterQuat`: `values` holds `count` unit quaternions (x, y, z, w); returns
/// `count * 8` bytes (`bits` 4..=16).
pub fn encode_filter_quat(values: &[f32], count: usize, bits: u32) -> Result<Vec<u8>, CodecError> {
    if !(4..=16).contains(&bits) {
        return Err(encode_err(format!(
            "meshopt: {bits} quaternion filter bits"
        )));
    }
    check_four(values, count)?;
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            backend::encode_filter_four(backend::Four::Quat, values, count, 8, bits)
        }
        #[cfg(not(feature = "meshopt"))]
        {
            unreachable!()
        }
    })
}

/// `meshopt_encodeFilterOct`: `values` holds `count` (x, y, z, w) unit vectors (w kept);
/// returns `count * stride` bytes (`stride` 4 with `bits` ≤ 8, or 8; `bits` 2..=16).
pub fn encode_filter_oct(
    values: &[f32],
    count: usize,
    stride: usize,
    bits: u32,
) -> Result<Vec<u8>, CodecError> {
    check_four_stride(stride, bits)?;
    check_four(values, count)?;
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            backend::encode_filter_four(backend::Four::Oct, values, count, stride, bits)
        }
        #[cfg(not(feature = "meshopt"))]
        {
            unreachable!()
        }
    })
}

/// `meshopt_encodeFilterColor` (`KHR_meshopt_compression`'s `COLOR`): `values` holds `count`
/// RGBA colours; returns `count * stride` bytes (`stride` 4 with `bits` ≤ 8, or 8; `bits`
/// 2..=16).
pub fn encode_filter_color(
    values: &[f32],
    count: usize,
    stride: usize,
    bits: u32,
) -> Result<Vec<u8>, CodecError> {
    check_four_stride(stride, bits)?;
    check_four(values, count)?;
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            backend::encode_filter_four(backend::Four::Color, values, count, stride, bits)
        }
        #[cfg(not(feature = "meshopt"))]
        {
            unreachable!()
        }
    })
}

fn check_four(values: &[f32], count: usize) -> Result<(), CodecError> {
    if values.len() != count * 4 {
        return Err(encode_err(format!(
            "meshopt: {} floats for {count} four-component elements",
            values.len()
        )));
    }
    Ok(())
}

fn check_four_stride(stride: usize, bits: u32) -> Result<(), CodecError> {
    if !(2..=16).contains(&bits) || !(stride == 8 || (stride == 4 && bits <= 8)) {
        return Err(encode_err(format!(
            "meshopt: {bits} filter bits do not fit a byteStride of {stride}"
        )));
    }
    Ok(())
}

fn backend_or_unavailable<T>(f: impl FnOnce() -> Result<T, CodecError>) -> Result<T, CodecError> {
    if cfg!(feature = "meshopt") {
        f()
    } else {
        Err(unavailable())
    }
}

/// The error of a build without the `meshopt` feature.
pub fn unavailable() -> CodecError {
    CodecError::Unavailable {
        codec: "meshopt",
        feature: "meshopt",
    }
}

/// Options of [`simplify`] (`meshopt_SimplifyX` flags).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct SimplifyOptions {
    /// `meshopt_SimplifyLockBorder`: border vertices stay (gltfpack `-slb`).
    pub lock_border: bool,
    /// `meshopt_SimplifyPrune`: small isolated parts may be removed (gltfpack's default
    /// without `-slb`).
    pub prune: bool,
    /// `meshopt_SimplifyRegularize` (gltfpack for meshes with morph targets or skins).
    pub regularize: bool,
}

impl SimplifyOptions {
    #[cfg(feature = "meshopt")]
    fn bits(self) -> u32 {
        u32::from(self.lock_border)
            | (u32::from(self.prune) << 3)
            | (u32::from(self.regularize) << 4)
    }
}

/// Checks a triangle list against its vertex count.
fn check_triangles(indices: &[u32], vertex_count: usize) -> Result<(), CodecError> {
    if !indices.len().is_multiple_of(3) {
        return Err(encode_err(format!(
            "meshopt: {} indices are not a triangle list",
            indices.len()
        )));
    }
    if let Some(i) = indices.iter().find(|&&i| i as usize >= vertex_count) {
        return Err(encode_err(format!(
            "meshopt: index {i} out of range ({vertex_count} vertices)"
        )));
    }
    Ok(())
}

/// `meshopt_simplifyWithAttributes` (or `meshopt_simplify` without attributes): a triangle
/// list of at most `target_index_count` indices into the same vertices. `positions` holds three
/// floats per vertex; `attributes` `weights.len()` floats per vertex (at most 32).
pub fn simplify(
    indices: &[u32],
    positions: &[f32],
    attributes: &[f32],
    weights: &[f32],
    target_index_count: usize,
    target_error: f32,
    options: SimplifyOptions,
) -> Result<Vec<u32>, CodecError> {
    let vertex_count = positions.len() / 3;
    if positions.len() != vertex_count * 3 {
        return Err(encode_err("meshopt: positions are not float triples"));
    }
    check_triangles(indices, vertex_count)?;
    if weights.len() > 32 || attributes.len() != vertex_count * weights.len() {
        return Err(encode_err(format!(
            "meshopt: {} attribute floats for {vertex_count} vertices of {} weights",
            attributes.len(),
            weights.len()
        )));
    }
    let _ = (target_index_count, target_error);
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            Ok(backend::simplify(
                indices,
                positions,
                attributes,
                weights,
                target_index_count,
                target_error,
                options.bits(),
            ))
        }
        #[cfg(not(feature = "meshopt"))]
        {
            let _ = options;
            unreachable!()
        }
    })
}

/// `meshopt_simplifySloppy` (gltfpack `-sa` when the topology-preserving pass stops short).
pub fn simplify_sloppy(
    indices: &[u32],
    positions: &[f32],
    target_index_count: usize,
    target_error: f32,
) -> Result<Vec<u32>, CodecError> {
    let vertex_count = positions.len() / 3;
    if positions.len() != vertex_count * 3 {
        return Err(encode_err("meshopt: positions are not float triples"));
    }
    check_triangles(indices, vertex_count)?;
    let _ = (target_index_count, target_error);
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            Ok(backend::simplify_sloppy(
                indices,
                positions,
                target_index_count,
                target_error,
            ))
        }
        #[cfg(not(feature = "meshopt"))]
        {
            unreachable!()
        }
    })
}

/// `meshopt_simplifyPoints`: the indices of at most `target_vertex_count` points to keep.
/// `colors`, when given, holds three floats per point (weight 1, gltfpack's choice).
pub fn simplify_points(
    positions: &[f32],
    colors: Option<&[f32]>,
    target_vertex_count: usize,
) -> Result<Vec<u32>, CodecError> {
    let vertex_count = positions.len() / 3;
    if positions.len() != vertex_count * 3 || colors.is_some_and(|c| c.len() != positions.len()) {
        return Err(encode_err("meshopt: point data are not float triples"));
    }
    let target = target_vertex_count.min(vertex_count);
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            Ok(backend::simplify_points(positions, colors, target))
        }
        #[cfg(not(feature = "meshopt"))]
        {
            let _ = target;
            unreachable!()
        }
    })
}

/// `meshopt_optimizeVertexCache`: the triangle list reordered for the post-transform cache.
pub fn optimize_vertex_cache(indices: &[u32], vertex_count: usize) -> Result<Vec<u32>, CodecError> {
    check_triangles(indices, vertex_count)?;
    backend_or_unavailable(|| {
        #[cfg(feature = "meshopt")]
        {
            Ok(backend::optimize_vertex_cache(indices, vertex_count))
        }
        #[cfg(not(feature = "meshopt"))]
        {
            unreachable!()
        }
    })
}

/// `meshopt_optimizeVertexFetchRemap`: per old vertex its new index in first-use order
/// (`u32::MAX` for unused vertices), and the number of used vertices.
pub fn optimize_vertex_fetch_remap(
    indices: &[u32],
    vertex_count: usize,
) -> Result<(Vec<u32>, usize), CodecError> {
    if let Some(i) = indices.iter().find(|&&i| i as usize >= vertex_count) {
        return Err(encode_err(format!(
            "meshopt: index {i} out of range ({vertex_count} vertices)"
        )));
    }
    // A pure port (the C function only assigns first-use order): `meshopt_optimizeVertexFetchRemap`,
    // vfetchoptimizer.cpp, MIT, Arseny Kapoulkine.
    let mut remap = vec![u32::MAX; vertex_count];
    let mut next = 0u32;
    for &i in indices {
        let r = &mut remap[i as usize];
        if *r == u32::MAX {
            *r = next;
            next += 1;
        }
    }
    Ok((remap, next as usize))
}

#[cfg(feature = "meshopt")]
mod backend {
    //! The meshoptimizer backend through the `meshopt =0.7.0` crate's raw FFI (its safe
    //! wrappers set process-global encoder versions or take typed slices; frap passes the
    //! version per call and works on byte slices).
    #![allow(unsafe_code)] // FFI wrappers (ARCHITECTURE §21.6: frap-codec-mesh may)

    use meshopt::ffi;

    use super::{
        CodecError, MAX_DECODED_BYTES, MeshoptCodec, MeshoptFilter, MeshoptMode, MeshoptVersion,
        check_filter_stride, check_vertex_stride, encode_err, filter_stride,
    };

    /// meshoptimizer's default encoder level (`meshopt::kEncodeDefaultLevel`), the level of
    /// the add-on's `meshopt_encodeVertexBuffer`.
    const LEVEL: i32 = 2;

    pub(super) struct Meshoptimizer;

    fn decode_err(m: impl Into<String>) -> CodecError {
        CodecError::Decode(m.into())
    }

    impl MeshoptCodec for Meshoptimizer {
        fn encode_vertex(
            &self,
            data: &[u8],
            count: usize,
            stride: usize,
            version: MeshoptVersion,
        ) -> Result<Vec<u8>, CodecError> {
            check_vertex_stride(stride).map_err(encode_err)?;
            if count.checked_mul(stride) != Some(data.len()) {
                return Err(encode_err(format!(
                    "meshopt: {} bytes for {count} vertices of {stride} bytes",
                    data.len()
                )));
            }
            // SAFETY: pure function of its arguments.
            let bound = unsafe { ffi::meshopt_encodeVertexBufferBound(count, stride) };
            let mut out = vec![0u8; bound];
            // SAFETY: `out` holds `bound` bytes, `data` `count * stride` bytes; the stride, the
            // level and the version (0 or 1) satisfy the function's asserts (checked above).
            let n = unsafe {
                ffi::meshopt_encodeVertexBufferLevel(
                    out.as_mut_ptr(),
                    out.len(),
                    data.as_ptr().cast(),
                    count,
                    stride,
                    LEVEL,
                    version.vertex_version(),
                )
            };
            if n == 0 {
                return Err(encode_err("meshopt: vertex encoding failed"));
            }
            out.truncate(n);
            Ok(out)
        }

        fn encode_index(
            &self,
            indices: &[u32],
            vertex_count: usize,
            index_size: usize,
            mode: MeshoptMode,
        ) -> Result<Vec<u8>, CodecError> {
            if index_size != 2 && index_size != 4 {
                return Err(encode_err(format!("meshopt: index size {index_size}")));
            }
            let (bound, n);
            let mut out;
            match mode {
                MeshoptMode::Triangles => {
                    if !indices.len().is_multiple_of(3) {
                        return Err(encode_err(format!(
                            "meshopt: {} indices are not a triangle list",
                            indices.len()
                        )));
                    }
                    // SAFETY: pure function of its arguments.
                    bound =
                        unsafe { ffi::meshopt_encodeIndexBufferBound(indices.len(), vertex_count) };
                    out = vec![0u8; bound];
                    // SAFETY: `out` holds `bound` bytes; the count is a multiple of 3.
                    n = unsafe {
                        ffi::meshopt_encodeIndexBuffer(
                            out.as_mut_ptr(),
                            out.len(),
                            indices.as_ptr(),
                            indices.len(),
                        )
                    };
                }
                MeshoptMode::Indices => {
                    // SAFETY: pure function of its arguments.
                    bound = unsafe {
                        ffi::meshopt_encodeIndexSequenceBound(indices.len(), vertex_count)
                    };
                    out = vec![0u8; bound];
                    // SAFETY: `out` holds `bound` bytes.
                    n = unsafe {
                        ffi::meshopt_encodeIndexSequence(
                            out.as_mut_ptr(),
                            out.len(),
                            indices.as_ptr(),
                            indices.len(),
                        )
                    };
                }
                MeshoptMode::Attributes => {
                    return Err(encode_err("meshopt: indices cannot use mode ATTRIBUTES"));
                }
            }
            if n == 0 {
                return Err(encode_err(
                    "meshopt: index encoding failed (an index exceeds the vertex count?)",
                ));
            }
            out.truncate(n);
            Ok(out)
        }

        fn decode(
            &self,
            data: &[u8],
            count: usize,
            stride: usize,
            mode: MeshoptMode,
            filter: MeshoptFilter,
        ) -> Result<Vec<u8>, CodecError> {
            let len = count
                .checked_mul(stride)
                .filter(|&n| n <= MAX_DECODED_BYTES)
                .ok_or_else(|| {
                    decode_err(format!(
                        "meshopt: {count} elements of {stride} bytes exceed frap's limit of {MAX_DECODED_BYTES} decoded bytes per view"
                    ))
                })?;
            let mut out = vec![0u8; len];
            let rc = match mode {
                MeshoptMode::Attributes => {
                    check_vertex_stride(stride).map_err(decode_err)?;
                    check_filter_stride(filter, filter_stride(filter, stride))
                        .map_err(decode_err)?;
                    // SAFETY: `out` holds `count * stride` bytes; the stride satisfies the
                    // asserts; the decoder bounds-checks the (untrusted) input.
                    unsafe {
                        ffi::meshopt_decodeVertexBuffer(
                            out.as_mut_ptr().cast(),
                            count,
                            stride,
                            data.as_ptr(),
                            data.len(),
                        )
                    }
                }
                MeshoptMode::Triangles | MeshoptMode::Indices => {
                    if stride != 2 && stride != 4 {
                        return Err(decode_err(format!(
                            "meshopt: {} needs a byteStride of 2 or 4, not {stride}",
                            mode.identifier()
                        )));
                    }
                    if filter != MeshoptFilter::None {
                        return Err(decode_err(format!(
                            "meshopt: filter {} on {} data",
                            filter.identifier(),
                            mode.identifier()
                        )));
                    }
                    if mode == MeshoptMode::Triangles {
                        if !count.is_multiple_of(3) {
                            return Err(decode_err(format!(
                                "meshopt: TRIANGLES count {count} is not a multiple of 3"
                            )));
                        }
                        // SAFETY: `out` holds `count * stride` bytes, count % 3 == 0, stride
                        // 2 or 4; the decoder bounds-checks the input.
                        unsafe {
                            ffi::meshopt_decodeIndexBuffer(
                                out.as_mut_ptr().cast(),
                                count,
                                stride,
                                data.as_ptr(),
                                data.len(),
                            )
                        }
                    } else {
                        // SAFETY: as above, without the triangle constraint.
                        unsafe {
                            ffi::meshopt_decodeIndexSequence(
                                out.as_mut_ptr().cast(),
                                count,
                                stride,
                                data.as_ptr(),
                                data.len(),
                            )
                        }
                    }
                }
            };
            if rc != 0 {
                return Err(decode_err(format!(
                    "meshopt: decoding a {} view failed (error {rc})",
                    mode.identifier()
                )));
            }
            if count > 0 {
                // A QUATERNION view of byteStride 16 filters its first `count * 8` bytes
                // (`filter_stride`), as Blender's decoder does.
                let stride = filter_stride(filter, stride);
                // SAFETY: `out` holds `count` elements of `stride` bytes (at least: the
                // quaternions of a 16-byte view fill half of it), a stride the filter accepts
                // (checked above).
                unsafe {
                    let p = out.as_mut_ptr().cast();
                    match filter {
                        MeshoptFilter::None => {}
                        MeshoptFilter::Octahedral => ffi::meshopt_decodeFilterOct(p, count, stride),
                        MeshoptFilter::Quaternion => {
                            ffi::meshopt_decodeFilterQuat(p, count, stride)
                        }
                        MeshoptFilter::Exponential => {
                            ffi::meshopt_decodeFilterExp(p, count, stride)
                        }
                        MeshoptFilter::Color => ffi::meshopt_decodeFilterColor(p, count, stride),
                    }
                }
            }
            Ok(out)
        }
    }

    /// The filters whose input is four floats per element.
    pub(super) enum Four {
        Quat,
        Oct,
        Color,
    }

    pub(super) fn encode_filter_four(
        which: Four,
        values: &[f32],
        count: usize,
        stride: usize,
        bits: u32,
    ) -> Result<Vec<u8>, CodecError> {
        let mut out = vec![0u8; count * stride];
        if count > 0 {
            let (d, v) = (out.as_mut_ptr().cast(), values.as_ptr());
            // SAFETY: `out` holds `count * stride` bytes and `values` `count * 4` floats;
            // stride and bits satisfy each function's asserts (checked by the callers).
            unsafe {
                match which {
                    Four::Quat => ffi::meshopt_encodeFilterQuat(d, count, stride, bits as i32, v),
                    Four::Oct => ffi::meshopt_encodeFilterOct(d, count, stride, bits as i32, v),
                    Four::Color => ffi::meshopt_encodeFilterColor(d, count, stride, bits as i32, v),
                }
            }
        }
        Ok(out)
    }

    pub(super) fn simplify(
        indices: &[u32],
        positions: &[f32],
        attributes: &[f32],
        weights: &[f32],
        target_index_count: usize,
        target_error: f32,
        options: u32,
    ) -> Vec<u32> {
        let vertex_count = positions.len() / 3;
        let mut out = vec![0u32; indices.len()];
        if indices.is_empty() {
            return out;
        }
        // SAFETY: `out` holds `indices.len()` entries (the worst case); every index is below
        // `vertex_count` (checked by the caller); positions hold 3 floats and attributes
        // `weights.len()` (≤ 32) floats per vertex.
        let n = unsafe {
            if weights.is_empty() {
                ffi::meshopt_simplify(
                    out.as_mut_ptr(),
                    indices.as_ptr(),
                    indices.len(),
                    positions.as_ptr(),
                    vertex_count,
                    12,
                    target_index_count,
                    target_error,
                    options,
                    std::ptr::null_mut(),
                )
            } else {
                ffi::meshopt_simplifyWithAttributes(
                    out.as_mut_ptr(),
                    indices.as_ptr(),
                    indices.len(),
                    positions.as_ptr(),
                    vertex_count,
                    12,
                    attributes.as_ptr(),
                    weights.len() * 4,
                    weights.as_ptr(),
                    weights.len(),
                    std::ptr::null(),
                    target_index_count,
                    target_error,
                    options,
                    std::ptr::null_mut(),
                )
            }
        };
        out.truncate(n);
        out
    }

    pub(super) fn simplify_sloppy(
        indices: &[u32],
        positions: &[f32],
        target_index_count: usize,
        target_error: f32,
    ) -> Vec<u32> {
        let mut out = vec![0u32; indices.len()];
        if indices.is_empty() {
            return out;
        }
        // SAFETY: as `simplify`.
        let n = unsafe {
            ffi::meshopt_simplifySloppy(
                out.as_mut_ptr(),
                indices.as_ptr(),
                indices.len(),
                positions.as_ptr(),
                positions.len() / 3,
                12,
                std::ptr::null(),
                target_index_count,
                target_error,
                std::ptr::null_mut(),
            )
        };
        out.truncate(n);
        out
    }

    pub(super) fn simplify_points(
        positions: &[f32],
        colors: Option<&[f32]>,
        target: usize,
    ) -> Vec<u32> {
        let mut out = vec![0u32; target];
        if target == 0 {
            return out;
        }
        // SAFETY: `out` holds `target` (≤ vertex count) entries; positions and colours hold
        // 3 floats per point.
        let n = unsafe {
            ffi::meshopt_simplifyPoints(
                out.as_mut_ptr(),
                positions.as_ptr(),
                positions.len() / 3,
                12,
                colors.map_or(std::ptr::null(), <[f32]>::as_ptr),
                12,
                1.0,
                target,
            )
        };
        out.truncate(n);
        out
    }

    pub(super) fn optimize_vertex_cache(indices: &[u32], vertex_count: usize) -> Vec<u32> {
        let mut out = vec![0u32; indices.len()];
        if indices.is_empty() {
            return out;
        }
        // SAFETY: `out` holds `indices.len()` entries; a triangle list with every index below
        // `vertex_count` (checked by the caller).
        unsafe {
            ffi::meshopt_optimizeVertexCache(
                out.as_mut_ptr(),
                indices.as_ptr(),
                indices.len(),
                vertex_count,
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip() {
        for m in [
            MeshoptMode::Attributes,
            MeshoptMode::Triangles,
            MeshoptMode::Indices,
        ] {
            assert_eq!(MeshoptMode::from_identifier(m.identifier()), Some(m));
        }
        for f in [
            MeshoptFilter::None,
            MeshoptFilter::Octahedral,
            MeshoptFilter::Quaternion,
            MeshoptFilter::Exponential,
            MeshoptFilter::Color,
        ] {
            assert_eq!(MeshoptFilter::from_identifier(f.identifier()), Some(f));
        }
        assert_eq!(MeshoptFilter::from_identifier("FOO"), None);
        assert_eq!(MeshoptVersion::Khr.vertex_version(), 1);
    }

    /// Blender 5.2.4's `bf_intern_meshopt_bridge` (meshoptimizer 1.1) `encodeFilterExp` of
    /// `[(0, 0, 0), (0, 0.0171467, 0), (0, 0.5, 0)]` (stride 12, 12 bits), probed with ctypes
    /// inside Blender: modes 0 (separate), 1 (shared vector, the add-on's) and 2.
    #[test]
    fn exponential_filter_is_meshoptimizer_1_1s() {
        let v = [0.0f32, 0.0, 0.0, 0.0, 0.0171467, 0.0, 0.0, 0.5, 0.0];
        let words = |m| -> Vec<u32> {
            encode_filter_exp(&v, 3, 12, 12, m)
                .unwrap()
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        };
        let z = 0xf500_0000;
        assert_eq!(
            words(ExpMode::Separate),
            [z, z, z, z, 0xf000_0464, z, z, 0xf500_0400, z]
        );
        assert_eq!(
            words(ExpMode::SharedVector),
            [z, z, z, z, 0xf500_0023, z, z, 0xf500_0400, z]
        );
        assert_eq!(
            words(ExpMode::SharedComponent),
            [z, z, z, z, 0xf500_0023, z, z, 0xf500_0400, z]
        );
        // NaN encodes as a zero mantissa (no error: Blender exports such meshes).
        let n = encode_filter_exp(&[f32::NAN, 1.0, 0.0], 1, 12, 12, ExpMode::SharedVector);
        assert_eq!(n.unwrap().len(), 12);
    }

    #[test]
    fn quantizers_match_meshoptimizer() {
        assert_eq!(quantize_unorm(0.0, 14), 0);
        assert_eq!(quantize_unorm(1.0, 14), 16383);
        assert_eq!(quantize_unorm(2.0, 8), 255);
        assert_eq!(quantize_unorm(-1.0, 8), 0);
        assert_eq!(quantize_unorm(0.5, 8), 128);
        assert_eq!(quantize_snorm(1.0, 8), 127);
        assert_eq!(quantize_snorm(-1.0, 8), -127);
        assert_eq!(quantize_snorm(-0.5, 8), -64);
        assert_eq!(quantize_snorm(3.0, 16), 32767);
    }

    #[test]
    fn filter_strides_follow_the_spec() {
        assert!(check_filter_stride(MeshoptFilter::Exponential, 12).is_ok());
        assert!(check_filter_stride(MeshoptFilter::Exponential, 6).is_err());
        assert!(check_filter_stride(MeshoptFilter::Quaternion, 8).is_ok());
        assert!(check_filter_stride(MeshoptFilter::Quaternion, 16).is_err());
        assert_eq!(filter_stride(MeshoptFilter::Quaternion, 16), 8);
        assert_eq!(filter_stride(MeshoptFilter::Quaternion, 8), 8);
        assert_eq!(filter_stride(MeshoptFilter::Exponential, 16), 16);
        assert!(check_filter_stride(MeshoptFilter::Octahedral, 4).is_ok());
        assert!(check_filter_stride(MeshoptFilter::Color, 12).is_err());
    }

    #[test]
    fn vertex_fetch_remap_is_first_use_order() {
        let (remap, n) = optimize_vertex_fetch_remap(&[3, 1, 3, 0], 5).unwrap();
        assert_eq!(n, 3);
        assert_eq!(remap, vec![2, 1, u32::MAX, 0, u32::MAX]);
        assert!(optimize_vertex_fetch_remap(&[5], 5).is_err());
    }

    #[cfg(not(feature = "meshopt"))]
    #[test]
    fn without_the_feature_nothing_is_available() {
        assert!(codec().is_none());
        // Pure Rust (meshoptimizer 1.1's), available without the backend.
        assert_eq!(
            encode_filter_exp(&[0.0; 3], 1, 12, 12, ExpMode::SharedVector)
                .unwrap()
                .len(),
            12
        );
        assert_eq!(
            encode_filter_quat(&[0.0, 0.0, 0.0, 1.0], 1, 8),
            Err(unavailable())
        );
        assert!(
            unavailable()
                .to_string()
                .contains("built without the `meshopt` feature")
        );
    }

    #[cfg(feature = "meshopt")]
    mod backend_tests {
        use super::*;

        fn c() -> &'static dyn MeshoptCodec {
            codec().expect("meshopt feature")
        }

        fn f32_bytes(v: &[f32]) -> Vec<u8> {
            v.iter().flat_map(|f| f.to_le_bytes()).collect()
        }

        fn bytes_f32(b: &[u8]) -> Vec<f32> {
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect()
        }

        #[test]
        fn vertex_codec_round_trips_both_versions() {
            let v: Vec<f32> = (0..96).map(|i| (i as f32 * 0.37).sin()).collect();
            let raw = f32_bytes(&v);
            for version in [MeshoptVersion::Ext, MeshoptVersion::Khr] {
                let enc = c().encode_vertex(&raw, 32, 12, version).unwrap();
                // The header byte carries the version (0xa0 | version).
                assert_eq!(enc[0], 0xa0 | version.vertex_version() as u8);
                let dec = c()
                    .decode(&enc, 32, 12, MeshoptMode::Attributes, MeshoptFilter::None)
                    .unwrap();
                assert_eq!(dec, raw);
            }
        }

        #[test]
        fn index_codecs_round_trip() {
            let tris: Vec<u32> = vec![0, 1, 2, 2, 1, 3, 3, 1, 4, 4, 5, 3];
            let enc = c()
                .encode_index(&tris, 6, 2, MeshoptMode::Triangles)
                .unwrap();
            let dec = c()
                .decode(&enc, 12, 2, MeshoptMode::Triangles, MeshoptFilter::None)
                .unwrap();
            let back: Vec<u32> = dec
                .chunks_exact(2)
                .map(|c| u32::from(u16::from_le_bytes([c[0], c[1]])))
                .collect();
            // The index codec may rotate triangles; the triangle set is kept.
            let norm = |t: &[u32]| {
                let mut v: Vec<[u32; 3]> = t
                    .chunks_exact(3)
                    .map(|t| {
                        let r = (0..3).min_by_key(|&k| t[k]).unwrap();
                        [t[r], t[(r + 1) % 3], t[(r + 2) % 3]]
                    })
                    .collect();
                v.sort_unstable();
                v
            };
            assert_eq!(norm(&back), norm(&tris));
            let seq: Vec<u32> = vec![0, 1, 1, 2, 7, 3];
            let enc = c().encode_index(&seq, 8, 4, MeshoptMode::Indices).unwrap();
            let dec = c()
                .decode(&enc, 6, 4, MeshoptMode::Indices, MeshoptFilter::None)
                .unwrap();
            let back: Vec<u32> = dec
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            assert_eq!(back, seq);
        }

        #[test]
        fn exponential_filter_round_trips_within_its_bits() {
            let v = [1.0f32, -0.25, 3.5, 0.001, 100.0, -7.75];
            let filtered = encode_filter_exp(&v, 2, 12, 12, ExpMode::SharedVector).unwrap();
            let enc = c()
                .encode_vertex(&filtered, 2, 12, MeshoptVersion::Ext)
                .unwrap();
            let dec = c()
                .decode(
                    &enc,
                    2,
                    12,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Exponential,
                )
                .unwrap();
            let back = bytes_f32(&dec);
            for (k, (a, b)) in v.iter().zip(&back).enumerate() {
                // Shared exponent per vector: error below 2^-11 of the vector's largest value.
                let m = if k < 3 { 3.5f32 } else { 100.0 };
                assert!((a - b).abs() <= m / 2048.0, "{a} vs {b}");
            }
        }

        /// The add-on's sampled object rotations (`sampled/object/sampler.py`): QUATERNION
        /// with byteStride 16, the filtered quaternions in the first `count * 8` bytes of the
        /// vertex stream; decoded as Blender's bridge decodes them (release
        /// `meshopt_decodeFilterQuat` ignores the stride).
        #[test]
        fn quaternion_filter_at_byte_stride_16_decodes_the_first_half() {
            let h = std::f32::consts::FRAC_1_SQRT_2;
            let q = [0.0f32, 0.0, h, h, 1.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.5];
            let mut filtered = encode_filter_quat(&q, 3, 8).unwrap();
            let packed = filtered.clone();
            filtered.resize(3 * 16, 0);
            let enc = c()
                .encode_vertex(&filtered, 3, 16, MeshoptVersion::Ext)
                .unwrap();
            let dec = c()
                .decode(
                    &enc,
                    3,
                    16,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Quaternion,
                )
                .unwrap();
            assert_eq!(dec.len(), 48);
            // The same quaternions as a tightly packed (stride 8) view decodes to.
            let enc8 = c()
                .encode_vertex(&packed, 3, 8, MeshoptVersion::Ext)
                .unwrap();
            let dec8 = c()
                .decode(
                    &enc8,
                    3,
                    8,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Quaternion,
                )
                .unwrap();
            assert_eq!(&dec[..24], &dec8[..]);
            let back: Vec<f32> = dec[..24]
                .chunks_exact(2)
                .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32767.0)
                .collect();
            for (a, b) in q.iter().zip(&back) {
                assert!((a - b).abs() < 1e-2, "{q:?} vs {back:?}");
            }
            assert!(dec[24..].iter().all(|&b| b == 0));
        }

        #[test]
        fn quaternion_and_colour_filters_round_trip() {
            let h = std::f32::consts::FRAC_1_SQRT_2;
            let q = [0.0f32, 0.0, h, h, 1.0, 0.0, 0.0, 0.0];
            let filtered = encode_filter_quat(&q, 2, 12).unwrap();
            let enc = c()
                .encode_vertex(&filtered, 2, 8, MeshoptVersion::Khr)
                .unwrap();
            let dec = c()
                .decode(
                    &enc,
                    2,
                    8,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Quaternion,
                )
                .unwrap();
            let back: Vec<f32> = dec
                .chunks_exact(2)
                .map(|c| f32::from(i16::from_le_bytes([c[0], c[1]])) / 32767.0)
                .collect();
            for (a, b) in q.iter().zip(&back) {
                assert!((a - b).abs() < 1e-3, "{q:?} vs {back:?}");
            }
            let col = [1.0f32, 0.5, 0.25, 1.0];
            let filtered = encode_filter_color(&col, 1, 4, 8).unwrap();
            let enc = c()
                .encode_vertex(&filtered, 1, 4, MeshoptVersion::Khr)
                .unwrap();
            let dec = c()
                .decode(&enc, 1, 4, MeshoptMode::Attributes, MeshoptFilter::Color)
                .unwrap();
            for (a, b) in col.iter().zip(&dec) {
                assert!(
                    (a - f32::from(*b) / 255.0).abs() < 0.02,
                    "{col:?} vs {dec:?}"
                );
            }
            let n = [0.0f32, 0.0, 1.0, 0.0];
            let filtered = encode_filter_oct(&n, 1, 4, 8).unwrap();
            let enc = c()
                .encode_vertex(&filtered, 1, 4, MeshoptVersion::Ext)
                .unwrap();
            let dec = c()
                .decode(
                    &enc,
                    1,
                    4,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Octahedral,
                )
                .unwrap();
            assert_eq!(dec[2] as i8, 127);
        }

        #[test]
        fn invalid_layouts_are_errors_not_aborts() {
            assert!(
                c().encode_vertex(&[0; 6], 1, 6, MeshoptVersion::Ext)
                    .is_err()
            );
            assert!(
                c().decode(&[0; 8], 1, 6, MeshoptMode::Attributes, MeshoptFilter::None)
                    .is_err()
            );
            assert!(
                c().decode(&[0; 8], 2, 2, MeshoptMode::Triangles, MeshoptFilter::None)
                    .is_err()
            );
            assert!(
                c().decode(
                    &[0; 8],
                    1,
                    12,
                    MeshoptMode::Attributes,
                    MeshoptFilter::Quaternion
                )
                .is_err()
            );
            assert!(
                c().decode(
                    &[0; 4],
                    usize::MAX,
                    4,
                    MeshoptMode::Attributes,
                    MeshoptFilter::None
                )
                .is_err()
            );
            // Garbage input: an error code, never a crash.
            assert!(
                c().decode(
                    &[0xff; 16],
                    3,
                    4,
                    MeshoptMode::Attributes,
                    MeshoptFilter::None
                )
                .is_err()
            );
            assert!(
                c().encode_index(&[0, 1], 2, 2, MeshoptMode::Triangles)
                    .is_err()
            );
            assert!(encode_filter_exp(&[1.0; 3], 1, 6, 12, ExpMode::Separate).is_err());
        }

        #[test]
        fn simplifiers_reduce_a_grid() {
            // A 9×9 vertex grid, 128 triangles.
            let mut pos = Vec::new();
            for y in 0..9 {
                for x in 0..9 {
                    pos.extend([x as f32, y as f32, 0.0]);
                }
            }
            let mut idx = Vec::new();
            for y in 0..8u32 {
                for x in 0..8u32 {
                    let a = y * 9 + x;
                    idx.extend([a, a + 1, a + 10, a, a + 10, a + 9]);
                }
            }
            let out = simplify(&idx, &pos, &[], &[], 24, 1e-2, SimplifyOptions::default()).unwrap();
            assert!(
                out.len() < idx.len() && out.len().is_multiple_of(3),
                "{}",
                out.len()
            );
            let locked = simplify(
                &idx,
                &pos,
                &[],
                &[],
                24,
                1e-2,
                SimplifyOptions {
                    lock_border: true,
                    ..SimplifyOptions::default()
                },
            )
            .unwrap();
            assert!(locked.len() >= out.len());
            let sloppy = simplify_sloppy(&idx, &pos, 24, 1.0).unwrap();
            assert!(sloppy.len() <= idx.len());
            let pts = simplify_points(&pos, None, 10).unwrap();
            assert!(pts.len() <= 10 && !pts.is_empty());
            let opt = optimize_vertex_cache(&idx, 81).unwrap();
            assert_eq!(opt.len(), idx.len());
        }
    }
}
