// SPDX-License-Identifier: GPL-3.0-or-later
//! `Far::PatchTableFactory` (`PatchTableBuilder`) as Blender configures it
//! (`ENDCAP_GREGORY_BASIS`, `useInfSharpPatch`, `generateFVarTables`, no legacy linear
//! face-varying patches, shared end-cap points), reduced to what face-varying evaluation
//! reads: every patch's parameterisation (for the patch map) and, per face-varying channel,
//! its patch parameterisation, control values and the local-point stencils of its Gregory end
//! caps. Uniform refinement builds bilinear patches of the last level (`BuildUniformPolygons`).
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/patchTableFactory.cpp` (Pixar, TOST /
//! Apache-2.0 modified).

// Parallel per-component arrays are indexed by component, as in OpenSubdiv.
#![allow(clippy::needless_range_loop)]

use super::gregory::SparseMatrix;
use super::level::VSpan;
use super::patch_builder::{PatchBuilder, PatchParam, ptex_indices};
use super::refiner::TopologyRefiner;
use super::sdc::FVarLinear;
use super::stencil::StencilTable;
use super::tags::ETag;
use super::{INDEX_INVALID, Index};

/// The basis of one face-varying patch.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PatchType {
    /// `PatchDescriptor::QUADS` (bilinear, 4 points).
    Quads,
    /// `PatchDescriptor::REGULAR` (bicubic B-spline, 16 points).
    Regular,
    /// `PatchDescriptor::GREGORY_BASIS` (20 points).
    GregoryBasis,
}

/// One face-varying patch.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FvarPatch {
    pub param: PatchParam,
    pub kind: PatchType,
    pub points: Vec<Index>,
}

/// The patch table of one refiner.
#[derive(Clone, Debug, Default)]
pub(crate) struct PatchTable {
    /// Vertex patch parameterisations, in table order (regular patches, then irregular).
    pub params: Vec<PatchParam>,
    /// Per channel, per patch (table order).
    pub fvar: Vec<Vec<FvarPatch>>,
    /// Per channel: the local-point stencils (`GetLocalPointFaceVaryingStencilTable`).
    pub local: Vec<StencilTable>,
    /// Uniform tables index the refined values of the last level only.
    pub uniform: bool,
}

#[derive(Clone, Debug, Default)]
struct PatchInfo {
    is_regular: bool,
    reg_boundary_mask: i32,
    corner_spans: [VSpan; 4],
    param_boundary_mask: i32,
    matrix: SparseMatrix,
}

const SHARE_CORNER: i16 = 0x10;
const SHARE_EDGE_BEGIN: i16 = 0x20;
const SHARE_EDGE_END: i16 = 0x40;
const SHARE_INDEX_MASK: i16 = 0x0f;
/// `gregoryQuadBits` (`GetShareBitsPerType(GREGORY_BASIS)`).
const GREGORY_QUAD_BITS: [i16; 20] = [
    0x10, 0x20, 0x43, 0, 0, 0x11, 0x21, 0x40, 0, 0, 0x12, 0x22, 0x41, 0, 0, 0x13, 0x23, 0x42, 0, 0,
];

/// `LocalPointHelper` for one face-varying channel (`shareLocalPoints`, float stencils).
struct LocalPointHelper {
    channel: usize,
    num_local_points: i32,
    local_point_offset: i32,
    shared_corner: Vec<Vec<Index>>,
    shared_edge: Vec<Vec<Index>>,
    table: StencilTable,
}

impl LocalPointHelper {
    fn new(refiner: &TopologyRefiner, channel: usize) -> LocalPointHelper {
        let total: i32 = refiner
            .levels
            .iter()
            .map(|l| l.num_fvar_values(channel))
            .sum();
        LocalPointHelper {
            channel,
            num_local_points: 0,
            local_point_offset: total,
            shared_corner: vec![Vec::new(); refiner.levels.len()],
            shared_edge: vec![Vec::new(); refiner.levels.len()],
            table: StencilTable::default(),
        }
    }

    /// `AppendLocalPatchPoints` (shared Gregory points, no reuse of source points).
    #[allow(clippy::too_many_arguments)]
    fn append_local_patch_points(
        &mut self,
        refiner: &TopologyRefiner,
        li: usize,
        f: Index,
        m: &SparseMatrix,
        source: &[Index],
        source_offset: i32,
        out: &mut Vec<Index>,
    ) {
        let level = &refiner.levels[li];
        let first = self.local_point_offset + self.num_local_points;
        let mut next = first;
        let corners = level.face_fvar_values(f, self.channel);
        let fe = level.face_edges(f);
        let fv = level.face_vertices(f);
        let mut et = [ETag::default(); 4];
        level.face_etags(f, Some(self.channel), &mut et);
        let mut reversed = [false; 4];
        let mut boundary = [false; 4];
        for i in 0..fe.len() {
            reversed[i] = level.edge_vertices(fe[i])[0] != fv[i];
            boundary[i] = et[i].boundary();
        }
        out.clear();
        for i in 0..m.num_rows {
            let mut point = next;
            let bits = GREGORY_QUAD_BITS[i];
            if bits != 0 {
                let index = (bits & SHARE_INDEX_MASK) as usize;
                if bits & SHARE_CORNER != 0 {
                    if self.shared_corner[li].is_empty() {
                        self.shared_corner[li] =
                            vec![INDEX_INVALID; level.num_fvar_values(self.channel) as usize];
                    }
                    let slot = &mut self.shared_corner[li][corners[index] as usize];
                    if *slot == INDEX_INVALID {
                        *slot = next;
                    }
                    point = *slot;
                } else if !boundary[index] {
                    let end = ((bits & SHARE_EDGE_END) > 0) != reversed[index];
                    if self.shared_edge[li].is_empty() {
                        self.shared_edge[li] = vec![INDEX_INVALID; 2 * level.num_edges() as usize];
                    }
                    let slot = &mut self.shared_edge[li][2 * fe[index] as usize + end as usize];
                    if *slot == INDEX_INVALID {
                        *slot = next;
                    }
                    point = *slot;
                }
                let _ = SHARE_EDGE_BEGIN;
            }
            if point == next {
                // appendLocalPointStencil
                let cols = m.row_columns(i);
                let ws = m.row_elements(i);
                self.table.sizes.push(cols.len() as i32);
                for k in 0..cols.len() {
                    self.table.weights.push(ws[k]);
                    self.table
                        .indices
                        .push(source[cols[k] as usize] + source_offset);
                }
                next += 1;
            }
            out.push(point);
        }
        self.num_local_points += next - first;
    }
}

/// `PatchTableFactory::Create`
pub(crate) fn create(refiner: &TopologyRefiner) -> PatchTable {
    if refiner.is_uniform {
        build_uniform(refiner)
    } else {
        build_adaptive(refiner)
    }
}

/// `BuildUniformPolygons` (the last level's faces as bilinear patches).
fn build_uniform(refiner: &TopologyRefiner) -> PatchTable {
    let pb = PatchBuilder { refiner };
    let ptex = ptex_indices(&refiner.levels[0]);
    let max = refiner.max_level();
    let level = &refiner.levels[max];
    let nch = level.num_fvar_channels();
    let mut t = PatchTable {
        fvar: vec![Vec::new(); nch],
        local: vec![StencilTable::default(); nch],
        uniform: true,
        ..PatchTable::default()
    };
    for f in 0..level.num_faces() {
        if refiner.has_holes && level.is_face_hole(f) {
            continue;
        }
        let param = pb.compute_patch_param(max, f, &ptex, true, 0, false);
        t.params.push(param);
        for c in 0..nch {
            t.fvar[c].push(FvarPatch {
                param,
                kind: PatchType::Quads,
                points: level.face_fvar_values(f, c).to_vec(),
            });
        }
    }
    t
}

fn build_adaptive(refiner: &TopologyRefiner) -> PatchTable {
    let pb = PatchBuilder { refiner };
    let ptex = ptex_indices(&refiner.levels[0]);
    let nch = refiner.levels[0].num_fvar_channels();

    // identifyPatches
    let mut level_fvar_offsets: Vec<Vec<i32>> = vec![vec![0]; nch];
    for l in &refiner.levels {
        for c in 0..nch {
            let last = *level_fvar_offsets[c].last().unwrap_or(&0);
            level_fvar_offsets[c].push(last + l.num_fvar_values(c));
        }
    }
    let mut patches: Vec<(usize, Index)> = Vec::new();
    for (li, l) in refiner.levels.iter().enumerate() {
        for f in 0..l.num_faces() {
            if pb.is_face_a_patch(li, f) && pb.is_face_a_leaf(li, f) {
                patches.push((li, f));
            }
        }
    }

    // populatePatches
    let mut helpers: Vec<LocalPointHelper> = (0..nch)
        .map(|c| LocalPointHelper::new(refiner, c))
        .collect();
    let mut regular: Vec<(PatchParam, Vec<FvarPatch>)> = Vec::new();
    let mut irregular: Vec<(PatchParam, Vec<FvarPatch>)> = Vec::new();
    let mut points = Vec::new();
    for &(li, f) in &patches {
        frap_math::budget::check();
        let info = identify_patch_topology(&pb, li, f, None);
        let param = pb.compute_patch_param(
            li,
            f,
            &ptex,
            info.is_regular,
            info.param_boundary_mask,
            true,
        );
        let mut fv_patches = Vec::with_capacity(nch);
        for c in 0..nch {
            let offset = level_fvar_offsets[c][li];
            let level = &refiner.levels[li];
            if level.fvar_level(c).options.fvar_linear == FVarLinear::All {
                let pts = level
                    .face_fvar_values(f, c)
                    .iter()
                    .map(|&v| v + offset)
                    .collect();
                fv_patches.push(FvarPatch {
                    param,
                    kind: PatchType::Quads,
                    points: pts,
                });
                continue;
            }
            let matches = level.does_face_fvar_topology_match(f, c);
            let own;
            let fi = if matches {
                &info
            } else {
                own = identify_patch_topology(&pb, li, f, Some(c));
                &own
            };
            let kind = if fi.is_regular {
                let p = pb.regular_patch_points(li, f, fi.reg_boundary_mask, Some(c));
                points.clear();
                points.extend(p.iter().map(|&v| v + offset));
                PatchType::Regular
            } else {
                let source = pb.irregular_patch_source_points(li, f, &fi.corner_spans, Some(c));
                helpers[c].append_local_patch_points(
                    refiner,
                    li,
                    f,
                    &fi.matrix,
                    &source,
                    offset,
                    &mut points,
                );
                PatchType::GregoryBasis
            };
            let mut fparam = param;
            fparam.boundary = fi.param_boundary_mask;
            fparam.regular = fi.is_regular;
            fv_patches.push(FvarPatch {
                param: fparam,
                kind,
                points: points.clone(),
            });
        }
        if info.is_regular {
            regular.push((param, fv_patches));
        } else {
            irregular.push((param, fv_patches));
        }
    }
    let mut t = PatchTable {
        fvar: vec![Vec::new(); nch],
        ..PatchTable::default()
    };
    for (param, fv) in regular.into_iter().chain(irregular) {
        t.params.push(param);
        for (c, p) in fv.into_iter().enumerate() {
            t.fvar[c].push(p);
        }
    }
    for h in helpers {
        let mut table = h.table;
        table.generate_offsets();
        t.local.push(table);
    }
    t
}

/// `identifyPatchTopology` (float matrices; no single-crease patches).
fn identify_patch_topology(
    pb: &PatchBuilder<'_>,
    li: usize,
    f: Index,
    fvc: Option<usize>,
) -> PatchInfo {
    let mut info = PatchInfo {
        is_regular: pb.is_patch_regular(li, f, fvc),
        ..PatchInfo::default()
    };
    if info.is_regular {
        info.reg_boundary_mask = pb.regular_patch_boundary_mask(li, f, fvc);
        info.param_boundary_mask = info.reg_boundary_mask;
    } else {
        info.corner_spans = pb.irregular_patch_corner_spans(li, f, fvc);
        info.matrix = pb.irregular_patch_conversion_matrix(li, f, &info.corner_spans);
        info.param_boundary_mask = 0;
    }
    info
}
