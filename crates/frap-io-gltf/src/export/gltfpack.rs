// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_use_gltfpack` (WP 7.1): the add-on runs the external `gltfpack` binary on the
//! written file (`blender/exp/export.py::__postprocess_with_gltfpack`, options from the
//! `export_gltfpack_*` props) and gltfpack writes `gltfpacked/<file name>` next to it; frap
//! maps those props onto in-process meshoptimizer passes with gltfpack-equivalent semantics
//! and an INFO report (§17.2), no external binary. `export()` calls [`apply`] on the finished
//! output, which gains the packed file(s) in [`ExportOutput::files`].
//!
//! The command line is the add-on's (`__postprocess_with_gltfpack`): `-tc` (and `-tq N`) with
//! `export_gltfpack_tc`, `-si R` when the ratio is not 1, `-sa`, `-slb`, `-noq`, then `-kn`
//! **or** `-vp -vt -vn -vc` with `-vpi`/`-vpn`/`-vpf` (the add-on passes the bit counts only
//! without `-kn`; with it gltfpack's defaults apply, which equal the props' defaults). The
//! passes follow gltfpack 1.1 (`gltf/mesh.cpp::processMesh`, `gltf/stream.cpp`, MIT, Arseny
//! Kapoulkine), ported in spirit:
//!
//! - every triangle list: degenerate triangles removed (`filterTriangles`), simplified to
//!   `floor(triangles · si) · 3` indices when `si` < 1 (`simplifyMesh`: target error 1e-2,
//!   `meshopt_simplifyWithAttributes` with the normals at weight 0.5 and the premultiplied
//!   colours at 1, the UV mirror-seam split of `simplifyUvSplit`, `LockBorder` with `-slb` else
//!   `Prune`, `Regularize` for morph targets and skins, `meshopt_simplifySloppy` at error 0.1
//!   with `-sa` when the target is not reached), then optimized for the vertex cache and
//!   compacted (`optimizeMesh`); point lists are simplified with `meshopt_simplifyPoints`;
//! - quantization unless `-noq` (`quantize.rs`, with one position transform for the whole
//!   file, gltfpack's `prepareQuantizationPosition`, placed on the mesh nodes, folded into the
//!   instance transforms and into the inverse bind matrices of skinned meshes); colours
//!   (`-vc` bits) and weights (8 bits, renormalized) are quantized even with `-noq`, joints
//!   become UNSIGNED_BYTE when they fit, as gltfpack always does;
//! - `-tc` converts the textures to KTX2 with Basis Universal (`KHR_texture_basisu`) when this
//!   build can encode it (`frap_imbuf::basis`, WP 7.3); otherwise the textures are kept and a
//!   WARNING says so; `-tq` has no equivalent in frap's encoder;
//! - `-kn` keeps named nodes: frap never merges or flattens nodes, so it always holds.
//!
//! What gltfpack does that frap does not (none changes the content): vertex welding
//! (`reindexMesh`), mesh merging and instancing, animation resampling, removal of unused data,
//! spatial sorting of point clouds. meshopt compression of the input is decoded (gltfpack
//! writes uncompressed files without `-c`, which the add-on never passes); a Draco input is
//! not repacked (WARNING). Blender itself, without a gltfpack binary configured (the add-on's
//! `gltfpack_path_ui` preference, empty by default), writes the file and then fails with
//! `PermissionError: [Errno 13] Permission denied: ''` (verified on 5.2.4): deviations §1.

use std::collections::HashMap;

use frap_codec_mesh::meshopt::{self as codec, SimplifyOptions};
use frap_gltf::accessor::{AccessorData, Components, encode_data_uri, read_accessor, uri_to_path};
use frap_gltf::json::{AccessorType, ComponentType, Document, Index, Mode, Primitive};
use frap_gltf::{AccessorOpts, BufferBuilder, Glb, WriteOpts, load_buffers, write_glb};
use frap_rna::{Report, ReportType};
use frap_spec_ops_io::export_scene::gltf::ExportGltfpackVpi;

use crate::export::images::path_to_uri;
use crate::export::mesh::gather::column_data;
use crate::export::mesh::post::meshopt::decode_document;
use crate::export::mesh::post::quantize::{
    self, Dequant, PositionMode, QuantBits, fold_instances, place_dequant,
};
use crate::export::{ExportError, ExportFormat, ExportOutput, ExportSettings};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
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
];

/// The output folder gltfpack writes into, next to the exported file.
pub const OUTPUT_DIR: &str = "gltfpacked";

/// The gltfpack options the add-on passes (see the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    /// `-tc`
    pub textures: bool,
    /// `-tq` (passed with `-tc` when non-zero)
    pub texture_quality: i32,
    /// `-si`
    pub simplify: f32,
    /// `-sa`
    pub aggressive: bool,
    /// `-slb`
    pub lock_borders: bool,
    /// Not `-noq`.
    pub quantize: bool,
    /// `-kn`
    pub keep_named: bool,
    /// `-vp -vt -vn -vc -vpi/-vpn/-vpf` (gltfpack's defaults with `-kn`).
    pub bits: QuantBits,
}

impl Options {
    /// The options of `settings` (as `__postprocess_with_gltfpack` builds them).
    pub fn from_settings(settings: &ExportSettings) -> Options {
        let p = &settings.props;
        let keep_named = p.export_gltfpack_kn.value;
        let bits = if keep_named {
            QuantBits::GLTFPACK
        } else {
            let b = |v: i32| v.clamp(1, 16) as u32;
            QuantBits {
                position: b(p.export_gltfpack_vp.value),
                texcoord: b(p.export_gltfpack_vt.value),
                normal: b(p.export_gltfpack_vn.value),
                color: b(p.export_gltfpack_vc.value).max(2),
                position_mode: match p.export_gltfpack_vpi.value {
                    ExportGltfpackVpi::Normalized => PositionMode::Normalized,
                    ExportGltfpackVpi::FloatingPoint => PositionMode::Float,
                    _ => PositionMode::Integer,
                },
            }
        };
        Options {
            textures: p.export_gltfpack_tc.value,
            texture_quality: p.export_gltfpack_tq.value,
            simplify: p.export_gltfpack_si.value.clamp(0.0, 1.0),
            aggressive: p.export_gltfpack_sa.value,
            lock_borders: p.export_gltfpack_slb.value,
            quantize: !p.export_gltfpack_noq.value,
            keep_named,
            bits,
        }
    }

    /// The command line the add-on would run (without `-i`/`-o`).
    pub fn args(&self) -> String {
        let mut a: Vec<String> = Vec::new();
        if self.textures {
            a.push("-tc".into());
            if self.texture_quality != 0 {
                a.push(format!("-tq {}", self.texture_quality));
            }
        }
        if self.simplify != 1.0 {
            a.push(format!("-si {}", py_float(self.simplify)));
        }
        if self.aggressive {
            a.push("-sa".into());
        }
        if self.lock_borders {
            a.push("-slb".into());
        }
        if !self.quantize {
            a.push("-noq".into());
        }
        if self.keep_named {
            a.push("-kn".into());
        } else {
            let b = &self.bits;
            a.push(format!(
                "-vp {} -vt {} -vn {} -vc {}",
                b.position, b.texcoord, b.normal, b.color
            ));
            a.push(
                match b.position_mode {
                    PositionMode::Integer => "-vpi",
                    PositionMode::Normalized => "-vpn",
                    PositionMode::Float => "-vpf",
                }
                .into(),
            );
        }
        a.join(" ")
    }
}

/// Python's `str` of a float prop (the add-on formats it with an f-string; RNA floats are
/// single precision, read back as doubles).
fn py_float(v: f32) -> String {
    let s = format!("{}", f64::from(v));
    if s.contains(['.', 'e', 'i', 'N']) {
        s
    } else {
        format!("{s}.0")
    }
}

/// Post-processes the written output as gltfpack would (see the module docs).
pub fn apply(
    settings: &ExportSettings,
    mut out: ExportOutput,
) -> Result<ExportOutput, ExportError> {
    if !settings.props.export_use_gltfpack.value {
        return Ok(out);
    }
    let opts = Options::from_settings(settings);
    let name = format!(
        "{}.{}",
        settings.file_stem,
        match settings.export_format {
            ExportFormat::Glb => "glb",
            _ => "gltf",
        }
    );
    let rel = format!("{OUTPUT_DIR}/{name}");
    let mut warnings: Vec<String> = Vec::new();
    let packed = pack(settings, &out, &opts, &mut warnings)?;
    let Some(files) = packed else {
        for w in warnings {
            push_report(&mut out.reports, ReportType::Warning, w);
        }
        return Ok(out);
    };
    let mut files = files;
    files[0].0.clone_from(&rel);
    out.files.extend(files);
    for w in warnings {
        push_report(&mut out.reports, ReportType::Warning, w);
    }
    push_report(
        &mut out.reports,
        ReportType::Info,
        format!(
            "frap: export_use_gltfpack: wrote '{rel}' in-process with gltfpack's semantics for `gltfpack {}` (no external gltfpack binary)",
            opts.args()
        ),
    );
    Ok(out)
}

fn push_report(r: &mut Vec<Report>, level: ReportType, message: String) {
    if !r.iter().any(|x| x.message == message) {
        r.push(Report::new(level, message));
    }
}

/// One primitive's data, decoded.
struct Prim {
    attrs: Streams,
    indices: Option<Vec<u32>>,
    targets: Vec<Streams>,
}

/// Named vertex streams (attributes, or one morph target), in file order.
type Streams = Vec<(String, AccessorData)>;

/// Files written next to the export: `(path relative to it, bytes)`.
type Files = Vec<(String, Vec<u8>)>;

fn err(e: impl std::fmt::Display) -> ExportError {
    ExportError::Invalid(format!("export_use_gltfpack: {e}"))
}

/// Elements `keep` of `d`, in that order.
fn remap(d: &AccessorData, keep: &[u32]) -> AccessorData {
    let n = d.type_.components();
    macro_rules! pick {
        ($v:expr, $w:path) => {
            $w(keep
                .iter()
                .flat_map(|&i| $v[i as usize * n..(i as usize + 1) * n].iter().copied())
                .collect())
        };
    }
    let components = match &d.components {
        Components::I8(v) => pick!(v, Components::I8),
        Components::U8(v) => pick!(v, Components::U8),
        Components::I16(v) => pick!(v, Components::I16),
        Components::U16(v) => pick!(v, Components::U16),
        Components::U32(v) => pick!(v, Components::U32),
        Components::F32(v) => pick!(v, Components::F32),
    };
    AccessorData {
        type_: d.type_,
        normalized: d.normalized,
        components,
    }
}

impl Prim {
    fn get(&self, name: &str) -> Option<&AccessorData> {
        self.attrs.iter().find(|(n, _)| n == name).map(|(_, d)| d)
    }

    fn vertex_count(&self) -> usize {
        self.attrs.first().map_or(0, |(_, d)| d.count())
    }

    /// Keeps vertices `keep` (new vertex `i` = old `keep[i]`) in every stream.
    fn remap_vertices(&mut self, keep: &[u32]) {
        for (_, d) in &mut self.attrs {
            *d = remap(d, keep);
        }
        for t in &mut self.targets {
            for (_, d) in t {
                *d = remap(d, keep);
            }
        }
    }

    fn floats<const N: usize>(&self, name: &str) -> Option<Vec<[f32; N]>> {
        let d = self.get(name)?;
        let f = d.to_f32();
        let n = d.type_.components();
        Some(
            f.chunks_exact(n)
                .map(|c| std::array::from_fn(|k| c.get(k).copied().unwrap_or(0.0)))
                .collect(),
        )
    }
}

/// `simplifyUvSplit`: duplicates the vertices on a UV mirror seam (used by triangles of both
/// UV windings) so the simplifier keeps the seam; returns the duplicated vertices' sources.
fn uv_split(p: &mut Prim, indices: &mut [u32]) -> Vec<u32> {
    let Some(uv) = p.floats::<2>("TEXCOORD_0") else {
        return Vec::new();
    };
    let vertex_count = uv.len();
    let mut sign = vec![0u8; indices.len() / 3];
    let mut flip = vec![0u8; vertex_count];
    for (t, tri) in indices.chunks_exact(3).enumerate() {
        let (a, b, c) = (
            uv[tri[0] as usize],
            uv[tri[1] as usize],
            uv[tri[2] as usize],
        );
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        let f = if area > 0.0 {
            1
        } else if area < 0.0 {
            2
        } else {
            0
        };
        sign[t] = f;
        for &v in tri {
            flip[v as usize] |= f;
        }
    }
    let mut keep: Vec<u32> = (0..vertex_count as u32).collect();
    let mut split = vec![0u32; vertex_count];
    for (v, &f) in flip.iter().enumerate() {
        if f == 3 {
            split[v] = keep.len() as u32;
            keep.push(v as u32);
        }
    }
    if keep.len() == vertex_count {
        return Vec::new();
    }
    for (t, tri) in indices.chunks_exact_mut(3).enumerate() {
        for v in tri {
            if flip[*v as usize] == 3 && sign[t] == 2 {
                *v = split[*v as usize];
            }
        }
    }
    p.remap_vertices(&keep);
    keep
}

/// gltfpack's `processMesh` for one triangle list (see the module docs).
fn process_triangles(p: &mut Prim, opts: &Options) -> Result<(), ExportError> {
    let n = p.vertex_count();
    let mut indices = p.indices.take().unwrap_or_else(|| (0..n as u32).collect());
    indices.truncate(indices.len() / 3 * 3);
    // `filterTriangles`
    let mut kept = Vec::with_capacity(indices.len());
    for t in indices.chunks_exact(3) {
        if t[0] != t[1] && t[0] != t[2] && t[1] != t[2] {
            kept.extend_from_slice(t);
        }
    }
    let mut indices = kept;
    if opts.simplify < 1.0 && !indices.is_empty() {
        let presplit = p.vertex_count();
        let uvremap = uv_split(p, &mut indices);
        let positions: Vec<f32> = p
            .floats::<3>("POSITION")
            .ok_or_else(|| err("a triangle list without POSITION"))?
            .into_iter()
            .flatten()
            .collect();
        let vc = positions.len() / 3;
        let target = ((indices.len() / 3) as f64 * f64::from(opts.simplify)) as usize * 3;
        let mut weights: Vec<f32> = Vec::new();
        let mut attrs: Vec<Vec<f32>> = vec![Vec::new(); vc];
        if let Some(nrm) = p.floats::<3>("NORMAL") {
            weights.extend([0.5; 3]);
            for (a, v) in attrs.iter_mut().zip(nrm) {
                a.extend(v);
            }
        }
        if let Some(col) = p.floats::<4>("COLOR_0") {
            weights.extend([1.0; 3]);
            for (a, c) in attrs.iter_mut().zip(col) {
                a.extend([c[0] * c[3], c[1] * c[3], c[2] * c[3]]);
            }
        }
        let attrs: Vec<f32> = attrs.into_iter().flatten().collect();
        let options = SimplifyOptions {
            lock_border: opts.lock_borders,
            prune: !opts.lock_borders,
            regularize: !p.targets.is_empty() || p.get("WEIGHTS_0").is_some(),
        };
        indices = codec::simplify(
            &indices, &positions, &attrs, &weights, target, 1e-2, options,
        )
        .map_err(err)?;
        if opts.aggressive && indices.len() > target {
            indices = codec::simplify_sloppy(&indices, &positions, target, 1e-1).map_err(err)?;
        }
        if !uvremap.is_empty() {
            for i in &mut indices {
                *i = uvremap[*i as usize];
            }
            let keep: Vec<u32> = (0..presplit as u32).collect();
            p.remap_vertices(&keep);
        }
    }
    // `optimizeMesh`
    let vc = p.vertex_count();
    let indices = codec::optimize_vertex_cache(&indices, vc).map_err(err)?;
    let (remap_table, used) = codec::optimize_vertex_fetch_remap(&indices, vc).map_err(err)?;
    let mut keep = vec![0u32; used];
    for (old, &new) in remap_table.iter().enumerate() {
        if new != u32::MAX {
            keep[new as usize] = old as u32;
        }
    }
    p.remap_vertices(&keep);
    p.indices = Some(indices.iter().map(|&i| remap_table[i as usize]).collect());
    Ok(())
}

/// `simplifyPointMesh`.
fn process_points(p: &mut Prim, opts: &Options) -> Result<(), ExportError> {
    if opts.simplify >= 1.0 || p.indices.is_some() {
        return Ok(());
    }
    let Some(pos) = p.floats::<3>("POSITION") else {
        return Ok(());
    };
    let target = (pos.len() as f64 * f64::from(opts.simplify)) as usize;
    let colors: Option<Vec<f32>> = p
        .floats::<4>("COLOR_0")
        .map(|c| c.into_iter().flat_map(|c| [c[0], c[1], c[2]]).collect());
    let positions: Vec<f32> = pos.into_iter().flatten().collect();
    let keep = codec::simplify_points(&positions, colors.as_deref(), target).map_err(err)?;
    p.remap_vertices(&keep);
    Ok(())
}

/// gltfpack's `renormalizeWeights`.
fn renormalize(w: &mut [u8; 4]) {
    let sum: i32 = w.iter().map(|&x| i32::from(x)).sum();
    if sum == 255 {
        return;
    }
    let mut max = 0;
    for k in 1..4 {
        if w[k] > w[max] {
            max = k;
        }
    }
    w[max] = (i32::from(w[max]) + 255 - sum) as u8;
}

/// The written streams of one primitive, quantized as gltfpack does.
fn quantize_prim(
    p: &Prim,
    opts: &Options,
    dq: Option<&Dequant>,
    all_float: bool,
) -> (Streams, Vec<Streams>) {
    let b = &opts.bits;
    let attrs = p
        .attrs
        .iter()
        .map(|(name, d)| {
            let float = d.component_type() == ComponentType::Float;
            let n = d.type_.components();
            let q = match name.as_str() {
                "POSITION" if opts.quantize && float && all_float => match (b.position_mode, dq) {
                    (PositionMode::Float, _) => Some(AccessorData {
                        components: Components::F32(
                            d.to_f32()
                                .into_iter()
                                .map(|v| quantize::quantize_float(v, b.position))
                                .collect(),
                        ),
                        ..d.clone()
                    }),
                    (_, Some(dq)) => Some(column_data(&quantize::quantize_positions(
                        &p.floats::<3>(name).unwrap_or_default(),
                        dq,
                    ))),
                    _ => None,
                },
                "NORMAL" if opts.quantize && float => Some(column_data(
                    &quantize::quantize_normals(&p.floats::<3>(name).unwrap_or_default(), b.normal),
                )),
                "TANGENT" if opts.quantize && float && n == 4 => {
                    Some(column_data(&quantize::quantize_tangents(
                        &p.floats::<4>(name).unwrap_or_default(),
                        b.normal,
                    )))
                }
                s if s.starts_with("TEXCOORD_") && opts.quantize && float => {
                    quantize::quantize_texcoords(
                        &p.floats::<2>(name).unwrap_or_default(),
                        b.texcoord,
                    )
                    .map(|c| column_data(&c))
                }
                s if s.starts_with("COLOR_") => {
                    let c: Vec<[f32; 4]> = p
                        .floats::<4>(name)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|mut c| {
                            if n == 3 {
                                c[3] = 1.0;
                            }
                            c
                        })
                        .collect();
                    Some(column_data(&quantize::quantize_colors(&c, b.color)))
                }
                s if s.starts_with("WEIGHTS_") && n == 4 => {
                    let w: Vec<[u8; 4]> = p
                        .floats::<4>(name)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|a| {
                            let ws: f32 = a.iter().sum();
                            let wsi = if ws == 0.0 { 0.0 } else { 1.0 / ws };
                            let mut v = a.map(|x| codec::quantize_unorm(x * wsi, 8) as u8);
                            if wsi != 0.0 {
                                renormalize(&mut v);
                            }
                            v
                        })
                        .collect();
                    Some(AccessorData::normalized_from_elements(&w))
                }
                s if s.starts_with("JOINTS_") && n == 4 => {
                    let j: Vec<u32> = d.to_u32().unwrap_or_default();
                    if j.iter().all(|&x| x <= 255) {
                        let v: Vec<[u8; 4]> = j
                            .chunks_exact(4)
                            .map(|c| [c[0] as u8, c[1] as u8, c[2] as u8, c[3] as u8])
                            .collect();
                        Some(AccessorData::from_elements(&v))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            (name.clone(), q.unwrap_or_else(|| d.clone()))
        })
        .collect();
    let targets = p
        .targets
        .iter()
        .map(|t| {
            t.iter()
                .map(|(name, d)| {
                    let q = match (name.as_str(), dq) {
                        ("POSITION", Some(dq))
                            if opts.quantize
                                && all_float
                                && b.position_mode != PositionMode::Float
                                && d.component_type() == ComponentType::Float =>
                        {
                            let v: Vec<[f32; 3]> = d
                                .to_f32()
                                .chunks_exact(3)
                                .map(|c| [c[0], c[1], c[2]])
                                .collect();
                            Some(column_data(&quantize::scale_position_deltas(&v, dq)))
                        }
                        _ => None,
                    };
                    (name.clone(), q.unwrap_or_else(|| d.clone()))
                })
                .collect()
        })
        .collect();
    (attrs, targets)
}

/// The buffers of the written output.
fn load(out: &ExportOutput) -> Result<(Document, Vec<Vec<u8>>), ExportError> {
    let glb = frap_gltf::read(&out.bytes).map_err(err)?;
    let doc = glb.document;
    let buffers = load_buffers(&doc, glb.bin.as_deref(), |uri| {
        let path = uri_to_path(uri);
        out.files
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, b)| b.clone())
            .ok_or_else(|| format!("{path} was not written"))
    })
    .map_err(err)?;
    Ok((doc, buffers))
}

/// Copies accessors as they are, each once.
struct Copier<'a> {
    doc: &'a Document,
    buffers: &'a [Vec<u8>],
    memo: HashMap<(Index, bool), Index>,
}

impl Copier<'_> {
    fn copy(
        &mut self,
        b: &mut BufferBuilder,
        old: Index,
        opts: &AccessorOpts,
    ) -> Result<Index, ExportError> {
        let key = (old, opts.min_max);
        if let Some(&i) = self.memo.get(&key) {
            return Ok(i);
        }
        let d = read_accessor(self.doc, self.buffers, old).map_err(err)?;
        let mut opts = opts.clone();
        opts.name = self
            .doc
            .accessors
            .get(old as usize)
            .and_then(|a| a.name.clone());
        let i = b.push_accessor(&d, &opts);
        self.memo.insert(key, i);
        Ok(i)
    }
}

fn instance_attrs(n: &frap_gltf::json::Node) -> Vec<(String, Index)> {
    n.extensions
        .get_raw("EXT_mesh_gpu_instancing")
        .and_then(|e| e.get("attributes"))
        .and_then(serde_json::Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_u64()? as Index)))
                .collect()
        })
        .unwrap_or_default()
}

fn set_instance_attr(n: &mut frap_gltf::json::Node, key: &str, acc: Index) {
    if let Some(serde_json::Value::Object(m)) = n
        .extensions
        .0
        .get_mut("EXT_mesh_gpu_instancing")
        .and_then(|e| e.get_mut("attributes"))
    {
        m.insert(key.to_owned(), serde_json::json!(acc));
    }
}

/// The packed file(s): `(path, bytes)` with the main file first (its path set by the caller),
/// or `None` when nothing could be packed (with a WARNING).
fn pack(
    settings: &ExportSettings,
    out: &ExportOutput,
    opts: &Options,
    warnings: &mut Vec<String>,
) -> Result<Option<Files>, ExportError> {
    let (mut doc, mut buffers) = load(out)?;
    if doc
        .extensions_used
        .iter()
        .any(|e| e == "KHR_draco_mesh_compression")
    {
        warnings.push(
            "frap: export_use_gltfpack: a Draco-compressed export is not repacked; no gltfpacked file was written"
                .into(),
        );
        return Ok(None);
    }
    if codec::codec().is_none() {
        warnings.push(format!(
            "frap: export_use_gltfpack: {}; no gltfpacked file was written",
            codec::unavailable()
        ));
        return Ok(None);
    }
    decode_document(&mut doc, &mut buffers).map_err(|e| err(format!("{e:?}")))?;
    let src = doc.clone();
    let mut copier = Copier {
        doc: &src,
        buffers: &buffers,
        memo: HashMap::new(),
    };
    let mut b = BufferBuilder::new();
    let mut nd = doc;
    nd.buffers.clear();
    nd.buffer_views.clear();
    nd.accessors.clear();
    for n in MESHOPT_NAMES {
        nd.extensions_used.retain(|e| e != n);
        nd.extensions_required.retain(|e| e != n);
    }
    // Decode every primitive, then process.
    let mut prims: Vec<Vec<Prim>> = Vec::with_capacity(src.meshes.len());
    for m in &src.meshes {
        let mut v = Vec::with_capacity(m.primitives.len());
        for p in &m.primitives {
            frap_math::budget::check_now();
            let read = |a: Index| read_accessor(&src, &buffers, a).map_err(err);
            let mut prim = Prim {
                attrs: p
                    .attributes
                    .iter()
                    .map(|(k, &a)| Ok((k.clone(), read(a)?)))
                    .collect::<Result<_, ExportError>>()?,
                indices: p
                    .indices
                    .map(|a| read(a)?.to_u32().map_err(err))
                    .transpose()?,
                targets: p
                    .targets
                    .iter()
                    .map(|t| {
                        t.iter()
                            .map(|(k, &a)| Ok((k.clone(), read(a)?)))
                            .collect::<Result<_, ExportError>>()
                    })
                    .collect::<Result<_, _>>()?,
            };
            match p.mode {
                None | Some(Mode::Triangles) => process_triangles(&mut prim, opts)?,
                Some(Mode::Points) => process_points(&mut prim, opts)?,
                _ => {}
            }
            v.push(prim);
        }
        prims.push(v);
    }
    // `prepareQuantizationPosition`: one transform for every mesh.
    let dq = (opts.quantize && opts.bits.position_mode != PositionMode::Float)
        .then(|| {
            Dequant::bounds(prims.iter().flatten().flat_map(|p| {
                p.get("POSITION")
                    .filter(|d| d.component_type() == ComponentType::Float)
                    .map(|_| p.floats::<3>("POSITION").unwrap_or_default())
                    .unwrap_or_default()
            }))
        })
        .flatten()
        .map(|(min, max)| {
            Dequant::from_bounds(
                min,
                max,
                opts.bits.position,
                opts.bits.position_mode == PositionMode::Normalized,
            )
        });
    for (mi, m) in prims.iter().enumerate() {
        let all_float = all_float_positions(m);
        let mut out_prims: Vec<Primitive> = Vec::with_capacity(m.len());
        for (pi, p) in m.iter().enumerate() {
            let (attrs, targets) = quantize_prim(p, opts, dq.as_ref(), all_float);
            let mut gp = src.meshes[mi].primitives[pi].clone();
            gp.attributes.clear();
            for (name, d) in &attrs {
                let i = b.push_accessor(d, &AccessorOpts::attribute(name == "POSITION"));
                gp.attributes.insert(name.clone(), i);
            }
            gp.indices = p.indices.as_ref().map(|ix| b.push_indices(ix));
            gp.targets = targets
                .iter()
                .map(|t| {
                    t.iter()
                        .map(|(name, d)| {
                            (
                                name.clone(),
                                b.push_accessor(d, &AccessorOpts::attribute(name == "POSITION")),
                            )
                        })
                        .collect()
                })
                .collect();
            out_prims.push(gp);
        }
        nd.meshes[mi].primitives = out_prims;
    }
    // The dequantization on the nodes, the instances and the skins, for the meshes whose
    // positions were quantized here (FLOAT in every primitive).
    let mut skin_done: HashMap<Index, Index> = HashMap::new();
    let mut instance_done: Vec<usize> = Vec::new();
    if let Some(dq) = dq {
        let map: HashMap<Index, Dequant> = prims
            .iter()
            .enumerate()
            .filter(|(_, m)| all_float_positions(m))
            .map(|(mi, _)| (mi as Index, dq))
            .collect();
        let unplaced = place_dequant(&mut nd, &map);
        for (ni, d) in unplaced.instanced {
            let attrs = instance_attrs(&nd.nodes[ni]);
            let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|&(_, a)| a);
            let count = attrs
                .first()
                .and_then(|&(_, a)| src.accessors.get(a as usize))
                .map_or(0, |a| a.count as usize);
            let read3 = |a: Option<Index>, def: [f32; 3]| -> Result<Vec<[f32; 3]>, ExportError> {
                match a {
                    Some(a) => read_accessor(&src, &buffers, a)
                        .and_then(|d| d.to_f32_elements::<3>())
                        .map_err(err),
                    None => Ok(vec![def; count]),
                }
            };
            let mut t = read3(get("TRANSLATION"), [0.0; 3])?;
            let mut s = read3(get("SCALE"), [1.0; 3])?;
            let r: Vec<[f32; 4]> = match get("ROTATION") {
                Some(a) => read_accessor(&src, &buffers, a)
                    .and_then(|d| d.to_f32_elements::<4>())
                    .map_err(err)?,
                None => vec![[0.0, 0.0, 0.0, 1.0]; count],
            };
            fold_instances(&mut t, &r, &mut s, &d);
            let ta = b.push_accessor(&AccessorData::from_elements(&t), &AccessorOpts::default());
            let sa = b.push_accessor(&AccessorData::from_elements(&s), &AccessorOpts::default());
            set_instance_attr(&mut nd.nodes[ni], "TRANSLATION", ta);
            set_instance_attr(&mut nd.nodes[ni], "SCALE", sa);
            instance_done.push(ni);
        }
        for (ni, d) in unplaced.skinned {
            let Some(si) = nd.nodes[ni].skin else {
                continue;
            };
            if skin_done.contains_key(&si) {
                continue;
            }
            let skin = &src.skins[si as usize];
            let mut mats: Vec<[f32; 16]> = match skin.inverse_bind_matrices {
                Some(a) => read_accessor(&src, &buffers, a)
                    .and_then(|d| d.to_f32_elements::<16>())
                    .map_err(err)?,
                None => vec![
                    [
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0
                    ];
                    skin.joints.len()
                ],
            };
            // `writeJointBindMatrices`
            let (o, ns) = (d.offset, d.node_scale());
            for m in &mut mats {
                m[12] += o[0] * m[0] + o[1] * m[4] + o[2] * m[8];
                m[13] += o[0] * m[1] + o[1] * m[5] + o[2] * m[9];
                m[14] += o[0] * m[2] + o[1] * m[6] + o[2] * m[10];
                for v in &mut m[..12] {
                    *v *= ns;
                }
            }
            let data = AccessorData {
                type_: AccessorType::Mat4,
                normalized: false,
                components: Components::F32(mats.iter().flatten().copied().collect()),
            };
            let a = b.push_accessor(&data, &AccessorOpts::default());
            skin_done.insert(si, a);
            nd.skins[si as usize].inverse_bind_matrices = Some(a);
        }
    }
    // Everything else, copied.
    for (si, s) in src.skins.iter().enumerate() {
        if skin_done.contains_key(&(si as Index)) {
            continue;
        }
        if let Some(a) = s.inverse_bind_matrices {
            nd.skins[si].inverse_bind_matrices =
                Some(copier.copy(&mut b, a, &AccessorOpts::default())?);
        }
    }
    for (ai, a) in src.animations.iter().enumerate() {
        for (si, s) in a.samplers.iter().enumerate() {
            let input = copier.copy(
                &mut b,
                s.input,
                &AccessorOpts {
                    min_max: true,
                    ..AccessorOpts::default()
                },
            )?;
            let output = copier.copy(&mut b, s.output, &AccessorOpts::default())?;
            let ns = &mut nd.animations[ai].samplers[si];
            ns.input = input;
            ns.output = output;
        }
    }
    for ni in 0..src.nodes.len() {
        for (k, a) in instance_attrs(&src.nodes[ni]) {
            if instance_done.contains(&ni) && (k == "TRANSLATION" || k == "SCALE") {
                continue;
            }
            let i = copier.copy(&mut b, a, &AccessorOpts::default())?;
            set_instance_attr(&mut nd.nodes[ni], &k, i);
        }
    }
    let mut extra_files: Vec<(String, Vec<u8>)> = Vec::new();
    let ktx = textures(
        &src,
        &buffers,
        &mut nd,
        &mut b,
        out,
        opts,
        settings,
        warnings,
        &mut extra_files,
    )?;
    if ktx {
        for list in [&mut nd.extensions_used, &mut nd.extensions_required] {
            if !list.iter().any(|e| e == "KHR_texture_basisu") {
                list.push("KHR_texture_basisu".to_owned());
            }
        }
    }
    // Serialize as the export did.
    let mut files: Vec<(String, Vec<u8>)> = vec![(String::new(), Vec::new())];
    let bin = if b.bytes.is_empty() {
        let mut b = b;
        nd.buffer_views.append(&mut b.views);
        nd.accessors.append(&mut b.accessors);
        None
    } else {
        let uri = match settings.export_format {
            ExportFormat::Glb => None,
            ExportFormat::GltfSeparate => Some(path_to_uri(&format!("{}.bin", settings.file_stem))),
            ExportFormat::GltfEmbedded => Some(String::new()),
        };
        let bytes = b.finish(&mut nd, uri);
        match settings.export_format {
            ExportFormat::Glb => Some(bytes),
            ExportFormat::GltfSeparate => {
                files.push((format!("{OUTPUT_DIR}/{}.bin", settings.file_stem), bytes));
                None
            }
            ExportFormat::GltfEmbedded => {
                nd.buffers[0].uri = Some(encode_data_uri("application/octet-stream", &bytes));
                None
            }
        }
    };
    // The accessors are in the document now.
    if quantize_used(&nd) {
        for list in [&mut nd.extensions_used, &mut nd.extensions_required] {
            if !list.iter().any(|e| e == quantize::EXTENSION) {
                list.push(quantize::EXTENSION.to_owned());
            }
        }
    }
    files[0].1 = match settings.export_format {
        ExportFormat::Glb => write_glb(&Glb { document: nd, bin }, &WriteOpts::default()),
        _ => {
            let mut v = nd.to_vec(&WriteOpts {
                indent: Some("\t".to_owned()),
            });
            v.push(b'\n');
            v
        }
    };
    files.extend(extra_files);
    Ok(Some(files))
}

/// Whether every primitive of a mesh has FLOAT positions (only those are quantized, so a mesh
/// `frap_mesh_quantization` already quantized keeps its node transforms).
fn all_float_positions(m: &[Prim]) -> bool {
    !m.is_empty()
        && m.iter().all(|p| {
            p.get("POSITION")
                .is_some_and(|d| d.component_type() == ComponentType::Float)
        })
}

const MESHOPT_NAMES: [&str; 2] = ["EXT_meshopt_compression", "KHR_meshopt_compression"];

fn quantize_used(doc: &Document) -> bool {
    frap_gltf::ext::khr_mesh_quantization::needs_quantization(doc)
}

/// Images: copied (GLB views, `GLTF_SEPARATE` files next to the packed file), or converted to
/// KTX2 with `-tc` when this build can encode it. Returns whether KTX2 was written.
#[allow(clippy::too_many_arguments)]
fn textures(
    src: &Document,
    buffers: &[Vec<u8>],
    nd: &mut Document,
    b: &mut BufferBuilder,
    out: &ExportOutput,
    opts: &Options,
    settings: &ExportSettings,
    warnings: &mut Vec<String>,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<bool, ExportError> {
    let mut ktx_any = false;
    for (ii, img) in src.images.iter().enumerate() {
        let bytes: Option<Vec<u8>> = match (img.buffer_view, &img.uri) {
            (Some(v), _) => Some(
                frap_gltf::accessor::buffer_view_bytes(src, buffers, v)
                    .map_err(err)?
                    .to_vec(),
            ),
            (None, Some(uri)) if uri.starts_with("data:") => None,
            (None, Some(uri)) => {
                let path = uri_to_path(uri);
                out.files
                    .iter()
                    .find(|(p, _)| *p == path)
                    .map(|(_, b)| b.clone())
            }
            (None, None) => None,
        };
        let mut ktx: Option<Vec<u8>> = None;
        if opts.textures
            && let Some(bytes) = &bytes
        {
            match frap_imbuf::decode(bytes)
                .and_then(|i| frap_imbuf::basis::encode_ktx2(&i, frap_imbuf::basis::BasisMode::Etc1s))
            {
                Ok(k) => ktx = Some(k),
                Err(e) => warnings.push(format!(
                    "frap: export_use_gltfpack: export_gltfpack_tc: {e}; the textures were kept as they are"
                )),
            }
        }
        let gi = &mut nd.images[ii];
        let (data, ext) = match &ktx {
            Some(k) => (Some(k.clone()), "ktx2"),
            None => (bytes, ""),
        };
        if ktx.is_some() {
            gi.mime_type = Some("image/ktx2".into());
            ktx_any = true;
        }
        match (settings.export_format, data) {
            (ExportFormat::Glb, Some(d)) | (ExportFormat::GltfEmbedded, Some(d))
                if img.buffer_view.is_some() =>
            {
                gi.buffer_view = Some(b.push_view(&d, 4, None, None));
            }
            (_, Some(d)) => {
                if let Some(uri) = &img.uri
                    && !uri.starts_with("data:")
                {
                    let mut path = uri_to_path(uri);
                    if !ext.is_empty() {
                        path = match path.rsplit_once('.') {
                            Some((stem, _)) => format!("{stem}.{ext}"),
                            None => format!("{path}.{ext}"),
                        };
                        gi.uri = Some(path_to_uri(&path));
                    }
                    files.push((format!("{OUTPUT_DIR}/{path}"), d));
                }
            }
            (_, None) => {}
        }
    }
    if ktx_any {
        for t in &mut nd.textures {
            if let Some(s) = t.source
                && nd
                    .images
                    .get(s as usize)
                    .and_then(|i| i.mime_type.as_deref())
                    == Some("image/ktx2")
            {
                t.extensions
                    .set(Some(frap_gltf::ext::khr_texture_basisu::KhrTextureBasisu {
                        source: Some(s),
                        ..Default::default()
                    }));
                t.source = None;
            }
        }
    }
    Ok(ktx_any)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_line_is_the_add_ons() {
        let s = ExportSettings::default();
        let o = Options::from_settings(&s);
        assert!(!o.quantize);
        assert_eq!(o.args(), "-tc -tq 8 -noq -vp 14 -vt 12 -vn 8 -vc 8 -vpi");
        let mut p = (*s.props).clone();
        p.export_gltfpack_kn.value = true;
        p.export_gltfpack_si.value = 0.5;
        p.export_gltfpack_tc.value = false;
        p.export_gltfpack_noq.value = false;
        p.export_gltfpack_vp.value = 10;
        let s2 = ExportSettings {
            props: std::sync::Arc::new(p),
            ..ExportSettings::default()
        };
        let o2 = Options::from_settings(&s2);
        assert_eq!(o2.args(), "-si 0.5 -kn");
        // With -kn the bit counts are not passed: gltfpack's defaults.
        assert_eq!(o2.bits, QuantBits::GLTFPACK);
        assert!(o2.quantize);
    }

    #[test]
    fn weights_are_renormalized() {
        let mut w = [85u8, 85, 84, 0];
        renormalize(&mut w);
        assert_eq!(w.iter().map(|&x| u32::from(x)).sum::<u32>(), 255);
        assert_eq!(w, [86, 85, 84, 0]);
    }

    use crate::export::mesh::post::meshopt::tests::{run_export, settings};
    use serde_json::Value as J;

    fn packed(out: &ExportOutput, name: &str) -> J {
        let f = out
            .files
            .iter()
            .find(|(p, _)| p == name)
            .unwrap_or_else(|| {
                panic!(
                    "{name} not written: {:?}",
                    out.files.iter().map(|f| &f.0).collect::<Vec<_>>()
                )
            });
        let glb = frap_gltf::read(&f.1).unwrap();
        serde_json::to_value(&glb.document).unwrap()
    }

    #[test]
    fn default_props_write_the_packed_file_with_an_info_report() {
        let (main, win) = frap_scene::startup::factory();
        let out = run_export(
            &main,
            &win,
            &settings(|p| p.export_use_gltfpack.value = true),
        );
        // The export itself is unchanged.
        let plain = run_export(&main, &win, &settings(|_| {}));
        assert_eq!(out.bytes, plain.bytes);
        let j = packed(&out, "gltfpacked/out.glb");
        // -noq by default: no quantization; colours and weights would still be quantized.
        assert!(j.get("extensionsUsed").is_none(), "{}", j["extensionsUsed"]);
        let acc = &j["accessors"];
        let prim = &j["meshes"][0]["primitives"][0];
        let pos = &acc[prim["attributes"]["POSITION"].as_u64().unwrap() as usize];
        assert_eq!(pos["componentType"], 5126);
        assert_eq!(acc[prim["indices"].as_u64().unwrap() as usize]["count"], 36);
        let info: Vec<_> = out
            .reports
            .iter()
            .filter(|r| r.level == ReportType::Info)
            .map(|r| r.message.as_str())
            .collect();
        assert_eq!(
            info,
            [
                "frap: export_use_gltfpack: wrote 'gltfpacked/out.glb' in-process with gltfpack's semantics for `gltfpack -tc -tq 8 -noq -vp 14 -vt 12 -vn 8 -vc 8 -vpi` (no external gltfpack binary)"
            ]
        );
        let f = &out
            .files
            .iter()
            .find(|f| f.0 == "gltfpacked/out.glb")
            .unwrap()
            .1;
        let r = frap_gltf::validate::validate_file(f).unwrap();
        assert!(r.is_ok(), "{r}");
    }

    #[test]
    fn quantization_follows_gltfpack() {
        let (main, win) = frap_scene::startup::factory();
        let out = run_export(
            &main,
            &win,
            &settings(|p| {
                p.export_use_gltfpack.value = true;
                p.export_gltfpack_noq.value = false;
                p.export_gltfpack_vp.value = 12;
                p.export_gltfpack_vpi.value = ExportGltfpackVpi::Normalized;
                p.export_meshopt_compression_enable.value = true;
            }),
        );
        let j = packed(&out, "gltfpacked/out.glb");
        // The meshopt input is decoded: gltfpack writes uncompressed files without -c.
        assert_eq!(
            j["extensionsUsed"],
            serde_json::json!(["KHR_mesh_quantization"])
        );
        assert_eq!(
            j["extensionsRequired"],
            serde_json::json!(["KHR_mesh_quantization"])
        );
        let acc = &j["accessors"];
        let prim = &j["meshes"][0]["primitives"][0];
        let at = |k: &str| &acc[prim["attributes"][k].as_u64().unwrap() as usize];
        assert_eq!(at("POSITION")["componentType"], 5123);
        assert_eq!(at("POSITION")["normalized"], true);
        assert_eq!(
            at("POSITION")["max"],
            serde_json::json!([4095.0, 4095.0, 4095.0])
        );
        assert_eq!(at("NORMAL")["componentType"], 5120);
        assert_eq!(at("TEXCOORD_0")["componentType"], 5123);
        // The cube spans [-1, 1]: offset -1, node scale 2 / 4095 × 65535 (normalized).
        let n = &j["nodes"][0];
        assert_eq!(n["translation"], serde_json::json!([-1.0, -1.0, -1.0]));
        let s = n["scale"][0].as_f64().unwrap();
        assert!(
            (s - f64::from(2.0f32 / 4095.0 * 65535.0)).abs() < 1e-3,
            "{s}"
        );
        let f = &out
            .files
            .iter()
            .find(|f| f.0 == "gltfpacked/out.glb")
            .unwrap()
            .1;
        let r = frap_gltf::validate::validate_file(f).unwrap();
        assert!(r.errors().next().is_none(), "{r}");
    }

    /// A smooth 9×9 grid as an export output (one mesh, one node).
    fn grid_output() -> ExportOutput {
        let mut pos = Vec::new();
        let mut nrm = Vec::new();
        for y in 0..9 {
            for x in 0..9 {
                pos.push([x as f32, 0.0, y as f32]);
                nrm.push([0.0f32, 1.0, 0.0]);
            }
        }
        let mut idx = Vec::new();
        for y in 0..8u32 {
            for x in 0..8u32 {
                let a = y * 9 + x;
                idx.extend([a, a + 9, a + 1, a + 1, a + 9, a + 10]);
            }
        }
        let mut b = BufferBuilder::new();
        let p = b.push_accessor(
            &AccessorData::from_elements(&pos),
            &AccessorOpts::attribute(true),
        );
        let n = b.push_accessor(
            &AccessorData::from_elements(&nrm),
            &AccessorOpts::attribute(false),
        );
        let i = b.push_indices(&idx);
        let mut doc = Document::default();
        let mut prim = Primitive::default();
        prim.attributes.insert("POSITION".into(), p);
        prim.attributes.insert("NORMAL".into(), n);
        prim.indices = Some(i);
        doc.meshes.push(frap_gltf::json::Mesh {
            primitives: vec![prim],
            ..Default::default()
        });
        doc.nodes.push(frap_gltf::json::Node {
            mesh: Some(0),
            ..Default::default()
        });
        doc.scenes.push(frap_gltf::json::Scene {
            nodes: vec![0],
            ..Default::default()
        });
        doc.scene = Some(0);
        let bin = b.finish(&mut doc, None);
        ExportOutput {
            bytes: write_glb(
                &Glb {
                    document: doc.clone(),
                    bin: Some(bin),
                },
                &WriteOpts::default(),
            ),
            files: Vec::new(),
            extensions_used: Vec::new(),
            nodes: 1,
            meshes: 1,
            reports: Vec::new(),
            document: doc,
        }
    }

    #[test]
    fn simplification_reaches_the_ratio() {
        for (si, lock) in [(0.25f32, false), (0.25, true)] {
            let s = settings(|p| {
                p.export_use_gltfpack.value = true;
                p.export_gltfpack_si.value = si;
                p.export_gltfpack_slb.value = lock;
                p.export_gltfpack_tc.value = false;
            });
            let out = apply(&s, grid_output()).unwrap();
            let j = packed(&out, "gltfpacked/out.glb");
            let prim = &j["meshes"][0]["primitives"][0];
            let count = j["accessors"][prim["indices"].as_u64().unwrap() as usize]["count"]
                .as_u64()
                .unwrap();
            let verts =
                j["accessors"][prim["attributes"]["POSITION"].as_u64().unwrap() as usize]["count"]
                    .as_u64()
                    .unwrap();
            assert!(count <= 96 && count > 0, "{count}");
            let bytes = &out
                .files
                .iter()
                .find(|f| f.0 == "gltfpacked/out.glb")
                .unwrap()
                .1;
            let glb = frap_gltf::read(bytes).unwrap();
            let bufs =
                load_buffers(&glb.document, glb.bin.as_deref(), |_| Err(String::new())).unwrap();
            let pos = read_accessor(
                &glb.document,
                &bufs,
                prim["attributes"]["POSITION"].as_u64().unwrap() as u32,
            )
            .unwrap()
            .to_f32_elements::<3>()
            .unwrap();
            let border = pos
                .iter()
                .filter(|p| p[0] == 0.0 || p[0] == 8.0 || p[2] == 0.0 || p[2] == 8.0)
                .count();
            if lock {
                // `-slb`: every one of the 32 border vertices stays.
                assert_eq!(border, 32, "{pos:?}");
            } else {
                assert!(border < 32, "{pos:?}");
            }
            // Unused vertices are dropped.
            assert!(verts < 81, "{verts}");
            assert!(
                out.reports
                    .iter()
                    .any(|r| r.message.contains(&format!("-si {si}")))
            );
        }
    }

    #[test]
    fn textures_without_an_encoder_are_kept_with_a_warning() {
        let mut out = grid_output();
        let png = frap_imbuf::encode(
            &frap_imbuf::DecodedImage {
                width: 1,
                height: 1,
                channels: 4,
                pixels: frap_imbuf::PixelData::U8(vec![255, 0, 0, 255]),
            },
            &frap_imbuf::EncodeFormat::Png {
                bit_depth: 8,
                compression: 15,
            },
        );
        let Ok(png) = png else { return };
        let mut doc = out.document.clone();
        let glb = frap_gltf::read(&out.bytes).unwrap();
        let mut bin = glb.bin.unwrap();
        let off = bin.len();
        bin.extend_from_slice(&png);
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        doc.buffer_views.push(frap_gltf::json::BufferView {
            buffer: 0,
            byte_offset: Some(off as u64),
            byte_length: png.len() as u64,
            ..Default::default()
        });
        doc.buffers[0].byte_length = bin.len() as u64;
        doc.images.push(frap_gltf::json::Image {
            buffer_view: Some((doc.buffer_views.len() - 1) as Index),
            mime_type: Some("image/png".into()),
            ..Default::default()
        });
        out.bytes = write_glb(
            &Glb {
                document: doc,
                bin: Some(bin),
            },
            &WriteOpts::default(),
        );
        let s = settings(|p| p.export_use_gltfpack.value = true);
        let out = apply(&s, out).unwrap();
        let w: Vec<_> = out
            .reports
            .iter()
            .filter(|r| r.level == ReportType::Warning)
            .collect();
        if !frap_imbuf::basis::CAN_ENCODE {
            assert_eq!(w.len(), 1, "{w:?}");
            assert!(
                w[0].message.contains("export_gltfpack_tc"),
                "{}",
                w[0].message
            );
        }
        let j = packed(&out, "gltfpacked/out.glb");
        assert_eq!(j["images"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn remap_picks_elements() {
        let d = AccessorData::from_elements(&[[1u16, 2], [3, 4], [5, 6]]);
        let r = remap(&d, &[2, 0]);
        assert_eq!(r, AccessorData::from_elements(&[[5u16, 6], [1, 2]]));
        assert_eq!(py_float(0.5), "0.5");
        assert_eq!(py_float(0.0), "0.0");
    }
}
