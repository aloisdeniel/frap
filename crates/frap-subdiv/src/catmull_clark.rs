// SPDX-License-Identifier: GPL-3.0-or-later
//! frap's own Catmull-Clark backend (WP 4.4): what Blender computes through OpenSubdiv for the
//! Subdivision Surface modifier, without OpenSubdiv.
//!
//! The pipeline follows Blender's (`subdiv_converter_mesh.cc` → `topology_refiner_factory.cc`
//! → `evaluator_impl.cc` → `subdiv_mesh.cc`):
//!
//! 1. The base level is the mesh without loose vertices and edges; vertices of loose edges are
//!    infinitely sharp; edge and vertex creases become sharpness `10·c²`; a vertex with two
//!    incident edges adds their smaller sharpness (Blender's converter quirk); `SIMPLE`
//!    subdivision is Catmull-Clark with every edge and vertex infinitely sharp
//!    (`BUGGY_SIMPLE_SCHEME_WORKAROUND`). UV maps become face-varying channels whose values
//!    are the corners Blender's UV vertex map welds (`BKE_mesh_uv_vert_map_create`, winding
//!    aware, 1e-4 limit).
//! 2. The mesh is refined uniformly (`vtr`, `primvar`).
//! 3. With `use_limit_surface` Blender evaluates bicubic patches of an adaptive refinement
//!    isolated to `quality` levels. At level `L ≤ quality` every output point is a vertex of
//!    the refined mesh at level `L`, and the patches place it at that vertex's limit position
//!    (regular B-spline patches interpolate it, Gregory end caps are built from it), so the
//!    output is the limit mask applied to the refined vertex — after refining further until
//!    semi-sharp creases have decayed (at most to `quality`; what is left then is ignored like
//!    OpenSubdiv's end caps ignore it). Two patch-table rules are reproduced at the isolation
//!    level: a smooth boundary corner with one face is a sharp corner patch
//!    (`generateLegacySharpCornerPatches`), so it keeps its isolation-level position, and an
//!    interior valence-2 vertex uses the end caps' `n / (n + 5)` weights (Suzanne's nose).
//!    Without `use_limit_surface` the points are the refined vertices themselves (`SIMPLE`:
//!    bilinear inside level-1 faces, Blender's uniform linear patches).
//!
//!    Past the isolation level (`levels > quality`, or `SIMPLE` with `levels > 1`, whose
//!    level-1 faces around the face point of a non-quad are irregular) Blender evaluates the
//!    points between isolation-level vertices inside Gregory end caps. With the `osd` feature
//!    (on by default, the M4 gate's decision) those cases take their points from the
//!    `opensubdiv-rs` patch table ([`needs_end_caps`]); without it they are the exact limit
//!    here (up to `5e-2` from Blender on a 6-sided cylinder, `SIMPLE` level 3; deviations.md).
//! 4. The output follows Blender's layout ([`crate::layout`]), so indices match (T1).

use smallvec::SmallVec;

use crate::fvar::FVarLevel;
use crate::layout::{self, Coarse, DataSource, Layout, PosSource};
use crate::primvar::{self, FVarChildSlots, V};
use crate::sdc::{self, Rule};
use crate::vtr::{self, Level, Refinement};
use crate::{
    SubdivError, SubdivResult, SubdivSettings, SubdivTopology, Subdivider, VtxBoundary, Weights,
};

/// frap's Catmull-Clark subdivider.
#[derive(Copy, Clone, Debug, Default)]
pub struct CatmullClark;

impl Subdivider for CatmullClark {
    fn name(&self) -> &'static str {
        "frap"
    }

    fn subdivide(
        &self,
        topo: &SubdivTopology<'_>,
        settings: &SubdivSettings,
    ) -> Result<SubdivResult, SubdivError> {
        if settings.level == 0 {
            return Ok(SubdivResult::identity(topo));
        }
        if settings.level > 16 {
            return Err(SubdivError::TooLarge(format!(
                "subdivision level {} is too high",
                settings.level
            )));
        }
        let resolution = (1u32 << settings.level) + 1;
        let lay = layout::compute(topo.coarse(), resolution);
        #[cfg(feature = "osd")]
        if needs_end_caps(settings) {
            return end_caps::subdivide(topo, settings, lay);
        }
        let surf = Surface::new(topo, settings)?;
        Ok(surf.evaluate(topo, settings, lay))
    }
}

/// Whether Blender evaluates some output points inside Gregory end-cap patches between the
/// vertices of its isolation level: the limit surface past the isolation level (`quality`, or
/// level 1 for `SIMPLE`, whose all-sharp level-1 faces around the face point of a non-quad are
/// irregular).
pub fn needs_end_caps(s: &SubdivSettings) -> bool {
    let isolation = if s.is_simple { 1 } else { s.quality.max(1) };
    s.use_limit_surface && s.level > isolation
}

/// The end-cap path (§3.6 `subdiv-osd`, decision log M34S3-1): the points come from the
/// `opensubdiv-rs` patch table, which builds and evaluates Blender's Gregory end caps
/// (`ENDCAP_GREGORY_BASIS`, `useInfSharpPatch`). `opensubdiv-rs` has no
/// `generateLegacySharpCornerPatches`, so at a smooth boundary corner (one face,
/// `boundary_smooth='ALL'`) it caps the isolation-level face at the corner where Blender has a
/// regular sharp-corner patch: the points of that face keep frap's own evaluation.
#[cfg(feature = "osd")]
mod end_caps {
    use super::{Manifold, Surface, base_level};
    use crate::layout::{Layout, PosSource};
    use crate::{SubdivError, SubdivResult, SubdivSettings, SubdivTopology, Subdivider};
    use crate::{VtxBoundary, sdc};

    /// Mesh vertices that Blender's patch table makes legacy sharp corner patches.
    fn smooth_corners(
        topo: &SubdivTopology<'_>,
        s: &SubdivSettings,
    ) -> Result<Vec<bool>, SubdivError> {
        let mut out = vec![false; topo.positions.len()];
        if s.is_simple || s.vtx_boundary != VtxBoundary::EdgeOnly {
            return Ok(out);
        }
        let m = Manifold::new(topo);
        if topo.face_offsets.len() < 2 || m.nverts == 0 {
            return Ok(out);
        }
        let (base, reverse) = base_level(topo, s, &m)?;
        for (bv, &v) in reverse.iter().enumerate() {
            out[v as usize] =
                base.vert_faces[bv].len() == 1 && base.vert_sharpness[bv] < sdc::SHARPNESS_INFINITE;
        }
        Ok(out)
    }

    /// Whether ptex point `(u, v)` lies in the isolation-level face at a smooth corner.
    struct CornerFaces<'a> {
        topo: &'a SubdivTopology<'a>,
        corners: Vec<bool>,
        face_ptex_offset: &'a [u32],
        /// Side of the isolation-level face in ptex units: `2^-quality` for a quad, twice
        /// that for the ptex quads of an n-gon (already one level down).
        quad_side: f32,
    }

    impl CornerFaces<'_> {
        fn contains(&self, ptex: u32, u: f32, v: f32) -> bool {
            let f = self.face_ptex_offset.partition_point(|&o| o <= ptex) - 1;
            let r = self.topo.face_offsets[f] as usize..self.topo.face_offsets[f + 1] as usize;
            let fv = &self.topo.corner_verts[r];
            if fv.len() == 4 {
                let side = self.quad_side;
                [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]
                    .iter()
                    .zip(fv)
                    .any(|(&(cu, cv), &vert)| {
                        self.corners[vert as usize]
                            && (u - cu).abs() <= side
                            && (v - cv).abs() <= side
                    })
            } else {
                let k = (ptex - self.face_ptex_offset[f]) as usize;
                let side = (2.0 * self.quad_side).min(1.0);
                self.corners[fv[k] as usize] && u <= side && v <= side
            }
        }
    }

    pub(super) fn subdivide(
        topo: &SubdivTopology<'_>,
        s: &SubdivSettings,
        lay: Layout,
    ) -> Result<SubdivResult, SubdivError> {
        let mut osd = crate::osd::OpenSubdiv.subdivide(topo, s)?;
        let corners = smooth_corners(topo, s)?;
        if !corners.iter().any(|&c| c) {
            return Ok(osd);
        }
        let cf = CornerFaces {
            topo,
            corners,
            face_ptex_offset: &lay.face_ptex_offset,
            quad_side: 1.0 / (1u32 << s.quality.clamp(1, 16)) as f32,
        };
        let own_vert: Vec<bool> = lay
            .vert_pos
            .iter()
            .map(|p| match *p {
                PosSource::Ptex { ptex, u, v } => cf.contains(ptex, u, v),
                _ => false,
            })
            .collect();
        let own_corner: Vec<bool> = lay
            .corner_ptex
            .iter()
            .map(|&(ptex, u, v)| cf.contains(ptex, u, v))
            .collect();
        drop(cf);
        let surf = Surface::new(topo, s)?;
        let own = surf.evaluate(topo, s, lay);
        for (i, _) in own_vert.iter().enumerate().filter(|(_, o)| **o) {
            osd.positions[i] = own.positions[i];
        }
        for (layer, own_layer) in osd.uv_layers.iter_mut().zip(&own.uv_layers) {
            for (c, _) in own_corner.iter().enumerate().filter(|(_, o)| **o) {
                layer[c] = own_layer[c];
            }
        }
        Ok(osd)
    }
}

/// `subdiv_converter_mesh.cc`'s manifold index maps.
pub(crate) struct Manifold {
    /// Base-level index of each mesh vertex used by a face.
    pub(crate) vert: Vec<Option<u32>>,
    pub(crate) nverts: usize,
    pub(crate) loose_edges: Vec<bool>,
    pub(crate) inf_sharp_verts: Vec<bool>,
}

impl Manifold {
    pub(crate) fn new(topo: &SubdivTopology<'_>) -> Manifold {
        let nv = topo.positions.len();
        let mut used = vec![false; nv];
        let mut edge_used = vec![false; topo.edges.len()];
        for (&v, &e) in topo.corner_verts.iter().zip(topo.corner_edges) {
            used[v as usize] = true;
            if let Some(u) = edge_used.get_mut(e as usize) {
                *u = true;
            }
        }
        let mut vert = vec![None; nv];
        let mut n = 0u32;
        for v in 0..nv {
            if used[v] {
                vert[v] = Some(n);
                n += 1;
            }
        }
        let loose_edges: Vec<bool> = edge_used.iter().map(|u| !u).collect();
        let mut inf = vec![false; nv];
        for (e, &l) in loose_edges.iter().enumerate() {
            if l {
                let ev = topo.edges[e];
                inf[ev[0] as usize] = true;
                inf[ev[1] as usize] = true;
            }
        }
        Manifold {
            vert,
            nverts: n as usize,
            loose_edges,
            inf_sharp_verts: inf,
        }
    }
}

/// `crease_to_sharpness`
pub(crate) fn crease_to_sharpness(c: f32) -> f32 {
    c * c * 10.0
}

/// `cross_poly_v2`
fn cross_poly_v2(uvs: &[[f32; 2]]) -> f32 {
    let n = uvs.len();
    let mut prev = uvs[n - 1];
    let mut cross = 0.0f32;
    for &cur in uvs {
        cross += (prev[0] - cur[0]) * (cur[1] + prev[1]);
        prev = cur;
    }
    cross
}

/// `precalc_uv_layer`: the face-varying value of every corner (`BKE_mesh_uv_vert_map_create`
/// with `STD_UV_CONNECT_LIMIT` and winding) and the number of values.
pub(crate) fn uv_value_indices(topo: &SubdivTopology<'_>, uv: &[[f32; 2]]) -> (Vec<u32>, usize) {
    const LIMIT: f32 = 0.0001;
    let nf = topo.face_offsets.len().saturating_sub(1);
    let mut corner_face = vec![0u32; topo.corner_verts.len()];
    for f in 0..nf {
        for c in topo.face_offsets[f]..topo.face_offsets[f + 1] {
            corner_face[c as usize] = f as u32;
        }
    }
    let face_of = |c: usize| corner_face[c] as usize;
    let winding: Vec<bool> = (0..nf)
        .map(|f| {
            let r = topo.face_offsets[f] as usize..topo.face_offsets[f + 1] as usize;
            cross_poly_v2(&uv[r]) < 0.0
        })
        .collect();
    // Per-vertex corner lists, most recently added first (Blender prepends).
    let nv = topo.positions.len();
    let mut lists: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (c, &v) in topo.corner_verts.iter().enumerate() {
        lists[v as usize].push(c as u32);
    }
    let mut out = vec![0u32; topo.corner_verts.len()];
    let mut count: i64 = -1;
    for list in lists.iter_mut() {
        list.reverse();
        let mut vlist: Vec<u32> = std::mem::take(list);
        // Groups in the final (prepended) order: Blender builds `newvlist` by prepending each
        // group head and then its matches, so the final list is groups in reverse order of
        // their heads, each group's members in reverse order of insertion.
        let mut newvlist: Vec<(u32, bool)> = Vec::new();
        while !vlist.is_empty() {
            let head = vlist.remove(0);
            newvlist.insert(0, (head, false));
            let huv = uv[head as usize];
            let hw = winding[face_of(head as usize)];
            let mut rest = Vec::with_capacity(vlist.len());
            for &c in &vlist {
                let cu = uv[c as usize];
                if (huv[0] - cu[0]).abs() < LIMIT
                    && (huv[1] - cu[1]).abs() < LIMIT
                    && winding[face_of(c as usize)] == hw
                {
                    newvlist.insert(0, (c, false));
                } else {
                    rest.push(c);
                }
            }
            vlist = rest;
            newvlist[0].1 = true;
        }
        for &(c, separate) in &newvlist {
            if separate {
                count += 1;
            }
            out[c as usize] = count as u32;
        }
    }
    (out, (count + 1) as usize)
}

/// One face-varying channel through the levels.
struct Channel {
    levels: Vec<FVarLevel>,
    data: Vec<Vec<V<2>>>,
}

/// The refined surface.
struct Surface {
    levels: Vec<Level>,
    refinements: Vec<Refinement>,
    positions: Vec<Vec<V<3>>>,
    channels: Vec<Channel>,
    /// Final values (limit or refined) at the last level.
    final_positions: Vec<V<3>>,
    final_uvs: Vec<Vec<V<2>>>,
    /// Ptex grids at the last level: per ptex face, `(size, vertices, cells)`.
    grids: Vec<Grid>,
    /// `SIMPLE` without the limit surface past level 1 interpolates bilinearly.
    bilinear: bool,
    manifold: Manifold,
}

#[derive(Clone, Debug, Default)]
struct Grid {
    size: usize,
    verts: Vec<u32>,
    cells: Vec<u32>,
}

impl Grid {
    fn vert(&self, i: usize, j: usize) -> u32 {
        self.verts[j * self.size + i]
    }
    fn cell(&self, i: usize, j: usize) -> u32 {
        self.cells[j * (self.size - 1) + i]
    }
}

/// The edge of face `f` between vertices `a` and `b` (either direction).
fn face_edge_between(lvl: &Level, f: usize, a: u32, b: u32) -> u32 {
    let fv = lvl.face_verts(f);
    let fe = lvl.face_edges(f);
    let n = fv.len();
    for k in 0..n {
        let (x, y) = (fv[k], fv[(k + 1) % n]);
        if (x == a && y == b) || (x == b && y == a) {
            return fe[k];
        }
    }
    lvl.find_edge(a, b).unwrap_or(0)
}

fn local_index(lvl: &Level, f: usize, v: u32) -> usize {
    lvl.face_verts(f).iter().position(|&x| x == v).unwrap_or(0)
}

impl Grid {
    /// The level-1 grid of ptex corner `k` of coarse face `f` (`k == 0` for a quad).
    fn level1(base: &Level, r: &Refinement, f: usize, k: usize) -> Grid {
        let fv = base.face_verts(f);
        let fe = base.face_edges(f);
        let n = fv.len();
        let vc = |v: u32| r.vert_child_vert[v as usize];
        let ec = |e: u32| r.edge_child_vert[e as usize];
        let fcv = r.face_child_vert[f];
        let child = |k: usize| Refinement::face_child_face(base, f, k) as u32;
        if n == 4 {
            Grid {
                size: 3,
                verts: vec![
                    vc(fv[0]),
                    ec(fe[0]),
                    vc(fv[1]),
                    ec(fe[3]),
                    fcv,
                    ec(fe[1]),
                    vc(fv[3]),
                    ec(fe[2]),
                    vc(fv[2]),
                ],
                cells: vec![child(0), child(1), child(3), child(2)],
            }
        } else {
            Grid {
                size: 2,
                verts: vec![vc(fv[k]), ec(fe[k]), ec(fe[(k + n - 1) % n]), fcv],
                cells: vec![child(k)],
            }
        }
    }

    /// The grid one level finer.
    fn refine(&self, parent: &Level, r: &Refinement) -> Grid {
        let g = self.size;
        let ng = 2 * g - 1;
        let mut verts = vec![0u32; ng * ng];
        let mut cells = vec![0u32; (ng - 1) * (ng - 1)];
        for j in 0..g {
            for i in 0..g {
                let a = self.vert(i, j);
                verts[(2 * j) * ng + 2 * i] = r.vert_child_vert[a as usize];
                if i + 1 < g {
                    let b = self.vert(i + 1, j);
                    let f = self.cell(i, if j + 1 < g { j } else { j - 1 }) as usize;
                    let e = face_edge_between(parent, f, a, b);
                    verts[(2 * j) * ng + 2 * i + 1] = r.edge_child_vert[e as usize];
                }
                if j + 1 < g {
                    let b = self.vert(i, j + 1);
                    let f = self.cell(if i + 1 < g { i } else { i - 1 }, j) as usize;
                    let e = face_edge_between(parent, f, a, b);
                    verts[(2 * j + 1) * ng + 2 * i] = r.edge_child_vert[e as usize];
                }
            }
        }
        for j in 0..g - 1 {
            for i in 0..g - 1 {
                let f = self.cell(i, j) as usize;
                verts[(2 * j + 1) * ng + 2 * i + 1] = r.face_child_vert[f];
                for b in 0..2 {
                    for a in 0..2 {
                        let v = self.vert(i + a, j + b);
                        let k = local_index(parent, f, v);
                        cells[(2 * j + b) * (ng - 1) + 2 * i + a] =
                            Refinement::face_child_face(parent, f, k) as u32;
                    }
                }
            }
        }
        Grid {
            size: ng,
            verts,
            cells,
        }
    }
}

fn has_semi_sharp(lvl: &Level) -> bool {
    lvl.edge_tags.iter().any(|t| t.semi_sharp) || lvl.vert_tags.iter().any(|t| t.semi_sharp)
}

/// Whether some smooth vertex has two faces: its patch point (`n / (n + 5)` weights) is not
/// its limit, so it is only right at the isolation level.
fn has_valence2_smooth(lvl: &Level) -> bool {
    (0..lvl.nverts).any(|v| {
        let t = lvl.vert_tags[v];
        matches!(t.rule, Rule::Smooth | Rule::Dart) && !t.boundary && lvl.vert_faces[v].len() == 2
    })
}

/// The base level as Blender's converter and `TopologyRefinerFactory` build it, before the
/// boundary tags (`assignComponentTags`): faces over the vertices used by faces, edge
/// sharpness from `crease_edge`, vertex sharpness from `crease_vert`, loose-edge vertices and
/// `SIMPLE` infinitely sharp, the two-edge vertex rule. Also returns the mesh vertex of every
/// base vertex.
pub(crate) fn base_level(
    topo: &SubdivTopology<'_>,
    s: &SubdivSettings,
    m: &Manifold,
) -> Result<(Level, Vec<u32>), SubdivError> {
    let face_verts: Vec<u32> = topo
        .corner_verts
        .iter()
        .map(|&v| m.vert[v as usize].expect("face vertex"))
        .collect();
    let mut base = Level::from_faces(m.nverts, topo.face_offsets.to_vec(), face_verts);
    for (e, ev) in topo.edges.iter().enumerate() {
        if m.loose_edges[e] {
            continue;
        }
        let sharp = if s.is_simple {
            sdc::SHARPNESS_INFINITE
        } else if let (true, Some(c)) = (s.use_creases, topo.edge_creases) {
            crease_to_sharpness(c[e])
        } else {
            0.0
        };
        if sharp < 1e-6 {
            continue;
        }
        let (Some(a), Some(b)) = (m.vert[ev[0] as usize], m.vert[ev[1] as usize]) else {
            continue;
        };
        match base.find_edge(a, b) {
            Some(be) => base.edge_sharpness[be as usize] = sharp,
            None => {
                return Err(SubdivError::Topology(
                    "OpenSubdiv Error: failed to find reconstructed edge".into(),
                ));
            }
        }
    }
    let mut reverse = vec![0u32; m.nverts];
    for (v, mv) in m.vert.iter().enumerate() {
        if let Some(mv) = mv {
            reverse[*mv as usize] = v as u32;
        }
    }
    for (bv, &v) in reverse.iter().enumerate() {
        let v = v as usize;
        if s.is_simple || m.inf_sharp_verts[v] {
            base.vert_sharpness[bv] = sdc::SHARPNESS_INFINITE;
            continue;
        }
        let mut sharp = match (s.use_creases, topo.vert_creases) {
            (true, Some(c)) => crease_to_sharpness(c[v]),
            _ => 0.0,
        };
        let ve = &base.vert_edges[bv];
        if ve.len() == 2 {
            let s0 = base.edge_sharpness[ve[0] as usize];
            let s1 = base.edge_sharpness[ve[1] as usize];
            sharp += s0.min(s1);
            sharp = sharp.min(10.0);
        }
        base.vert_sharpness[bv] = sharp;
    }
    Ok((base, reverse))
}

impl Surface {
    fn new(topo: &SubdivTopology<'_>, s: &SubdivSettings) -> Result<Surface, SubdivError> {
        let m = Manifold::new(topo);
        let nf = topo.face_offsets.len().saturating_sub(1);
        let mut surf = Surface {
            levels: Vec::new(),
            refinements: Vec::new(),
            positions: Vec::new(),
            channels: Vec::new(),
            final_positions: Vec::new(),
            final_uvs: Vec::new(),
            grids: Vec::new(),
            bilinear: false,
            manifold: Manifold {
                vert: Vec::new(),
                nverts: 0,
                loose_edges: Vec::new(),
                inf_sharp_verts: Vec::new(),
            },
        };
        if nf == 0 || m.nverts == 0 {
            surf.manifold = m;
            return Ok(surf);
        }
        let (mut base, reverse) = base_level(topo, s, &m)?;
        base.prepare_base_tags(s.vtx_boundary);
        let geom_corners_sharp = s.vtx_boundary == VtxBoundary::EdgeAndCorner;
        let p0: Vec<V<3>> = reverse
            .iter()
            .map(|&v| {
                let p = topo.positions[v as usize];
                [p[0] as f64, p[1] as f64, p[2] as f64]
            })
            .collect();
        for uv in &topo.uv_layers {
            let (idx, n) = uv_value_indices(topo, uv);
            let mut data = vec![[0.0f64; 2]; n];
            for (c, &i) in idx.iter().enumerate() {
                data[i as usize] = [uv[c][0] as f64, uv[c][1] as f64];
            }
            let fl = FVarLevel::complete(&base, idx, n, s.fvar_linear, geom_corners_sharp);
            surf.channels.push(Channel {
                levels: vec![fl],
                data: vec![data],
            });
        }
        surf.levels.push(base);
        surf.positions.push(p0);
        surf.manifold = m;

        let level = s.level as usize;
        let patch_level = if s.is_simple {
            1
        } else if s.use_limit_surface {
            s.quality.max(1) as usize
        } else {
            level
        };
        let target = if s.use_limit_surface {
            level
        } else {
            level.min(patch_level)
        };
        surf.bilinear = !s.use_limit_surface && level > patch_level;
        for _ in 0..target {
            surf.refine_once(s);
            if s.use_limit_surface && level > patch_level && surf.levels.len() - 1 == patch_level {
                surf.sharpen_smooth_corners();
            }
        }
        if s.use_limit_surface {
            while surf.levels.len() - 1 < patch_level && {
                let l = surf.levels.last().expect("level");
                has_semi_sharp(l) || has_valence2_smooth(l)
            } {
                surf.refine_once(s);
            }
        }
        let corner_steps = patch_level.saturating_sub(surf.levels.len() - 1);
        surf.finish(topo, s.use_limit_surface, corner_steps);
        Ok(surf)
    }

    /// Past the isolation level, the face at a smooth boundary corner is Blender's regular
    /// sharp-corner patch (`generateLegacySharpCornerPatches`): a B-spline patch whose corner
    /// interpolates the isolation-level vertex, i.e. the limit with that vertex infinitely
    /// sharp from here on. Only the corner face's surface changes.
    fn sharpen_smooth_corners(&mut self) {
        let lvl = self.levels.last_mut().expect("level");
        for v in 0..lvl.nverts {
            let t = lvl.vert_tags[v];
            if t.rule == Rule::Crease
                && t.boundary
                && !t.inf_sharp
                && !t.non_manifold
                && lvl.vert_faces[v].len() == 1
            {
                let t = &mut lvl.vert_tags[v];
                t.inf_sharp = true;
                t.rule = Rule::Corner;
                lvl.vert_sharpness[v] = sdc::SHARPNESS_INFINITE;
            }
        }
    }

    fn refine_once(&mut self, s: &SubdivSettings) {
        let parent = self.levels.last().expect("level");
        let (child, r) = vtr::refine(parent);
        let pos = primvar::interp_vertex(parent, &child, &r, self.positions.last().expect("p"));
        let geom_corners_sharp = s.vtx_boundary == VtxBoundary::EdgeAndCorner;
        for ch in &mut self.channels {
            let pf = ch.levels.last().expect("fvar");
            let (fvals, slots): (Vec<u32>, FVarChildSlots) =
                primvar::refine_fvar_topology(parent, &r, pf);
            let cf = FVarLevel::complete(
                &child,
                fvals,
                slots.nslots,
                s.fvar_linear,
                geom_corners_sharp,
            );
            let data = primvar::interp_fvar(
                parent,
                &child,
                &r,
                pf,
                &cf,
                &slots,
                ch.data.last().expect("data"),
            );
            ch.levels.push(cf);
            ch.data.push(data);
        }
        self.positions.push(pos);
        self.levels.push(child);
        self.refinements.push(r);
    }

    /// Final values and ptex grids at the last level.
    fn finish(&mut self, topo: &SubdivTopology<'_>, limit: bool, corner_steps: usize) {
        let last = self.levels.len() - 1;
        let lvl = &self.levels[last];
        let src = &self.positions[last];
        let inf_only = has_semi_sharp(lvl);
        if limit {
            let mut dst = src.clone();
            primvar::limit_vertex(lvl, src, 0..lvl.nverts, inf_only, corner_steps, &mut dst);
            self.final_positions = dst;
        } else {
            self.final_positions = src.clone();
        }
        for ch in &self.channels {
            let fl = &ch.levels[last];
            let data = &ch.data[last];
            if limit {
                let mut dst = data.clone();
                primvar::limit_fvar(
                    lvl,
                    fl,
                    data,
                    0..lvl.nverts,
                    inf_only,
                    corner_steps,
                    &mut dst,
                );
                self.final_uvs.push(dst);
            } else {
                self.final_uvs.push(data.clone());
            }
        }
        if last == 0 {
            return;
        }
        let base = &self.levels[0];
        let r0 = &self.refinements[0];
        let nf = topo.face_offsets.len() - 1;
        for f in 0..nf {
            let n = base.face_verts(f).len();
            let ks = if n == 4 { 1 } else { n };
            for k in 0..ks {
                let mut g = Grid::level1(base, r0, f, k);
                for l in 1..last {
                    g = g.refine(&self.levels[l], &self.refinements[l]);
                }
                self.grids.push(g);
            }
        }
    }

    /// Grid coordinates of a ptex point at the output resolution: `(grid, x, y)` with the
    /// numerator over `out_res - 1`.
    fn ptex_point(
        &self,
        lay: &Layout,
        ptex: u32,
        u: f32,
        v: f32,
        quad: bool,
    ) -> (usize, usize, usize) {
        let pr = if quad {
            lay.resolution as usize
        } else {
            (lay.resolution as usize >> 1) + 1
        };
        let d = (pr - 1) as f32;
        (
            ptex as usize,
            (u * d).round() as usize,
            (v * d).round() as usize,
        )
    }

    /// Evaluates a grid point `(x, y)` over `den` of `grid`: `(vertex, None)` when it is a
    /// vertex of the last level, or a bilinear blend of a cell's corners.
    fn locate(&self, grid: &Grid, x: usize, y: usize, den: usize) -> Loc {
        let gd = grid.size - 1;
        if (x * gd).is_multiple_of(den) && (y * gd).is_multiple_of(den) {
            return Loc::Vert(grid.vert(x * gd / den, y * gd / den));
        }
        let fx = x as f64 * gd as f64 / den as f64;
        let fy = y as f64 * gd as f64 / den as f64;
        let ci = (fx.floor() as usize).min(gd - 1);
        let cj = (fy.floor() as usize).min(gd - 1);
        Loc::Bilinear {
            cell: (ci, cj),
            s: fx - ci as f64,
            t: fy - cj as f64,
        }
    }

    fn evaluate(&self, topo: &SubdivTopology<'_>, s: &SubdivSettings, lay: Layout) -> SubdivResult {
        let nv = lay.vert_pos.len();
        let quad_of_ptex: Vec<bool> = {
            let mut q = Vec::new();
            for f in 0..topo.face_offsets.len().saturating_sub(1) {
                let n = (topo.face_offsets[f + 1] - topo.face_offsets[f]) as usize;
                if n == 4 {
                    q.push(true);
                } else {
                    q.extend(std::iter::repeat_n(false, n));
                }
            }
            q
        };
        let last = self.levels.len().saturating_sub(1);
        let mut positions = vec![[0.0f32; 3]; nv];
        let vert_to_edges = loose_edge_vert_map(topo, &self.manifold);
        for (i, src) in lay.vert_pos.iter().enumerate() {
            positions[i] = match *src {
                PosSource::Coarse(v) => topo.positions[v as usize],
                PosSource::LooseEdge { edge, u } => {
                    interpolate_position_on_edge(topo, &vert_to_edges, edge, s.is_simple, u)
                }
                PosSource::Ptex { ptex, u, v } => {
                    let quad = quad_of_ptex[ptex as usize];
                    let (g, x, y) = self.ptex_point(&lay, ptex, u, v, quad);
                    let grid = &self.grids[g];
                    let den = if quad {
                        lay.resolution as usize - 1
                    } else {
                        lay.resolution as usize >> 1
                    };
                    let p = match self.locate(grid, x, y, den) {
                        Loc::Vert(vx) => self.final_positions[vx as usize],
                        Loc::Bilinear { cell, s, t } => {
                            let c = |a: usize, b: usize| {
                                self.final_positions[grid.vert(cell.0 + a, cell.1 + b) as usize]
                            };
                            bilinear(c(0, 0), c(1, 0), c(1, 1), c(0, 1), s, t)
                        }
                    };
                    let _ = last;
                    [p[0] as f32, p[1] as f32, p[2] as f32]
                }
            };
        }
        // UVs per corner, through the cell of the corner's face.
        let nl = lay.corner_verts.len();
        let mut uv_layers = Vec::with_capacity(topo.uv_layers.len());
        for (ci, ch) in self.channels.iter().enumerate() {
            let fl = &ch.levels[last];
            let vals = &self.final_uvs[ci];
            let lvl = &self.levels[last];
            let mut out = vec![[0.0f32; 2]; nl];
            for face in 0..nl / 4 {
                let (ptex, u0, v0) = lay.corner_ptex[4 * face];
                let quad = quad_of_ptex[ptex as usize];
                let (g, cx, cy) = self.ptex_point(&lay, ptex, u0, v0, quad);
                let grid = &self.grids[g];
                let den = if quad {
                    lay.resolution as usize - 1
                } else {
                    lay.resolution as usize >> 1
                };
                let gd = grid.size - 1;
                for (k, &(a, b)) in [(0usize, 0usize), (1, 0), (1, 1), (0, 1)]
                    .iter()
                    .enumerate()
                {
                    let (x, y) = (cx + a, cy + b);
                    let value = if gd >= den {
                        let sc = gd / den;
                        let vx = grid.vert(x * sc, y * sc);
                        let cell_i = (cx * sc + a * (sc - 1)).min(gd - 1);
                        let cell_j = (cy * sc + b * (sc - 1)).min(gd - 1);
                        let f = grid.cell(cell_i, cell_j) as usize;
                        let slot = fl.face_values(lvl, f)[local_index(lvl, f, vx)];
                        vals[slot as usize]
                    } else {
                        // Bilinear inside a coarser cell (`SIMPLE` without limit surface).
                        let fx = x as f64 * gd as f64 / den as f64;
                        let fy = y as f64 * gd as f64 / den as f64;
                        let ccx = (cx as f64 * gd as f64 / den as f64).floor() as usize;
                        let ccy = (cy as f64 * gd as f64 / den as f64).floor() as usize;
                        let (ci2, cj2) = (ccx.min(gd - 1), ccy.min(gd - 1));
                        let f = grid.cell(ci2, cj2) as usize;
                        let fv = fl.face_values(lvl, f);
                        let corner = |a: usize, b: usize| {
                            let vx = grid.vert(ci2 + a, cj2 + b);
                            vals[fv[local_index(lvl, f, vx)] as usize]
                        };
                        bilinear(
                            corner(0, 0),
                            corner(1, 0),
                            corner(1, 1),
                            corner(0, 1),
                            fx - ci2 as f64,
                            fy - cj2 as f64,
                        )
                    };
                    out[4 * face + k] = [value[0] as f32, value[1] as f32];
                }
            }
            uv_layers.push(out);
        }
        if self.channels.is_empty() {
            // Meshes without faces keep no UV data; UV maps of faceless meshes are empty.
            for _ in &topo.uv_layers {
                uv_layers.push(vec![[0.0; 2]; nl]);
            }
        }
        build_result(topo, lay, positions, uv_layers)
    }
}

enum Loc {
    Vert(u32),
    Bilinear {
        cell: (usize, usize),
        s: f64,
        t: f64,
    },
}

fn bilinear<const N: usize>(a: V<N>, b: V<N>, c: V<N>, d: V<N>, s: f64, t: f64) -> V<N> {
    let mut o = [0.0; N];
    for k in 0..N {
        o[k] = (1.0 - s) * (1.0 - t) * a[k]
            + s * (1.0 - t) * b[k]
            + s * t * c[k]
            + (1.0 - s) * t * d[k];
    }
    o
}

/// `mesh::build_vert_to_edge_map` (only needed with loose edges).
pub(crate) fn loose_edge_vert_map(
    topo: &SubdivTopology<'_>,
    m: &Manifold,
) -> Vec<SmallVec<[u32; 4]>> {
    if !m.loose_edges.iter().any(|&l| l) {
        return Vec::new();
    }
    let mut map = vec![SmallVec::new(); topo.positions.len()];
    for (e, ev) in topo.edges.iter().enumerate() {
        map[ev[0] as usize].push(e as u32);
        map[ev[1] as usize].push(e as u32);
    }
    map
}

/// `key_curve_position_weights(t, KEY_BSPLINE)`
fn bspline_weights(t: f32) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        -0.16666666 * t3 + 0.5 * t2 - 0.5 * t + 0.16666666,
        // `0.66666666f` in Blender (the same `f32` as 0.666_666_7).
        0.5 * t3 - t2 + 0.666_666_7,
        -0.5 * t3 + 0.5 * t2 + 0.5 * t + 0.16666666,
        0.16666666 * t3,
    ]
}

/// `bke::subdiv::mesh_interpolate_position_on_edge`
pub(crate) fn interpolate_position_on_edge(
    topo: &SubdivTopology<'_>,
    vert_to_edges: &[SmallVec<[u32; 4]>],
    e: u32,
    is_simple: bool,
    u: f32,
) -> [f32; 3] {
    let ev = topo.edges[e as usize];
    let p = |v: u32| topo.positions[v as usize];
    let lerp = |a: [f32; 3], b: [f32; 3], t: f32| {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    };
    if is_simple {
        return lerp(p(ev[0]), p(ev[1]), u);
    }
    let neighbor = |v: u32| -> Option<[u32; 2]> {
        let n = &vert_to_edges[v as usize];
        if n.len() != 2 {
            return None;
        }
        let o = if n[0] == e { n[1] } else { n[0] };
        Some(topo.edges[o as usize])
    };
    let other = |edge: [u32; 2], v: u32| if edge[0] == v { edge[1] } else { edge[0] };
    let p1 = p(ev[0]);
    let p2 = p(ev[1]);
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] * 2.0 - b[0], a[1] * 2.0 - b[1], a[2] * 2.0 - b[2]];
    let p0 = match neighbor(ev[0]) {
        Some(o) => p(other(o, ev[0])),
        None => sub(p1, p2),
    };
    let p3 = match neighbor(ev[1]) {
        Some(o) => p(other(o, ev[1])),
        None => sub(p2, p1),
    };
    let w = bspline_weights(u);
    let mut o = [0.0f32; 3];
    for k in 0..3 {
        o[k] = w[0] * p0[k] + w[1] * p1[k] + w[2] * p2[k] + w[3] * p3[k];
    }
    o
}

/// Linear weights over coarse vertices (or corners) of a data source.
fn flatten(topo: &SubdivTopology<'_>, src: DataSource, corners: bool) -> Weights {
    let mut w: Weights = SmallVec::new();
    let mut add = |i: u32, x: f32| {
        if x == 0.0 {
            return;
        }
        if let Some(e) = w.iter_mut().find(|e| e.0 == i) {
            e.1 += x;
        } else {
            w.push((i, x));
        }
    };
    match src {
        DataSource::Copy(i) => add(i, 1.0),
        DataSource::Keep => {}
        DataSource::Edge { edge, u } => {
            let ev = topo.edges[edge as usize];
            add(ev[0], 1.0 - u);
            add(ev[1], u);
        }
        DataSource::Ptex { face, corner, u, v } => {
            let r = topo.face_offsets[face as usize] as usize
                ..topo.face_offsets[face as usize + 1] as usize;
            let elem = |c: usize| -> u32 {
                if corners {
                    c as u32
                } else {
                    topo.corner_verts[c]
                }
            };
            let q = [(1.0 - u) * (1.0 - v), u * (1.0 - v), u * v, (1.0 - u) * v];
            let n = r.len();
            if n == 4 {
                for (k, &w) in q.iter().enumerate() {
                    add(elem(r.start + k), w);
                }
            } else {
                let k = corner as usize;
                let c0 = r.start + k;
                let c1 = r.start + (k + 1) % n;
                let c3 = r.start + (k + n - 1) % n;
                add(elem(c0), q[0] + 0.5 * q[1] + 0.5 * q[3]);
                add(elem(c1), 0.5 * q[1]);
                add(elem(c3), 0.5 * q[3]);
                for c in r {
                    add(elem(c), q[2] / n as f32);
                }
            }
        }
    }
    w
}

pub(crate) fn build_result(
    topo: &SubdivTopology<'_>,
    lay: Layout,
    positions: Vec<[f32; 3]>,
    uv_layers: Vec<Vec<[f32; 2]>>,
) -> SubdivResult {
    // Data sources that were never set (`Keep`) copy their coarse vertex: loose-edge ends.
    let mut vert_data = lay.vert_data;
    for (i, d) in vert_data.iter_mut().enumerate() {
        if *d == DataSource::Keep {
            *d = DataSource::Copy(i as u32);
        }
    }
    let vert_weights = vert_data.iter().map(|&d| flatten(topo, d, false)).collect();
    let corner_weights = lay
        .corner_data
        .iter()
        .map(|&d| flatten(topo, d, true))
        .collect();
    let nfaces = lay.face_origin.len();
    let edge_creases = topo.edge_creases.map(|c| {
        lay.edge_origin
            .iter()
            .map(|o| o.map_or(0.0, |e| c[e as usize]))
            .collect()
    });
    SubdivResult {
        positions,
        edges: lay.edges,
        face_offsets: (0..=nfaces as u32).map(|f| 4 * f).collect(),
        corner_verts: lay.corner_verts,
        corner_edges: lay.corner_edges,
        uv_layers,
        vert_weights,
        edge_origin: lay.edge_origin,
        face_origin: lay.face_origin,
        corner_weights,
        edge_creases,
        vert_sources: vert_data,
        corner_sources: lay.corner_data,
    }
}

impl<'a> SubdivTopology<'a> {
    /// The coarse mesh for [`layout::compute`].
    pub fn coarse(&self) -> Coarse<'a> {
        Coarse {
            nverts: self.positions.len(),
            edges: self.edges,
            face_offsets: self.face_offsets,
            corner_verts: self.corner_verts,
            corner_edges: self.corner_edges,
        }
    }
}
