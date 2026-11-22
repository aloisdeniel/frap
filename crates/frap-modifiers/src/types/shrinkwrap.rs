// SPDX-License-Identifier: GPL-3.0-or-later
//! SHRINKWRAP (`ShrinkwrapModifier`): moves the vertices onto the evaluated surface of
//! `target`: to the nearest surface point (`NEAREST_SURFACEPOINT`), the nearest vertex
//! (`NEAREST_VERTEX`), the point whose smooth normal passes through the vertex
//! (`TARGET_PROJECT`), or along the vertex normal or an axis (`PROJECT`, ray casts in the
//! positive and/or negative direction, face culling, `auxiliary_target`, `project_limit`,
//! `subsurf_levels`), then snaps by `wrap_mode` (`ON_SURFACE`, `INSIDE`, `OUTSIDE`,
//! `OUTSIDE_SURFACE`, `ABOVE_SURFACE`) at `offset`, weighted by `vertex_group`.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_shrinkwrap.cc and
//! blender/source/blender/blenkernel/intern/shrinkwrap.cc (`shrinkwrapModifier_deform`,
//! `shrinkwrap_calc_nearest_vertex`, `shrinkwrap_calc_normal_projection`,
//! `shrinkwrap_calc_nearest_surface_point`, `shrinkwrap_calc_subdivided_positions`),
//! Blender 5.2.4, GPL-2.0-or-later; the target tree and its queries in [`tree`].
//!
//! Blender runs the vertices in `threading::parallel_for` chunks of 512 and seeds each nearest
//! search with the previous vertex's hit (a nearer hit must be strictly nearer, so a tie keeps
//! the previous one); frap runs the same chunks (halved until at most 512 vertices, TBB's
//! `blocked_range` split) one after another, which gives the same results.

mod geom;
pub(crate) mod tree;

use std::sync::Arc;

use frap_geom::bvh::RayHit;
use frap_mesh::{AttrData, DeformVert, Domain};
use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::generated::modifier_settings::ShrinkwrapSettings;
use frap_subdiv::{SubdivSettings, SubdivTopology, VtxBoundary};
use smallvec::SmallVec;

use self::geom::{SpaceTransform, V3, interp, len_sq, normalized};
use self::tree::{
    CULL_BACKFACE, CULL_FRONTFACE, CULL_MASK, Method, Mode, Tree, hit_none, nearest_none,
};
use super::warp::shared::{vgroup, weight_safe};
use super::weld::util::m4_from_glam;
use crate::prelude::*;

fn shrinkwrap_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Shrinkwrap(w) = s {
        v.extend(w.target);
        v.extend(w.auxiliary_target);
    }
    v
}

fn shrinkwrap_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Shrinkwrap(w) if w.target.is_none() => Some("Shrinkwrap: no target"),
        _ => None,
    }
}

#[modifier(
    SHRINKWRAP,
    kind = OnlyDeform,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = shrinkwrap_deps,
    is_disabled = shrinkwrap_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Shrinkwrap(s) = &md.settings else {
        return Ok(mesh);
    };
    let me = cx.object_ref();
    // Loop dependencies on the object itself are removed.
    let other = |p: Option<IdAny>| {
        p.and_then(IdRef::<ObjectId>::from_any)
            .filter(|o| *o != me)
            .filter(|o| cx.main().objects.get(*o).is_some())
    };
    let target = other(s.target);
    let aux = other(s.auxiliary_target);
    let Some(target) = target else {
        return Ok(mesh);
    };
    let ob_world = m4_from_glam(cx.object_matrix_world(me));
    let target_world = m4_from_glam(cx.object_matrix_world(target));
    let target_mesh = cx.evaluated_mesh(target)?;
    let aux_target = match aux {
        Some(a) => {
            let w = m4_from_glam(cx.object_matrix_world(a));
            Some((cx.evaluated_mesh(a)?, w))
        }
        None => None,
    };
    let subdivided = (Method::from_identifier(&s.wrap_method) == Method::Project
        && s.subsurf_levels > 0)
        .then(|| subdivided_positions(cx, &mesh, s.subsurf_levels as u32));
    let input = ShrinkwrapInput {
        target: &target_mesh,
        local2target: SpaceTransform::from_matrices(&ob_world, &target_world),
        aux: aux_target
            .as_ref()
            .map(|(m, w)| (&**m, SpaceTransform::from_matrices(&ob_world, w))),
        subdivided: subdivided.as_deref(),
    };
    shrinkwrap(s, &input, &mut mesh);
    Ok(mesh)
}

/// What SHRINKWRAP reads besides the settings and the deformed mesh.
pub(crate) struct ShrinkwrapInput<'a> {
    /// The evaluated target mesh.
    pub target: &'a Mesh,
    /// Local space of the deformed object → the target's.
    pub local2target: SpaceTransform,
    /// The evaluated auxiliary target and its transform (`PROJECT` only).
    pub aux: Option<(&'a Mesh, SpaceTransform)>,
    /// `PROJECT` with `subsurf_levels`: the vertices refined uniformly to that level.
    pub subdivided: Option<&'a [V3]>,
}

/// `ShrinkwrapCalcData`
struct Calc<'a> {
    s: &'a ShrinkwrapSettings,
    method: Method,
    mode: Mode,
    /// `calc->vert_positions` (`PROJECT` only).
    vert_positions: Option<Vec<V3>>,
    vert_normals: Option<Arc<Vec<V3>>>,
    dvert: Option<&'a [DeformVert]>,
    vgroup: Option<u32>,
    invert_vgroup: bool,
    local2target: SpaceTransform,
    tree: Tree,
    keep_dist: f32,
}

impl Calc<'_> {
    fn weight(&self, i: usize) -> f32 {
        weight_safe(self.dvert, i, self.vgroup, self.invert_vgroup)
    }
}

/// The ranges `threading::parallel_for(IndexRange(n), 512, …)` hands out: halved until at
/// most 512 long (one range below the grain size).
fn chunks(start: usize, len: usize, out: &mut Vec<(usize, usize)>) {
    if len <= 512 {
        if len > 0 {
            out.push((start, len));
        }
        return;
    }
    let half = len / 2;
    chunks(start, half, out);
    chunks(start + half, len - half, out);
}

fn parallel_chunks(n: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    chunks(0, n, &mut v);
    v
}

// Ported from blender/source/blender/blenkernel/intern/shrinkwrap.cc (shrinkwrapModifier_deform).
/// Shrink-wraps `mesh` in place.
pub(crate) fn shrinkwrap(s: &ShrinkwrapSettings, inp: &ShrinkwrapInput<'_>, mesh: &mut Mesh) {
    let method = Method::from_identifier(&s.wrap_method);
    let mode = Mode::from_identifier(&s.wrap_mode);
    let Some(tree) = Tree::new(inp.target, method, mode) else {
        return;
    };
    let (group, dvert) = vgroup(mesh, &s.vertex_group);
    let dvert = dvert.map(<[_]>::to_vec);
    let (vert_positions, vert_normals) = if method == Method::Project {
        let p = match inp.subdivided {
            Some(p) => p.to_vec(),
            None => mesh.positions().to_vec(),
        };
        (Some(p), Some(mesh.vert_normals()))
    } else {
        (None, None)
    };
    let calc = Calc {
        s,
        method,
        mode,
        vert_positions,
        vert_normals,
        dvert: dvert.as_deref(),
        vgroup: group,
        invert_vgroup: s.invert_vertex_group,
        local2target: inp.local2target,
        tree,
        keep_dist: s.offset,
    };
    let cos = mesh.positions_mut();
    match method {
        Method::NearestSurface | Method::TargetProject => calc_nearest_surface_point(&calc, cos),
        Method::Project => calc_normal_projection(&calc, inp.aux, cos),
        Method::NearestVertex => calc_nearest_vertex(&calc, cos),
    }
}

/// `shrinkwrap_calc_nearest_vertex`
#[allow(clippy::needless_range_loop)] // Blender's chunked index loops
fn calc_nearest_vertex(calc: &Calc<'_>, cos: &mut [V3]) {
    for (start, len) in parallel_chunks(cos.len()) {
        let mut nearest = nearest_none();
        for i in start..start + len {
            let mut weight = calc.weight(i);
            if weight == 0.0 {
                continue;
            }
            let co = cos[i];
            let tmp_co = calc.local2target.apply(co);
            // Local proximity heuristic: start from the previous hit's distance.
            nearest.dist_sq = if nearest.index.is_some() {
                len_sq(tmp_co, nearest.co)
            } else {
                f32::MAX
            };
            calc.tree.find_nearest(tmp_co, &mut nearest);
            if nearest.index.is_some() {
                // Keep `offset` from the nearest position after the interpolation.
                if nearest.dist_sq > f32::EPSILON {
                    let dist = nearest.dist_sq.sqrt();
                    weight *= (dist - calc.keep_dist) / dist;
                }
                let t = calc.local2target.invert(nearest.co);
                cos[i] = interp(co, t, weight);
            }
        }
    }
}

/// `shrinkwrap_calc_nearest_surface_point`
#[allow(clippy::needless_range_loop)] // Blender's chunked index loops
fn calc_nearest_surface_point(calc: &Calc<'_>, cos: &mut [V3]) {
    for (start, len) in parallel_chunks(cos.len()) {
        let mut nearest = nearest_none();
        for i in start..start + len {
            let weight = calc.weight(i);
            if weight == 0.0 {
                continue;
            }
            let co = cos[i];
            let mut tmp_co = calc.local2target.apply(co);
            if nearest.index.is_some() {
                if calc.method == Method::TargetProject {
                    // The heuristic does not work with the extra restrictions.
                    nearest = nearest_none();
                } else {
                    nearest.dist_sq = len_sq(tmp_co, nearest.co);
                }
            } else {
                nearest.dist_sq = f32::MAX;
            }
            calc.tree
                .find_nearest_surface(&mut nearest, tmp_co, calc.method);
            if let Some(index) = nearest.index {
                tmp_co = calc.tree.snap_point_to_surface(
                    None,
                    calc.mode,
                    index,
                    nearest.co,
                    nearest.no,
                    calc.keep_dist,
                    tmp_co,
                );
                let t = calc.local2target.invert(tmp_co);
                cos[i] = interp(co, t, weight);
            }
        }
    }
}

/// `shrinkwrap_calc_normal_projection`
#[allow(clippy::needless_range_loop)] // Blender's chunked index loops
fn calc_normal_projection(calc: &Calc<'_>, aux: Option<(&Mesh, SpaceTransform)>, cos: &mut [V3]) {
    let s = calc.s;
    if !s.use_positive_direction && !s.use_negative_direction {
        return;
    }
    let over_normal = !(s.use_project_x || s.use_project_y || s.use_project_z);
    let mut proj_axis = [0.0f32; 3];
    if !over_normal {
        if s.use_project_x {
            proj_axis[0] = 1.0;
        }
        if s.use_project_y {
            proj_axis[1] = 1.0;
        }
        if s.use_project_z {
            proj_axis[2] = 1.0;
        }
        proj_axis = normalized(proj_axis);
        if proj_axis[0] * proj_axis[0] + proj_axis[1] * proj_axis[1] + proj_axis[2] * proj_axis[2]
            < f32::EPSILON
        {
            return;
        }
    }
    let aux_tree = aux.and_then(|(m, t)| Tree::new(m, calc.method, calc.mode).map(|tr| (tr, t)));
    let mut options = 0u8;
    match s.cull_face.as_str() {
        "FRONT" => options |= CULL_FRONTFACE,
        "BACK" => options |= CULL_BACKFACE,
        _ => {}
    }
    let proj_limit_squared = s.project_limit * s.project_limit;
    for (start, len) in parallel_chunks(cos.len()) {
        for i in start..start + len {
            let weight = calc.weight(i);
            if weight == 0.0 {
                continue;
            }
            let co = cos[i];
            let (tmp_co, tmp_no) = match (&calc.vert_positions, &calc.vert_normals) {
                (Some(p), Some(n)) if over_normal => (p[i], n[i]),
                _ => (co, proj_axis),
            };
            // `hit.dist` stays in the target's space; it only ranks the hits.
            let mut hit: RayHit = hit_none();
            let mut is_aux = false;
            // Along the positive direction of the axis.
            if s.use_positive_direction {
                if let Some((t, tr)) = &aux_tree
                    && t.project_normal(0, tmp_co, tmp_no, tr, &mut hit)
                {
                    is_aux = true;
                }
                if calc
                    .tree
                    .project_normal(options, tmp_co, tmp_no, &calc.local2target, &mut hit)
                {
                    is_aux = false;
                }
            }
            // Along the negative direction.
            if s.use_negative_direction {
                let inv_no = [-tmp_no[0], -tmp_no[1], -tmp_no[2]];
                let mut opts = options;
                if s.use_invert_cull && opts & CULL_MASK != 0 {
                    opts ^= CULL_MASK;
                }
                if let Some((t, tr)) = &aux_tree
                    && t.project_normal(0, tmp_co, inv_no, tr, &mut hit)
                {
                    is_aux = true;
                }
                if calc
                    .tree
                    .project_normal(opts, tmp_co, inv_no, &calc.local2target, &mut hit)
                {
                    is_aux = false;
                }
            }
            // The limit is measured in the object's own space.
            if proj_limit_squared != 0.0
                && hit.index.is_some()
                && len_sq(hit.co, co) > proj_limit_squared
            {
                hit.index = None;
            }
            if let Some(index) = hit.index {
                let (t, tr) = match (&aux_tree, is_aux) {
                    (Some((t, tr)), true) => (t, tr),
                    _ => (&calc.tree, &calc.local2target),
                };
                hit.co = t.snap_point_to_surface(
                    Some(tr),
                    calc.mode,
                    index,
                    hit.co,
                    hit.no,
                    calc.keep_dist,
                    tmp_co,
                );
                cos[i] = interp(co, hit.co, weight);
            }
        }
    }
}

// Ported from blender/source/blender/blenkernel/intern/shrinkwrap.cc
// (shrinkwrap_calc_subdivided_positions).
/// The vertices where a non-adaptive Catmull-Clark `Subdiv` at `settings.level = level` puts
/// them (`subdiv::deform_coarse_vertices` with creases, boundary `EDGE_ONLY`): Blender refines
/// uniformly to `level`, and OpenSubdiv's uniform patch table is made of linear patches, so the
/// "limit point" at a coarse vertex's corner is its refined position at `level` (it approaches
/// the limit surface only as `level` rises). Loose vertices keep their positions, and a
/// topology the subdivider rejects keeps the coarse positions.
fn subdivided_positions(cx: &mut dyn ModEvalCx, me: &Mesh, level: u32) -> Vec<V3> {
    let mut out = me.positions().to_vec();
    if me.faces_num() == 0 {
        return out;
    }
    let float_attr = |name: &str, domain: Domain| -> Option<&[f32]> {
        let a = me.attributes().get(name)?;
        if a.domain != domain {
            return None;
        }
        match &a.data {
            AttrData::Float(v) => Some(v.as_slice()),
            _ => None,
        }
    };
    let edges: Vec<[u32; 2]> = me
        .edges()
        .iter()
        .map(|e| [e[0] as u32, e[1] as u32])
        .collect();
    let offsets: Vec<u32> = me.face_offsets().iter().map(|&o| o as u32).collect();
    let corner_verts: Vec<u32> = me.corner_verts().iter().map(|&v| v as u32).collect();
    let corner_edges: Vec<u32> = me.corner_edges().iter().map(|&e| e as u32).collect();
    let mut topo = SubdivTopology::new(
        me.positions(),
        &edges,
        &offsets,
        &corner_verts,
        &corner_edges,
    );
    topo.edge_creases = float_attr("crease_edge", Domain::Edge);
    topo.vert_creases = float_attr("crease_vert", Domain::Point);
    // `is_adaptive = false`: the refined vertices themselves (frap's `use_limit_surface`
    // is Blender's adaptive evaluation).
    let mut settings = SubdivSettings::default();
    settings.is_simple = false;
    settings.level = level;
    settings.use_creases = true;
    settings.vtx_boundary = VtxBoundary::EdgeOnly;
    settings.use_limit_surface = false;
    let Ok(r) = cx.subdivider().subdivide(&topo, &settings) else {
        return out;
    };
    // The corner vertices of faces come first in the subdivided mesh, in coarse order.
    for &v in me.corner_verts() {
        let v = v as usize;
        if let Some(p) = r.positions.get(v) {
            out[v] = *p;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::weld::util::unit_m4;

    fn plane() -> Mesh {
        frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
            x_subdivisions: 4,
            y_subdivisions: 4,
            size: 4.0,
            ..Default::default()
        })
    }

    fn input(target: &Mesh) -> ShrinkwrapInput<'_> {
        ShrinkwrapInput {
            target,
            local2target: SpaceTransform::from_matrices(&unit_m4(), &unit_m4()),
            aux: None,
            subdivided: None,
        }
    }

    #[test]
    fn chunks_follow_the_grain_size() {
        assert_eq!(parallel_chunks(100), vec![(0, 100)]);
        assert_eq!(parallel_chunks(1000), vec![(0, 500), (500, 500)]);
        assert_eq!(
            parallel_chunks(1025),
            vec![(0, 512), (512, 256), (768, 257)]
        );
        assert!(parallel_chunks(0).is_empty());
    }

    #[test]
    fn nearest_surface_flattens_onto_a_plane() {
        let target = plane();
        let mut m = frap_mesh::primitives::cube(1.0, false, [1.0; 3]);
        let s = ShrinkwrapSettings::default();
        shrinkwrap(&s, &input(&target), &mut m);
        for p in m.positions() {
            assert!(p[2].abs() < 1e-6, "{p:?}");
        }
    }

    #[test]
    fn modes_keep_the_offset_on_the_right_side() {
        let target = plane();
        let mut m = frap_mesh::primitives::cube(1.0, false, [1.0; 3]);
        let s = ShrinkwrapSettings {
            wrap_mode: "OUTSIDE".into(),
            offset: 0.25,
            ..ShrinkwrapSettings::default()
        };
        let before = m.positions().to_vec();
        shrinkwrap(&s, &input(&target), &mut m);
        for (p, b) in m.positions().iter().zip(&before) {
            if b[2] > 0.0 {
                // Already outside and farther than the offset: unchanged.
                assert_eq!(p, b);
            } else {
                // Below the plane: moved to the outside at the offset.
                assert!((p[2].abs() - 0.25).abs() < 1e-6, "{p:?}");
            }
        }
    }

    #[test]
    fn projection_along_z_hits_the_plane() {
        let target = plane();
        let mut m = frap_mesh::primitives::cube(1.0, false, [1.0; 3]);
        let s = ShrinkwrapSettings {
            wrap_method: "PROJECT".into(),
            use_project_z: true,
            use_negative_direction: true,
            ..ShrinkwrapSettings::default()
        };
        shrinkwrap(&s, &input(&target), &mut m);
        for p in m.positions() {
            assert!(p[2].abs() < 1e-6, "{p:?}");
        }
    }

    #[test]
    fn nearest_vertex_snaps_to_grid_vertices() {
        let target = plane();
        let mut m = frap_mesh::primitives::cube(0.4, false, [1.0; 3]);
        let s = ShrinkwrapSettings {
            wrap_method: "NEAREST_VERTEX".into(),
            ..ShrinkwrapSettings::default()
        };
        shrinkwrap(&s, &input(&target), &mut m);
        for p in m.positions() {
            // The grid vertex at the origin (within the BVH's FLT_EPSILON bounds).
            assert!(p.iter().all(|c| c.abs() < 1e-6), "{p:?}");
        }
    }
}
