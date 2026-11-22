// SPDX-License-Identifier: GPL-3.0-or-later
//! Bit-exact face-varying (UV) evaluation: OpenSubdiv 3.7's `Far` pipeline as Blender's
//! evaluator drives it (`intern/opensubdiv/internal/evaluator/evaluator_impl.cc`,
//! `openSubdiv_createEvaluatorFromTopologyRefiner`), so the subdivided UVs of every corner are
//! the same `f32` values as Blender's — including the one- or two-ulp differences between
//! corners that share a vertex but are evaluated in different patches, which decide how many
//! vertices the glTF exporter writes (deviations D-121, D-124; ARCHITECTURE §22.10 WP 8.11).
//!
//! Pipeline (all in `f32` where OpenSubdiv uses `float`, with clang's `-ffp-contract=on` fused
//! multiply-adds as `mul_add`):
//!
//! 1. [`refiner`]: the base level from Blender's converter (partial topology, edge and vertex
//!    sharpness, the UV maps' face-varying channels), refined adaptively to `quality` (`SIMPLE`:
//!    level 1) with `considerFVarChannels` and `useInfSharpPatch` ([`level`], [`refinement`],
//!    [`fvar_level`], [`fvar_refinement`]), or uniformly to `levels` without the limit surface.
//! 2. [`stencil`]: the face-varying stencil table (`StencilTableFactory`,
//!    `INTERPOLATE_FACE_VARYING`, intermediate levels, factorised) through [`primvar`].
//! 3. [`patch_table`]: the face-varying patches (regular B-spline, Gregory end caps from
//!    [`gregory`] with shared local points, or bilinear) and their local-point stencils,
//!    appended to the stencil table (`AppendLocalPointStencilTableFaceVarying`).
//! 4. [`eval`]: `CpuEvaluator::EvalStencils` on the coarse UVs, then per output corner Blender's
//!    `PatchMap::FindPatch` and `CpuEvaluator::EvalPatches` at its ptex coordinates.
//!
//! Ported from OpenSubdiv 3.7.0 (Pixar, TOST / Apache-2.0 modified, GPL-compatible), the
//! version Blender 5.2.4 ships; `// Ported from` notes name the source of each part.

#![allow(clippy::needless_range_loop)]

mod eval;
mod fvar_level;
mod fvar_refinement;
mod gregory;
mod level;
mod patch_builder;
mod patch_table;
mod primvar;
mod refinement;
mod refiner;
mod sdc;
mod stencil;
mod tags;
mod trig;

use crate::catmull_clark::{Manifold, crease_to_sharpness, uv_value_indices};
use crate::{FVarLinear, SubdivError, SubdivSettings, SubdivTopology, VtxBoundary};

/// `Vtr::Index`
pub(crate) type Index = i32;
/// `Vtr::LocalIndex`
pub(crate) type LocalIndex = u16;
/// `Vtr::INDEX_INVALID`
pub(crate) const INDEX_INVALID: Index = -1;
/// `Vtr::VALENCE_LIMIT`
pub(crate) const VALENCE_LIMIT: i32 = (1 << 16) - 1;

/// The OpenSubdiv refinement Blender sets up for `settings`
/// (`BKE_subsurf_modifier_settings_init`): adaptive to `quality` with the limit surface
/// (`SIMPLE`: level 1), uniform to `levels` without it.
fn refine_level(s: &SubdivSettings) -> (bool, u32) {
    let adaptive = s.use_limit_surface;
    let level = if s.is_simple {
        1
    } else if adaptive {
        s.quality
    } else {
        s.level
    };
    (adaptive, level)
}

fn sdc_options(s: &SubdivSettings) -> sdc::SdcOptions {
    sdc::SdcOptions {
        vtx_boundary: match s.vtx_boundary {
            VtxBoundary::None => sdc::VtxBoundary::None,
            VtxBoundary::EdgeOnly => sdc::VtxBoundary::EdgeOnly,
            VtxBoundary::EdgeAndCorner => sdc::VtxBoundary::EdgeAndCorner,
        },
        fvar_linear: match s.fvar_linear {
            FVarLinear::None => sdc::FVarLinear::None,
            FVarLinear::CornersOnly => sdc::FVarLinear::CornersOnly,
            FVarLinear::CornersPlusOne => sdc::FVarLinear::CornersPlus1,
            FVarLinear::CornersPlusTwo => sdc::FVarLinear::CornersPlus2,
            FVarLinear::Boundaries => sdc::FVarLinear::Boundaries,
            FVarLinear::All => sdc::FVarLinear::All,
        },
    }
}

/// The base mesh as `subdiv_converter_mesh.cc` describes it to OpenSubdiv, plus the coarse
/// face-varying values of every UV map (`set_face_varying_data_from_uv`: the last corner of a
/// value wins).
fn base_mesh(
    topo: &SubdivTopology<'_>,
    s: &SubdivSettings,
) -> (refiner::BaseMesh, Vec<Vec<[f32; 2]>>) {
    let m = Manifold::new(topo);
    let nf = topo.face_offsets.len().saturating_sub(1);
    let mut mesh = refiner::BaseMesh {
        num_vertices: m.nverts as i32,
        face_sizes: (0..nf)
            .map(|f| (topo.face_offsets[f + 1] - topo.face_offsets[f]) as i32)
            .collect(),
        face_verts: topo
            .corner_verts
            .iter()
            .map(|&v| m.vert[v as usize].map_or(0, |x| x as Index))
            .collect(),
        ..refiner::BaseMesh::default()
    };
    for (e, ev) in topo.edges.iter().enumerate() {
        if m.loose_edges[e] {
            continue;
        }
        let sharp = if s.is_simple {
            10.0
        } else if let (true, Some(c)) = (s.use_creases, topo.edge_creases) {
            crease_to_sharpness(c[e])
        } else {
            0.0
        };
        let (Some(a), Some(b)) = (m.vert[ev[0] as usize], m.vert[ev[1] as usize]) else {
            continue;
        };
        mesh.edge_sharpness.push(([a as Index, b as Index], sharp));
    }
    mesh.inf_sharp_verts = vec![false; m.nverts];
    mesh.vert_sharpness = vec![0.0; m.nverts];
    for (v, mv) in m.vert.iter().enumerate() {
        let Some(bv) = *mv else { continue };
        let bv = bv as usize;
        mesh.inf_sharp_verts[bv] = s.is_simple || m.inf_sharp_verts[v];
        if let (true, Some(c)) = (s.use_creases, topo.vert_creases) {
            mesh.vert_sharpness[bv] = crease_to_sharpness(c[v]);
        }
    }
    let mut values = Vec::with_capacity(topo.uv_layers.len());
    for uv in &topo.uv_layers {
        let (idx, n) = uv_value_indices(topo, uv);
        let mut data = vec![[0.0f32; 2]; n];
        for (c, &i) in idx.iter().enumerate() {
            data[i as usize] = uv[c];
        }
        mesh.fvar
            .push((idx.iter().map(|&i| i as Index).collect(), n as i32));
        values.push(data);
    }
    (mesh, values)
}

/// The UV maps of `topo` subdivided with `settings` at the output corners `corner_ptex`
/// (`(ptex face, u, v)` of every corner, in Blender's order), bit-identical to Blender's
/// `eval_face_varying`. `None` when the mesh has no faces or no UV maps.
pub(crate) fn evaluate_uvs(
    topo: &SubdivTopology<'_>,
    settings: &SubdivSettings,
    corner_ptex: &[(u32, f32, f32)],
) -> Result<Option<Vec<Vec<[f32; 2]>>>, SubdivError> {
    if topo.uv_layers.is_empty() || topo.face_offsets.len() < 2 {
        return Ok(None);
    }
    let (mesh, coarse) = base_mesh(topo, settings);
    if mesh.num_vertices == 0 {
        return Ok(None);
    }
    let mut r = refiner::TopologyRefiner::create(&mesh, sdc_options(settings))
        .map_err(|e| SubdivError::Topology(format!("OpenSubdiv Error: {}", e.0)))?;
    let (adaptive, level) = refine_level(settings);
    if adaptive {
        r.refine_adaptive(level, true);
    } else {
        r.refine_uniform(level);
    }
    let table = patch_table::create(&r);
    let map = eval::PatchMap::new(&table.params);
    let mut out = Vec::with_capacity(coarse.len());
    for (c, values) in coarse.iter().enumerate() {
        let base = stencil::create_fvar_table(&r, c, adaptive);
        let full = stencil::append_local_point_table(&r, &base, &table.local[c], c)
            .map_err(SubdivError::Topology)?
            .unwrap_or(base);
        let nc = full.num_control as usize;
        let mut data = vec![[0.0f32; 2]; nc + full.num_stencils()];
        data[..values.len().min(nc)].copy_from_slice(&values[..values.len().min(nc)]);
        full.eval_into(&mut data);
        let src_base = if table.uniform { nc } else { 0 };
        let patches = &table.fvar[c];
        let mut uv = Vec::with_capacity(corner_ptex.len());
        for &(ptex, u, v) in corner_ptex {
            frap_math::budget::check();
            let value = match map.find(ptex as i32, u as f64, v as f64) {
                Some(h) => eval::eval_patch(&patches[h], u, v, &data, src_base),
                None => [0.0, 0.0],
            };
            uv.push(value);
        }
        out.push(uv);
    }
    Ok(Some(out))
}

#[cfg(test)]
mod oracle_tests;
