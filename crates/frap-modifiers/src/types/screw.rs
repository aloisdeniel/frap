// SPDX-License-Identifier: GPL-3.0-or-later
//! SCREW (`ScrewModifier`): revolves the edges about `axis` (or `object`'s axis, relative to
//! the object) by `angle` in `steps` (`render_steps` for renders) slices, `iterations` times,
//! moving along the axis by `screw_offset` (or `object`'s offset with
//! `use_object_screw_offset`). A full turn without offset closes the ring. Every edge becomes a
//! strip of quads that keep their source face's data; `use_normal_calculate` orders connected
//! edges so the faces point outwards and `use_normal_flip` flips them; `use_smooth_shade`
//! shades the new faces smooth; UVs run along the turn (`use_stretch_u`/`use_stretch_v`
//! normalise them); `use_merge_vertices` welds the vertices within `merge_threshold` of the
//! axis (without offset).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_screw.cc (Blender 5.2.4). The weld of
//! the axis vertices is the part of `geometry::mesh_merge_verts`
//! (geometry/intern/mesh_merge_verts.cc) that an axis-only merge map exercises: see
//! [`merge_axis`].

use frap_mesh::{AttrData, Attribute, Domain};
use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::EvalMode;
use frap_scene::generated::modifier_settings::ScrewSettings;
use glam::Mat4;
use smallvec::SmallVec;
use std::sync::Arc;

use super::smooth::common::{Sources, Src, Topology, rebuild};
use crate::prelude::*;

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Screw(c) = s
        && let Some(o) = c.object
    {
        v.push(o);
    }
    v
}

#[modifier(
    SCREW,
    kind = Constructive,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = deps,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Screw(s) = &md.settings else {
        return Ok(mesh);
    };
    // The axis object's matrix relative to this object (`mtx_tx`). Blender refuses to assign
    // the modified object itself, so a self reference counts as no axis object.
    let own = cx.object_ref();
    let mtx_tx = s
        .object
        .and_then(IdRef::<ObjectId>::from_any)
        .filter(|o| *o != own && cx.main().objects.get(*o).is_some())
        .map(|o| {
            let own = cx.object_matrix_world(own);
            let axis = cx.object_matrix_world(o);
            m4(own.inverse() * axis)
        });
    let render = cx.mode() == EvalMode::Render;
    screw(s, mtx_tx, render, &mesh)
}

type V3 = [f32; 3];
/// Blender's `float[4][4]` (`m[col][row]`).
type M4 = [[f32; 4]; 4];

fn m4(m: Mat4) -> M4 {
    m.to_cols_array_2d()
}

const SV_UNUSED: u32 = u32::MAX;
const SV_INVALID: u32 = u32::MAX - 1;
fn sv_is_valid(v: u32) -> bool {
    v < SV_INVALID
}

/// `ScrewVertConnect` (edges by index into the new edge array).
#[derive(Copy, Clone)]
struct VertConnect {
    dist_sq: f32,
    co: V3,
    v: [u32; 2],
    e: [Option<usize>; 2],
    flag: u8,
}

/// `ScrewVertIter`.
struct VertIter {
    v: u32,
    v_other: u32,
    /// `v_poin` (the vertex `v` when valid).
    poin: Option<usize>,
    e: Option<usize>,
}

impl VertIter {
    // Ported from blender/source/blender/modifiers/intern/MOD_screw.cc (screwvert_iter_init).
    fn init(array: &[VertConnect], v_init: u32, dir: usize) -> VertIter {
        if sv_is_valid(v_init) {
            let p = &array[v_init as usize];
            VertIter {
                v: v_init,
                v_other: p.v[dir],
                poin: Some(v_init as usize),
                e: p.e[1 - dir],
            }
        } else {
            VertIter {
                v: v_init,
                v_other: SV_UNUSED,
                poin: None,
                e: None,
            }
        }
    }

    // Ported from blender/source/blender/modifiers/intern/MOD_screw.cc (screwvert_iter_step).
    fn step(&mut self, array: &[VertConnect]) {
        let p = array[self.poin.expect("stepping a valid iterator")];
        if p.v[0] == self.v_other {
            self.v_other = self.v;
            self.v = p.v[1];
        } else if p.v[1] == self.v_other {
            self.v_other = self.v;
            self.v = p.v[0];
        }
        if sv_is_valid(self.v) {
            let q = &array[self.v as usize];
            self.poin = Some(self.v as usize);
            self.e = q.e[usize::from(q.e[0] == self.e)];
        } else {
            self.e = None;
            self.poin = None;
        }
    }
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
/// `normalize_v3`.
fn normalize(v: V3) -> V3 {
    let d = dot(v, v);
    if d > 1.0e-35 {
        let l = 1.0 / d.sqrt();
        [v[0] * l, v[1] * l, v[2] * l]
    } else {
        [0.0; 3]
    }
}
/// `mul_m4_v3`.
fn mul_m4_v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2] + m[3][0],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2] + m[3][1],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2] + m[3][2],
    ]
}
/// `mul_mat3_m4_v3`.
fn mul_mat3_m4_v3(m: &M4, v: V3) -> V3 {
    let (x, y) = (v[0], v[1]);
    [
        x * m[0][0] + y * m[1][0] + m[2][0] * v[2],
        x * m[0][1] + y * m[1][1] + m[2][1] * v[2],
        x * m[0][2] + y * m[1][2] + m[2][2] * v[2],
    ]
}
fn t3(m: &M4) -> V3 {
    [m[3][0], m[3][1], m[3][2]]
}

/// `axis_angle_to_mat3_single`.
fn axis_angle_to_mat3_single(axis: usize, angle: f32) -> [[f32; 3]; 3] {
    let (c, s) = (libm::cosf(angle), libm::sinf(angle));
    match axis {
        0 => [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]],
        1 => [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]],
        _ => [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// `axis_angle_normalized_to_mat3`.
fn axis_angle_normalized_to_mat3(axis: V3, angle: f32) -> [[f32; 3]; 3] {
    let (si, co) = (libm::sinf(angle), libm::cosf(angle));
    let ico = 1.0 - co;
    let nsi = [axis[0] * si, axis[1] * si, axis[2] * si];
    let n_00 = (axis[0] * axis[0]) * ico;
    let n_01 = (axis[0] * axis[1]) * ico;
    let n_11 = (axis[1] * axis[1]) * ico;
    let n_02 = (axis[0] * axis[2]) * ico;
    let n_12 = (axis[1] * axis[2]) * ico;
    let n_22 = (axis[2] * axis[2]) * ico;
    [
        [n_00 + co, n_01 + nsi[2], n_02 - nsi[1]],
        [n_01 - nsi[2], n_11 + co, n_12 + nsi[0]],
        [n_02 + nsi[1], n_12 - nsi[0], n_22 + co],
    ]
}

/// What happens to the UVs of one new corner.
#[derive(Copy, Clone)]
enum UvOp {
    /// `mluv[0] += u` (the corner was copied from a source corner).
    AddU(f32),
    /// `mluv = (u, v)`.
    Set(f32, f32),
}

/// The output sizes of MOD_screw.cc: `(close, step_tot, max_verts, max_edges, max_polys)`.
///
/// Blender computes them in `uint` and wraps on overflow; frap computes them in `u64` and
/// checks them against the element budget (ARCHITECTURE §12, `--max-elements` and Blender's
/// `int` index range) before anything is allocated, failing with the `LIMIT` message.
fn screw_sizes(
    steps: u32,
    iter: u32,
    totvert: u32,
    totedge: u32,
    screw_ofs: f32,
    angle: f32,
) -> Result<(bool, u32, u32, u32, u32), ModError> {
    let (steps, iter, totvert, totedge) = (
        u64::from(steps),
        u64::from(iter),
        u64::from(totvert),
        u64::from(totedge),
    );
    // `((step_tot + 1) * iter) - (iter - 1)`, which is `step_tot * iter + 1` (also for
    // `iter == 0`, where Blender's `uint` arithmetic wraps back to 1).
    let sizes = steps
        .checked_mul(iter)
        .and_then(|n| n.checked_add(1))
        .and_then(|step_tot| {
            let close = screw_ofs.abs() <= f32::EPSILON * 100.0
                && (angle.abs() - std::f32::consts::PI * 2.0).abs() <= f32::EPSILON * 100.0
                && step_tot > 3;
            let (step_tot, verts, edges, polys) = if close {
                let step_tot = step_tot - 1;
                let verts = totvert.checked_mul(step_tot)?;
                (
                    step_tot,
                    verts,
                    verts.checked_add(totedge.checked_mul(step_tot)?)?,
                    totedge.checked_mul(step_tot)?,
                )
            } else {
                let step_tot = step_tot.max(2);
                let verts = totvert.checked_mul(step_tot)?;
                let edges = totvert
                    .checked_mul(step_tot - 1)?
                    .checked_add(totedge.checked_mul(step_tot)?)?;
                (step_tot, verts, edges, totedge.checked_mul(step_tot - 1)?)
            };
            Some((close, step_tot, verts, edges, polys, polys.checked_mul(4)?))
        });
    let reserve = match sizes {
        Some((.., verts, edges, polys, corners)) => frap_math::budget::reserve_corners(
            Some(corners),
            Some(verts.max(edges).max(polys).max(corners)),
        ),
        None => frap_math::budget::reserve_corners(None, None),
    };
    reserve.map_err(|e| ModError::Error(e.to_string()))?;
    let (close, step_tot, verts, edges, polys, _) =
        sizes.expect("reserve_corners rejects an overflowed estimate");
    // Within Blender's `int` range now (`reserve_corners` checked it).
    let n = |v: u64| u32::try_from(v).map_err(|_| ModError::Error("frap: limit exceeded".into()));
    Ok((close, n(step_tot)?, n(verts)?, n(edges)?, n(polys)?))
}

// Ported from blender/source/blender/modifiers/intern/MOD_screw.cc (modify_mesh).
/// The SCREW result of `mesh`; `mtx_tx` is the axis object's matrix in object space.
pub(crate) fn screw(
    s: &ScrewSettings,
    mtx_tx: Option<M4>,
    render: bool,
    mesh: &Mesh,
) -> Result<Mesh, ModError> {
    let totvert = mesh.verts_num() as u32;
    let totedge = mesh.edges_num() as u32;
    let faces_num = mesh.faces_num();
    let step_tot = if render { s.render_steps } else { s.steps }.max(0) as u32;
    let do_flip = s.use_normal_flip;
    let quad_ord: [usize; 4] = [
        if do_flip { 3 } else { 0 },
        if do_flip { 2 } else { 1 },
        if do_flip { 1 } else { 2 },
        if do_flip { 0 } else { 3 },
    ];
    let quad_ord_ofs: [usize; 4] = [
        if do_flip { 2 } else { 0 },
        1,
        if do_flip { 0 } else { 2 },
        3,
    ];
    if totvert == 0 {
        return Ok(rebuild(mesh, Topology::default(), &Sources::default()));
    }
    let axis = match s.axis.as_str() {
        "X" => 0usize,
        "Y" => 1,
        _ => 2,
    };
    let (other_axis_1, other_axis_2) = match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    let iter = s.iterations.max(0) as u32;
    let mut angle = s.angle;
    let mut screw_ofs = s.screw_offset;
    let mut axis_vec: V3 = [0.0; 3];
    axis_vec[axis] = 1.0;
    if let Some(mtx) = &mtx_tx {
        axis_vec = normalize(mul_mat3_m4_v3(mtx, axis_vec));
        if s.use_object_screw_offset {
            let t = t3(mtx);
            let totlen = dot(t, t).sqrt();
            screw_ofs = if totlen != 0.0 {
                // closest_to_line_v3(cp, mtx_tx[3], zero, axis_vec): line_point_factor_v3.
                let len_sq = dot(axis_vec, axis_vec);
                if len_sq > 0.0 {
                    dot(axis_vec, t) / len_sq
                } else {
                    0.0
                }
            } else {
                0.0
            };
        }
    }
    angle *= iter as f32;
    screw_ofs *= iter as f32;
    let mut uv_u_scale = 1.0 / step_tot as f32;
    let sizes = screw_sizes(step_tot, iter, totvert, totedge, screw_ofs, angle)?;
    let (close, step_tot, max_verts, max_edges, max_polys) = sizes;
    if close {
        screw_ofs = 0.0;
    }
    if !s.use_stretch_u {
        uv_u_scale = (uv_u_scale / iter as f32) * (angle / (std::f32::consts::PI * 2.0));
    }
    let do_remove_doubles = s.use_merge_vertices && screw_ofs == 0.0;
    let use_flat_shading = !s.use_smooth_shade;

    let positions_orig = mesh.positions();
    let edges_orig = mesh.edges();
    let cv_orig = mesh.corner_verts();
    let ce_orig = mesh.corner_edges();
    let uv_names: Vec<String> = mesh.uv_maps().iter().map(|(n, _)| n.to_string()).collect();
    let has_uv = !uv_names.is_empty();

    let mut positions: Vec<V3> = vec![[0.0; 3]; max_verts as usize];
    positions[..totvert as usize].copy_from_slice(positions_orig);
    let mut vert_src: Vec<Src> = (0..max_verts as usize)
        .map(|i| Src::Copy(i % totvert as usize))
        .collect();
    let mut edges_new: Vec<[i32; 2]> = vec![[0; 2]; max_edges as usize];

    // UV plane and V range.
    let mut uv_axis_plane = [0.0f32; 4];
    let mut uv_v_minmax = [f32::MAX, -f32::MAX];
    let mut uv_v_range_inv = 0.0f32;
    let plane_side = |plane: &[f32; 4], p: V3| dot([plane[0], plane[1], plane[2]], p) + plane[3];
    if has_uv {
        uv_axis_plane = [
            axis_vec[0],
            axis_vec[1],
            axis_vec[2],
            -dot(axis_vec, [0.0; 3]),
        ];
        if s.use_stretch_v {
            for &p in positions_orig {
                // dist_signed_squared_to_plane_v3.
                let len_sq = dot(axis_vec, axis_vec);
                let side = plane_side(&uv_axis_plane, p);
                let fac = side / len_sq;
                let v = (len_sq * (fac * fac)).copysign(side);
                uv_v_minmax[0] = v.min(uv_v_minmax[0]);
                uv_v_minmax[1] = v.max(uv_v_minmax[1]);
            }
            let sqrtf_signed = |f: f32| if f >= 0.0 { f.sqrt() } else { -(-f).sqrt() };
            uv_v_minmax = [sqrtf_signed(uv_v_minmax[0]), sqrtf_signed(uv_v_minmax[1])];
        }
        uv_v_range_inv = uv_v_minmax[1] - uv_v_minmax[0];
        uv_v_range_inv = if uv_v_range_inv != 0.0 {
            1.0 / uv_v_range_inv
        } else {
            0.0
        };
    }
    let dist_signed_to_plane = |p: V3| {
        let len_sq = dot(axis_vec, axis_vec);
        let side = plane_side(&uv_axis_plane, p);
        let fac = side / len_sq;
        len_sq.sqrt() * fac
    };

    // Copy the first set of edges.
    edges_new[..totedge as usize].copy_from_slice(edges_orig);
    // Face → edge map, vertex → corner map, edges ordered by faces.
    let mut edge_face_map: Option<Vec<u32>> = None;
    let mut vert_loop_map: Option<Vec<u32>> = None;
    if faces_num > 0 {
        let mut efm = vec![u32::MAX; totedge as usize];
        let mut vlm = vec![u32::MAX; totvert as usize];
        for f in 0..faces_num {
            for c in mesh.face(f) {
                let vert_i = cv_orig[c];
                let edge_i = ce_orig[c] as usize;
                efm[edge_i] = f as u32;
                vlm[vert_i as usize] = c as u32;
                if edges_new[edge_i][0] != vert_i {
                    edges_new[edge_i].swap(0, 1);
                }
            }
        }
        edge_face_map = Some(efm);
        vert_loop_map = Some(vlm);
    }

    if s.use_normal_calculate && totedge != 0 {
        // Normal calculation (for face flipping): sort the edge vertices.
        let mut vc: Vec<VertConnect> = positions_orig
            .iter()
            .map(|&p| {
                let co = match &mtx_tx {
                    Some(m) => mul_m4_v3(m, p),
                    None => p,
                };
                VertConnect {
                    dist_sq: co[other_axis_1] * co[other_axis_1]
                        + co[other_axis_2] * co[other_axis_2],
                    co,
                    v: [SV_UNUSED; 2],
                    e: [None; 2],
                    flag: 0,
                }
            })
            .collect();
        for (i, e) in edges_new[..totedge as usize].iter().enumerate() {
            for (a, b) in [(e[0], e[1]), (e[1], e[0])] {
                let c = &mut vc[a as usize];
                if c.v[0] == SV_UNUSED {
                    c.v[0] = b as u32;
                    c.e[0] = Some(i);
                } else if c.v[1] == SV_UNUSED {
                    c.v[1] = b as u32;
                    c.e[1] = Some(i);
                } else {
                    c.v = [SV_INVALID; 2];
                }
            }
        }
        for i in 0..totvert {
            frap_math::budget::check();
            if vc[i as usize].flag != 0 {
                continue;
            }
            let mut vc_tot_linked = 0u32;
            let mut v_best = SV_UNUSED;
            let mut ed_loop_closed = 0usize;
            let mut fl = -1.0f32;
            for j in 0..2 {
                let mut it = VertIter::init(&vc, i, j);
                if j == 1 {
                    it.step(&vc);
                }
                while let Some(p) = it.poin {
                    if vc[p].flag != 0 {
                        ed_loop_closed = 1;
                        break;
                    }
                    vc[p].flag = 1;
                    vc_tot_linked += 1;
                    if fl <= vc[p].dist_sq {
                        fl = vc[p].dist_sq;
                        v_best = it.v;
                    }
                    it.step(&vc);
                    if it.poin.is_none() {
                        break;
                    }
                }
            }
            if vc_tot_linked <= 1 {
                continue;
            }
            let tmp = vc[v_best as usize];
            let mut ed_loop_flip = false;
            if sv_is_valid(tmp.v[0]) && sv_is_valid(tmp.v[1]) {
                let tmpf1 = vc[tmp.v[0] as usize].co;
                let tmpf2 = vc[tmp.v[1] as usize].co;
                let (vf_1, vf_2, vf_best) = (tmpf1[axis], tmpf2[axis], tmp.co[axis]);
                if vf_1 < vf_best && vf_best < vf_2 {
                    ed_loop_flip = false;
                } else if vf_1 > vf_best && vf_best > vf_2 {
                    ed_loop_flip = true;
                } else {
                    let v1 = normalize(sub(tmpf1, tmp.co));
                    let v2 = normalize(sub(tmpf2, tmp.co));
                    ed_loop_flip = v1[axis] < v2[axis];
                }
            } else if sv_is_valid(tmp.v[0]) {
                let tmpf1 = vc[tmp.v[0] as usize].co;
                ed_loop_flip = tmpf1[axis] < tmp.co[axis];
            }
            if angle < 0.0 {
                ed_loop_flip = !ed_loop_flip;
            }
            for j in ed_loop_closed..2 {
                let mut it = VertIter::init(&vc, v_best, j);
                if let Some(p) = it.poin {
                    vc[p].flag = 1;
                }
                if j == 1 && sv_is_valid(tmp.v[0]) && sv_is_valid(tmp.v[1]) {
                    ed_loop_flip = !ed_loop_flip;
                }
                while let Some(p) = it.poin {
                    if vc[p].flag == 2 {
                        break;
                    }
                    vc[p].flag = 2;
                    if let Some(e) = it.e {
                        if it.v == edges_new[e][0] as u32 {
                            if !ed_loop_flip {
                                edges_new[e].swap(0, 1);
                            }
                        } else if it.v == edges_new[e][1] as u32 && ed_loop_flip {
                            edges_new[e].swap(0, 1);
                        }
                    }
                    it.step(&vc);
                }
            }
        }
    }

    // Add the slices.
    let mut edge_cursor = totedge as usize;
    for step in 1..step_tot {
        let stride = (totvert * step) as usize;
        let step_angle = (angle / (step_tot - u32::from(!close)) as f32) * step as f32;
        let mat3 = if mtx_tx.is_some() {
            axis_angle_normalized_to_mat3(axis_vec, step_angle)
        } else {
            axis_angle_to_mat3_single(axis, step_angle)
        };
        let mut mat: M4 = [
            [mat3[0][0], mat3[0][1], mat3[0][2], 0.0],
            [mat3[1][0], mat3[1][1], mat3[1][2], 0.0],
            [mat3[2][0], mat3[2][1], mat3[2][2], 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        if screw_ofs != 0.0 {
            let f = screw_ofs * (step as f32 / (step_tot - 1) as f32);
            for k in 0..3 {
                mat[3][k] += axis_vec[k] * f;
            }
        }
        for j in 0..totvert as usize {
            let vert_new = stride + j;
            let mut p = positions[j];
            if let Some(mtx) = &mtx_tx {
                let t = t3(mtx);
                p = sub(p, t);
                p = mul_m4_v3(&mat, p);
                p = [p[0] + t[0], p[1] + t[1], p[2] + t[2]];
            } else {
                p = mul_m4_v3(&mat, p);
            }
            positions[vert_new] = p;
            vert_src[vert_new] = Src::Copy(j);
            edges_new[edge_cursor] = [vert_new as i32, vert_new as i32 - totvert as i32];
            edge_cursor += 1;
        }
    }
    if close {
        let stride = ((step_tot - 1) * totvert) as i32;
        for i in 0..totvert as i32 {
            edges_new[edge_cursor] = [i, stride + i];
            edge_cursor += 1;
        }
    }

    // Faces.
    let edge_offset = totedge + totvert * (step_tot - u32::from(!close));
    let mut face_offsets = Vec::with_capacity(max_polys as usize + 1);
    let mut corner_verts = vec![0i32; max_polys as usize * 4];
    let mut corner_edges = vec![0i32; max_polys as usize * 4];
    let mut face_src: Vec<Src> = Vec::with_capacity(max_polys as usize);
    let mut face_is_new: Vec<bool> = Vec::with_capacity(max_polys as usize);
    let mut corner_src: Vec<Src> = vec![Src::Default; max_polys as usize * 4];
    let mut uv_ops: Vec<UvOp> = vec![UvOp::AddU(0.0); max_polys as usize * 4];
    let mut new_loop = 0usize;
    for i in 0..totedge as usize {
        let first = edges_new[i];
        let step_last = step_tot - if close { 1 } else { 2 };
        let face_index_orig = edge_face_map.as_ref().map_or(u32::MAX, |m| m[i]);
        let has_mpoly_orig = face_index_orig != u32::MAX;
        let mloop_index_orig = [
            vert_loop_map
                .as_ref()
                .map_or(u32::MAX, |m| m[first[0] as usize]),
            vert_loop_map
                .as_ref()
                .map_or(u32::MAX, |m| m[first[1] as usize]),
        ];
        let has_mloop_orig = edge_face_map.as_ref().is_some_and(|m| m[i] != u32::MAX)
            && mloop_index_orig[0] != u32::MAX;
        let (mut i1, mut i2) = (first[0] as u32, first[1] as u32);
        let (mut uv_v_offset_a, mut uv_v_offset_b) = (0.0f32, 0.0f32);
        if !has_mloop_orig && has_uv {
            uv_v_offset_a = dist_signed_to_plane(positions[first[0] as usize]);
            uv_v_offset_b = dist_signed_to_plane(positions[first[1] as usize]);
            if s.use_stretch_v {
                uv_v_offset_a = (uv_v_offset_a - uv_v_minmax[0]) * uv_v_range_inv;
                uv_v_offset_b = (uv_v_offset_b - uv_v_minmax[0]) * uv_v_range_inv;
            }
        }
        for step in 0..=step_last {
            frap_math::budget::check();
            let face_index = face_src.len();
            if has_mpoly_orig {
                face_src.push(Src::Copy(face_index_orig as usize));
                face_is_new.push(false);
            } else {
                face_src.push(Src::Default);
                face_is_new.push(true);
            }
            face_offsets.push((face_index * 4) as i32);
            let uv_u_offset_a = step as f32 * uv_u_scale;
            let uv_u_offset_b = (step + 1) as f32 * uv_u_scale;
            if has_mloop_orig {
                let (a, b) = (mloop_index_orig[0] as usize, mloop_index_orig[1] as usize);
                corner_src[new_loop] = Src::Copy(a);
                corner_src[new_loop + 1] = Src::Copy(b);
                corner_src[new_loop + 2] = Src::Copy(b);
                corner_src[new_loop + 3] = Src::Copy(a);
                uv_ops[new_loop + quad_ord[0]] = UvOp::AddU(uv_u_offset_a);
                uv_ops[new_loop + quad_ord[1]] = UvOp::AddU(uv_u_offset_a);
                uv_ops[new_loop + quad_ord[2]] = UvOp::AddU(uv_u_offset_b);
                uv_ops[new_loop + quad_ord[3]] = UvOp::AddU(uv_u_offset_b);
            } else {
                uv_ops[new_loop + quad_ord[0]] = UvOp::Set(uv_u_offset_a, uv_v_offset_a);
                uv_ops[new_loop + quad_ord[1]] = UvOp::Set(uv_u_offset_a, uv_v_offset_b);
                uv_ops[new_loop + quad_ord[2]] = UvOp::Set(uv_u_offset_b, uv_v_offset_b);
                uv_ops[new_loop + quad_ord[3]] = UvOp::Set(uv_u_offset_b, uv_v_offset_a);
            }
            let row = (i as u32 * (step_tot - 1)) as i32;
            if !(close && step == step_last) {
                corner_verts[new_loop + quad_ord[0]] = i1 as i32;
                corner_verts[new_loop + quad_ord[1]] = i2 as i32;
                corner_verts[new_loop + quad_ord[2]] = (i2 + totvert) as i32;
                corner_verts[new_loop + quad_ord[3]] = (i1 + totvert) as i32;
                corner_edges[new_loop + quad_ord_ofs[0]] = if step == 0 {
                    i as i32
                } else {
                    edge_offset as i32 + step as i32 + row - 1
                };
                corner_edges[new_loop + quad_ord_ofs[1]] = (totedge + i2) as i32;
                corner_edges[new_loop + quad_ord_ofs[2]] = edge_offset as i32 + step as i32 + row;
                corner_edges[new_loop + quad_ord_ofs[3]] = (totedge + i1) as i32;
                if step != 0 {
                    edges_new[edge_cursor] = [i1 as i32, i2 as i32];
                    edge_cursor += 1;
                }
                i1 += totvert;
                i2 += totvert;
            } else {
                corner_verts[new_loop + quad_ord[0]] = i1 as i32;
                corner_verts[new_loop + quad_ord[1]] = i2 as i32;
                corner_verts[new_loop + quad_ord[2]] = first[1];
                corner_verts[new_loop + quad_ord[3]] = first[0];
                corner_edges[new_loop + quad_ord_ofs[0]] =
                    edge_offset as i32 + step as i32 + row - 1;
                corner_edges[new_loop + quad_ord_ofs[1]] = (totedge + i2) as i32;
                corner_edges[new_loop + quad_ord_ofs[2]] = i as i32;
                corner_edges[new_loop + quad_ord_ofs[3]] = (totedge + i1) as i32;
            }
            new_loop += 4;
        }
        edges_new[edge_cursor] = [i1 as i32, i2 as i32];
        edge_cursor += 1;
    }
    face_offsets.push(new_loop as i32);
    debug_assert_eq!(edge_cursor, max_edges as usize);

    let topo = Topology {
        edges: edges_new,
        face_offsets,
        corner_verts,
        corner_edges,
    };
    let src = Sources {
        vert: vert_src,
        // The modifier never copies edge data (`edge_interp` is unused).
        edge: vec![Src::Default; max_edges as usize],
        face: face_src,
        corner: corner_src,
    };
    let mut result = rebuild(mesh, topo, &src);
    result.set_positions(positions);
    // `lookup_or_add` of `sharp_face` and `material_index`: new faces get flat shading and
    // material 0.
    let nf = face_is_new.len();
    let mut sharp: Vec<bool> = match result.attributes().get("sharp_face") {
        Some(Attribute {
            domain: Domain::Face,
            data: AttrData::Bool(v),
            ..
        }) => v.to_vec(),
        _ => vec![false; nf],
    };
    let mut mat: Vec<i32> = match result.attributes().get("material_index") {
        Some(Attribute {
            domain: Domain::Face,
            data: AttrData::Int(v),
            ..
        }) => v.to_vec(),
        _ => vec![0; nf],
    };
    for (f, new) in face_is_new.iter().enumerate() {
        if *new {
            sharp[f] = use_flat_shading;
            mat[f] = 0;
        }
    }
    let attrs = result.attributes_mut();
    attrs.insert(Attribute {
        name: "sharp_face".into(),
        domain: Domain::Face,
        data: AttrData::Bool(Arc::new(sharp)),
    });
    attrs.insert(Attribute {
        name: "material_index".into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(mat)),
    });
    for name in &uv_names {
        if let Some(Attribute {
            data: AttrData::Float2(uv),
            ..
        }) = attrs.get_mut(name)
        {
            let uv = Arc::make_mut(uv);
            for (c, op) in uv_ops.iter().enumerate() {
                match *op {
                    UvOp::AddU(u) => uv[c][0] += u,
                    UvOp::Set(u, v) => uv[c] = [u, v],
                }
            }
        }
    }
    if do_remove_doubles {
        let offset = mtx_tx.as_ref().map(t3);
        result = remove_doubles_on_axis(
            result,
            totvert as usize,
            step_tot as usize,
            axis_vec,
            offset,
            s.merge_threshold,
        );
    }
    Ok(result)
}

// Ported from blender/source/blender/modifiers/intern/MOD_screw.cc (mesh_remove_doubles_on_axis).
/// Snaps the first slice's vertices within `merge_threshold` of the axis onto it and welds
/// their copies in the other slices into them.
fn remove_doubles_on_axis(
    mut result: Mesh,
    totvert: usize,
    step_tot: usize,
    axis_vec: V3,
    axis_offset: Option<V3>,
    merge_threshold: f32,
) -> Mesh {
    let thr_sq = merge_threshold * merge_threshold;
    let mut tag = vec![false; totvert];
    let mut tot_doubles = 0usize;
    {
        let positions = result.positions_mut();
        for i in 0..totvert {
            let p = positions[i];
            let axis_co = match axis_offset {
                Some(o) => {
                    let oc = sub(p, o);
                    let m = dot(oc, axis_vec);
                    [
                        axis_vec[0] * m + o[0],
                        axis_vec[1] * m + o[1],
                        axis_vec[2] * m + o[2],
                    ]
                }
                None => {
                    let m = dot(p, axis_vec);
                    [axis_vec[0] * m, axis_vec[1] * m, axis_vec[2] * m]
                }
            };
            let d = sub(axis_co, p);
            if dot(d, d) <= thr_sq {
                tag[i] = true;
                tot_doubles += 1;
                positions[i] = axis_co;
            }
        }
    }
    if tot_doubles == 0 {
        return result;
    }
    let mut dest = vec![-1i32; result.verts_num()];
    for (i, t) in tag.iter().enumerate() {
        if *t {
            for step in 1..step_tot {
                dest[step * totvert + i] = i as i32;
            }
        }
    }
    merge_axis(&result, &mut dest)
}

/// The weld of `geometry::mesh_merge_verts(mesh, dest, …, false)` for a merge map whose
/// targets are never merged themselves (the axis weld): merged vertices are removed (attributes
/// of the target kept); edges whose ends merge into one vertex collapse; edges that end up
/// joining the same two vertices keep the lowest-index one; a face loses the corners whose edge
/// collapsed (its first remaining corner leads), a triangle that loses one dies, and a corner
/// at a merged vertex mixes the corners of its face that merge into the same vertex.
fn merge_axis(mesh: &Mesh, dest: &mut [i32]) -> Mesh {
    const OUT: i32 = -1;
    const COLLAPSED: i32 = -2;
    let nv = mesh.verts_num();
    // weld_vert_ctx_alloc_and_setup: targets reference themselves.
    for i in 0..nv {
        let d = dest[i];
        if d != OUT && dest[d as usize] != d {
            dest[d as usize] = d;
        }
    }
    // merge_customdata_all (vertices).
    let mut vert_final = vec![0i32; nv];
    let mut vert_src: Vec<Src> = Vec::new();
    for i in 0..nv {
        if dest[i] == OUT || dest[i] == i as i32 {
            vert_final[i] = vert_src.len() as i32;
            vert_src.push(Src::Copy(i));
        }
    }
    for i in 0..nv {
        if dest[i] != OUT && dest[i] != i as i32 {
            vert_final[i] = vert_final[dest[i] as usize];
        }
    }
    let vdest = |v: i32| -> i32 {
        let d = dest[v as usize];
        if d == OUT { v } else { d }
    };
    let in_ctx = |v: i32| dest[v as usize] != OUT;
    // Edges: collapsed, then doubles (the lowest index of a pair of final vertices stays).
    let edges = mesh.edges();
    let mut edge_dest = vec![OUT; edges.len()];
    let mut pairs: rustc_hash::FxHashMap<(i32, i32), usize> = rustc_hash::FxHashMap::default();
    for (i, e) in edges.iter().enumerate() {
        if !in_ctx(e[0]) && !in_ctx(e[1]) {
            continue;
        }
        let (a, b) = (vdest(e[0]), vdest(e[1]));
        if a == b {
            edge_dest[i] = COLLAPSED;
            continue;
        }
        let key = (a.min(b), a.max(b));
        match pairs.get(&key) {
            Some(&k) => edge_dest[i] = k as i32,
            None => {
                pairs.insert(key, i);
                edge_dest[i] = i as i32;
            }
        }
    }
    let mut edge_final = vec![0i32; edges.len()];
    let mut edge_src: Vec<Src> = Vec::new();
    let mut new_edges: Vec<[i32; 2]> = Vec::new();
    for (i, e) in edges.iter().enumerate() {
        if edge_dest[i] == OUT || edge_dest[i] == i as i32 {
            edge_final[i] = new_edges.len() as i32;
            edge_src.push(Src::Copy(i));
            new_edges.push([vert_final[e[0] as usize], vert_final[e[1] as usize]]);
        }
    }
    for i in 0..edges.len() {
        if edge_dest[i] >= 0 && edge_dest[i] != i as i32 {
            edge_final[i] = edge_final[edge_dest[i] as usize];
        }
    }
    // Faces.
    let cv = mesh.corner_verts();
    let ce = mesh.corner_edges();
    let mut face_offsets = vec![0i32];
    let mut corner_verts = Vec::new();
    let mut corner_edges = Vec::new();
    let mut face_src = Vec::new();
    let mut corner_src = Vec::new();
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        let n = r.len();
        let start = r.start;
        // In-context loops: `(loop, final vertex dest, edge dest)`.
        let ctx: Vec<Option<(i32, i32)>> = (0..n)
            .map(|k| {
                let l = start + k;
                let v = cv[l];
                let v_next = cv[start + (k + 1) % n];
                if in_ctx(v) || in_ctx(v_next) {
                    let e = ce[l];
                    let ed = edge_dest[e as usize];
                    Some((vdest(v), if ed == OUT { e } else { ed }))
                } else {
                    None
                }
            })
            .collect();
        let collapsed = |k: usize| matches!(ctx[k], Some((_, COLLAPSED)));
        let alive = (0..n).filter(|&k| !collapsed(k)).count();
        if alive < n && (n == 3 || alive < 3) {
            continue;
        }
        face_src.push(Src::Copy(f));
        for k in (0..n).filter(|&k| !collapsed(k)) {
            let l = start + k;
            match ctx[k] {
                None => {
                    corner_src.push(Src::Copy(l));
                    corner_verts.push(vert_final[cv[l] as usize]);
                    corner_edges.push(edge_final[ce[l] as usize]);
                }
                Some((v, e)) => {
                    let group: Vec<usize> = (0..n)
                        .filter(|&q| matches!(ctx[q], Some((qv, _)) if qv == v))
                        .map(|q| start + q)
                        .collect();
                    let w = 1.0 / group.len() as f32;
                    corner_src.push(if group.len() == 1 {
                        Src::Copy(group[0])
                    } else {
                        Src::Mix(group.iter().map(|&g| (g, w)).collect())
                    });
                    corner_verts.push(vert_final[v as usize]);
                    corner_edges.push(edge_final[e as usize]);
                }
            }
        }
        face_offsets.push(corner_verts.len() as i32);
    }
    let topo = Topology {
        edges: new_edges,
        face_offsets,
        corner_verts,
        corner_edges,
    };
    let src = Sources {
        vert: vert_src,
        edge: edge_src,
        face: face_src,
        corner: corner_src,
    };
    rebuild(mesh, topo, &src)
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> ScrewSettings {
        let b = |k: &str| p[k].as_bool().unwrap();
        let f = |k: &str| p[k].as_f64().unwrap() as f32;
        let i = |k: &str| p[k].as_i64().unwrap() as i32;
        ScrewSettings {
            object: None,
            steps: i("steps"),
            render_steps: i("render_steps"),
            iterations: i("iterations"),
            axis: p["axis"].as_str().unwrap().into(),
            angle: f("angle"),
            screw_offset: f("screw_offset"),
            merge_threshold: f("merge_threshold"),
            use_normal_flip: b("use_normal_flip"),
            use_normal_calculate: b("use_normal_calculate"),
            use_object_screw_offset: b("use_object_screw_offset"),
            use_merge_vertices: b("use_merge_vertices"),
            use_smooth_shade: b("use_smooth_shade"),
            use_stretch_u: b("use_stretch_u"),
            use_stretch_v: b("use_stretch_v"),
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/screw.json`): exact
    /// element order (T1).
    #[test]
    fn matches_blender() {
        let d = load("vgroups/screw.json");
        for case in d["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            eprintln!("screw case {name}");
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = screw(&s, None, false, &mesh).unwrap();
            let want = &case["output"];
            assert_topology(&out, want);
            assert_positions(&out, want, 2e-5);
            assert_uv(&out, want, 2e-5);
            assert_attribute(&out, want, "sharp_face", 0.0);
            assert_attribute(&out, want, "material_index", 0.0);
            assert_corner_normals(&out, want, 1e-4);
        }
    }

    /// A 4-vertex profile from the axis out and back.
    fn profile() -> Mesh {
        let mut m = Mesh::new("P");
        m.set_positions(vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.5],
            [1.0, 0.0, 1.5],
            [0.0, 0.0, 2.0],
        ]);
        m.set_topology(vec![0], vec![], vec![[0, 1], [1, 2], [2, 3]]);
        m
    }

    #[test]
    fn full_turn_closes_and_merges_the_axis() {
        let s = ScrewSettings {
            steps: 8,
            use_merge_vertices: true,
            ..ScrewSettings::default()
        };
        let out = screw(&s, None, false, &profile()).unwrap();
        // 2 axis vertices + 2 ring vertices × 8.
        assert_eq!(out.verts_num(), 2 + 2 * 8);
        // 8 quads + 16 triangles.
        assert_eq!(out.faces_num(), 24);
        let s = ScrewSettings {
            steps: 8,
            ..ScrewSettings::default()
        };
        let out = screw(&s, None, false, &profile()).unwrap();
        assert_eq!(out.verts_num(), 4 * 8);
        assert_eq!(out.faces_num(), 3 * 8);
        assert_eq!(out.edges_num(), 4 * 8 + 3 * 8);
    }

    /// A 32-vertex circle (edges only).
    fn circle() -> Mesh {
        let n = 32;
        let mut m = Mesh::new("C");
        m.set_positions(
            (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    [libm::cosf(a) + 2.0, 0.0, libm::sinf(a)]
                })
                .collect(),
        );
        m.set_topology(vec![0], vec![], (0..n).map(|i| [i, (i + 1) % n]).collect());
        m
    }

    /// Settings within RNA range whose output does not fit Blender's `int` range fail with the
    /// `LIMIT` message instead of overflowing (ARCHITECTURE §12).
    #[test]
    fn huge_steps_times_iterations_is_a_limit_error() {
        let s = ScrewSettings {
            steps: 10000,
            render_steps: 10000,
            iterations: 10000,
            ..ScrewSettings::default()
        };
        for render in [false, true] {
            let Err(ModError::Error(msg)) = screw(&s, None, render, &circle()) else {
                panic!("expected a limit error");
            };
            assert!(msg.contains("limit exceeded (max_elements)"), "{msg}");
        }
    }

    /// `--max-elements` rejects a moderate screw before it is built.
    #[test]
    fn max_elements_budget_rejects_a_moderate_screw() {
        let s = ScrewSettings {
            steps: 100,
            iterations: 100,
            ..ScrewSettings::default()
        };
        let _scope = frap_math::budget::scope(frap_math::budget::Budget {
            timeout: None,
            max_elements: 1000,
        });
        let Err(ModError::Error(msg)) = screw(&s, None, false, &circle()) else {
            panic!("expected a limit error");
        };
        assert!(msg.contains("(limit 1000)"), "{msg}");
        // A small one still fits.
        let s = ScrewSettings {
            steps: 4,
            ..ScrewSettings::default()
        };
        assert!(screw(&s, None, false, &circle()).is_ok());
    }
}
