// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSubdiv backend over `opensubdiv-rs` (feature `osd`, ARCHITECTURE §3.6 `subdiv-osd`, on by
//! default since the M4 gate): the same base level and the same output layout as
//! [`crate::catmull_clark`], but the surface comes from the crate's `TopologyRefiner` refined
//! as Blender refines it (`evaluator_impl.cc`: adaptive to `quality` with
//! `considerFVarChannels` for the limit surface, uniform otherwise) and its patch tables with
//! Gregory end caps, evaluated at every ptex coordinate of the layout like
//! `eval_limit_point` / `eval_face_varying`.
//!
//! frap's own backend stays the default subdivider; it calls this one where Blender evaluates
//! Gregory end caps (`levels > quality`, `SIMPLE` past level 1; decision log M34S3-1).
//! `opensubdiv-rs` 0.7 has no `generateLegacySharpCornerPatches`, so smooth boundary corners
//! are evaluated as smooth end caps where Blender's OpenSubdiv uses sharp corner patches
//! (within `6e-3` there; the own backend keeps those faces), and its face-varying patches are
//! built over a separate value mesh (UVs inside face-varying Gregory caps within `5e-3`,
//! D-121). Positions are exact elsewhere, including the Gregory caps
//! (`tests/blender_dumps.rs::osd_backend_matches_blender`). Without `use_limit_surface` it
//! uses frap's own uniform refinement.

use opensubdiv_rs::far::{
    AdaptiveOptions, FVarChannelDescriptor, PatchMap, PatchTableFactory, PatchTableOptions,
    PrimvarRefiner, TopologyDescriptor, TopologyRefinerFactory,
};
use opensubdiv_rs::sdc;

use crate::catmull_clark::{
    Manifold, base_level, build_result, interpolate_position_on_edge, loose_edge_vert_map,
    uv_value_indices,
};
use crate::layout::{self, PosSource};
use crate::{
    FVarLinear, SubdivError, SubdivResult, SubdivSettings, SubdivTopology, Subdivider, VtxBoundary,
};

/// The `opensubdiv-rs` subdivider.
#[derive(Copy, Clone, Debug, Default)]
pub struct OpenSubdiv;

fn sdc_options(s: &SubdivSettings) -> sdc::Options {
    let vtx = match s.vtx_boundary {
        VtxBoundary::None => sdc::VtxBoundaryInterpolation::None,
        VtxBoundary::EdgeAndCorner => sdc::VtxBoundaryInterpolation::EdgeAndCorner,
        _ => sdc::VtxBoundaryInterpolation::EdgeOnly,
    };
    let fvar = match s.fvar_linear {
        FVarLinear::None => sdc::FVarLinearInterpolation::None,
        FVarLinear::CornersOnly => sdc::FVarLinearInterpolation::CornersOnly,
        FVarLinear::CornersPlusOne => sdc::FVarLinearInterpolation::CornersPlus1,
        FVarLinear::CornersPlusTwo => sdc::FVarLinearInterpolation::CornersPlus2,
        FVarLinear::Boundaries => sdc::FVarLinearInterpolation::Boundaries,
        _ => sdc::FVarLinearInterpolation::All,
    };
    sdc::Options::new()
        .with_vtx_boundary_interpolation(vtx)
        .with_fvar_linear_interpolation(fvar)
        .with_creasing_method(sdc::CreasingMethod::Uniform)
}

impl Subdivider for OpenSubdiv {
    fn name(&self) -> &'static str {
        "opensubdiv"
    }

    fn subdivide(
        &self,
        topo: &SubdivTopology<'_>,
        settings: &SubdivSettings,
    ) -> Result<SubdivResult, SubdivError> {
        let s = settings;
        if s.level == 0 {
            return Ok(SubdivResult::identity(topo));
        }
        if s.level > 16 {
            return Err(SubdivError::TooLarge(format!(
                "subdivision level {} is too high",
                s.level
            )));
        }
        if !s.use_limit_surface {
            // Blender evaluates uniform refinement through bilinear patches over the refined
            // vertices (`PatchTableFactory::createUniform`), which frap's own refinement
            // reproduces exactly; `opensubdiv-rs` builds bicubic patches there.
            return crate::catmull_clark::CatmullClark.subdivide(topo, s);
        }
        let resolution = (1u32 << s.level) + 1;
        let lay = layout::compute(topo.coarse(), resolution);
        let m = Manifold::new(topo);
        let nf = topo.face_offsets.len().saturating_sub(1);
        let nl = lay.corner_verts.len();
        let vert_to_edges = loose_edge_vert_map(topo, &m);
        let mut positions = vec![[0.0f32; 3]; lay.vert_pos.len()];
        let mut uv_layers = vec![vec![[0.0f32; 2]; nl]; topo.uv_layers.len()];
        let has_surface = nf > 0 && m.nverts > 0;
        let mut eval_pos: Box<dyn Fn(u32, f32, f32) -> [f32; 3]> = Box::new(|_, _, _| [0.0; 3]);
        if has_surface {
            let (base, reverse) = base_level(topo, s, &m)?;
            let verts_per_face: Vec<usize> = (0..nf)
                .map(|f| (topo.face_offsets[f + 1] - topo.face_offsets[f]) as usize)
                .collect();
            let mut crease_pairs = Vec::new();
            let mut crease_weights = Vec::new();
            for e in 0..base.nedges() {
                if base.edge_sharpness[e] > 0.0 {
                    crease_pairs.push(base.edge_verts[e]);
                    crease_weights.push(base.edge_sharpness[e]);
                }
            }
            let mut corner_verts = Vec::new();
            let mut corner_weights = Vec::new();
            for v in 0..base.nverts {
                if base.vert_sharpness[v] > 0.0 {
                    corner_verts.push(v as u32);
                    corner_weights.push(base.vert_sharpness[v]);
                }
            }
            let uv_idx: Vec<(Vec<u32>, usize)> = topo
                .uv_layers
                .iter()
                .map(|uv| uv_value_indices(topo, uv))
                .collect();
            let channels: Vec<FVarChannelDescriptor<'_>> = uv_idx
                .iter()
                .map(|(idx, n)| FVarChannelDescriptor::new(*n, idx))
                .collect();
            let descriptor =
                TopologyDescriptor::new(base.nverts, &verts_per_face, &base.face_verts)
                    .with_creases(&crease_pairs, &crease_weights)
                    .with_corners(&corner_verts, &corner_weights)
                    .with_fvar_channels(&channels);
            let mut refiner = TopologyRefinerFactory::create(
                descriptor,
                sdc::SchemeType::Catmark,
                sdc_options(s),
            )
            .map_err(|e| SubdivError::Topology(format!("OpenSubdiv Error: {e}")))?;
            let level = if s.is_simple {
                1
            } else {
                s.quality.max(1) as usize
            };
            refiner.refine_adaptive(
                AdaptiveOptions::new(level).with_consider_fvar_channels(!channels.is_empty()),
            );
            let table = PatchTableFactory::create_with_options(
                &refiner,
                &PatchTableOptions::new()
                    .with_fvar_tables(!channels.is_empty())
                    .with_fvar_legacy_linear_patches(false),
            )
            .map_err(|e| SubdivError::Topology(format!("OpenSubdiv Error: {e}")))?;
            let primvar = PrimvarRefiner::new(&refiner);
            let mut control: Vec<[f32; 3]> = reverse
                .iter()
                .map(|&v| topo.positions[v as usize])
                .collect();
            for lvl in primvar.interpolate_all(&control.clone()) {
                control.extend(lvl);
            }
            let map = PatchMap::new(&table);
            let mut fvar_values: Vec<Vec<[f32; 2]>> = Vec::new();
            for (ch, (idx, n)) in uv_idx.iter().enumerate() {
                let uv = topo.uv_layers[ch];
                let mut base_vals = vec![[0.0f32; 2]; *n];
                for (c, &i) in idx.iter().enumerate() {
                    base_vals[i as usize] = uv[c];
                }
                let mut vals = base_vals.clone();
                for lvl in primvar.interpolate_face_varying_all(ch, &base_vals) {
                    vals.extend(lvl);
                }
                fvar_values.push(vals);
            }
            for (l, &(ptex, u, v)) in lay.corner_ptex.iter().enumerate() {
                if let Some(p) = map.find_patch(ptex as usize, u, v) {
                    for (ch, vals) in fvar_values.iter().enumerate() {
                        uv_layers[ch][l] = table.evaluate_face_varying(p, u, v, vals, ch).0;
                    }
                }
            }
            let mut cache: Vec<[f32; 3]> = Vec::with_capacity(lay.vert_pos.len());
            for src in &lay.vert_pos {
                if let PosSource::Ptex { ptex, u, v } = *src {
                    let p = map
                        .find_patch(ptex as usize, u, v)
                        .map_or([0.0; 3], |p| table.evaluate(p, u, v, &control).0);
                    cache.push(p);
                } else {
                    cache.push([0.0; 3]);
                }
            }
            eval_pos = Box::new(move |i, _, _| cache[i as usize]);
        }
        for (i, src) in lay.vert_pos.iter().enumerate() {
            positions[i] = match *src {
                PosSource::Coarse(v) => topo.positions[v as usize],
                PosSource::LooseEdge { edge, u } => {
                    interpolate_position_on_edge(topo, &vert_to_edges, edge, s.is_simple, u)
                }
                PosSource::Ptex { .. } => eval_pos(i as u32, 0.0, 0.0),
            };
        }
        Ok(build_result(topo, lay, positions, uv_layers))
    }
}
