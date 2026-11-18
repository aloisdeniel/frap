// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_cone_add` geometry (ARCHITECTURE §6.1, tier T1); also the engine of
//! `mesh.primitive_cylinder_add` (`radius1 = radius2 = radius`) and of the circle's cap fill.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (`bmo_create_cone_exec`,
//! `BM_mesh_calc_uvs_cone`) and editmesh_add.cc (`add_primitive_cone_exec`: `end_fill_type`
//! NOTHING/NGON/TRIFAN → `cap_ends`/`cap_tris`; the creation matrix includes `scale`).
//!
//! The post-processing BMesh operations are reproduced by their effect on element order, which
//! was derived from Blender's source and checked against Blender 5.2.4 for 3–13 segments, every
//! fill type and radius combination (`tests/primitives_vs_blender.rs`):
//! - a zero radius kills that end's cap centre (`BM_vert_kill`: its spokes in creation order,
//!   each spoke's faces in creation order) and collapses the end ring onto one vertex
//!   (`BM_edge_collapse` per side face: radius1 merges `v1[i]` into `v1[i-1]`, radius2 merges
//!   `v2[i-1]` into `v2[i]`); the loop of the killed vertex leaves each face (taking `l_first`
//!   with it moves `l_first` to the surviving loop), duplicate edges keep the first-created one;
//! - NGON caps (`dissolve_faces`) replace each remaining fan by one face: the bottom fan first,
//!   its n-gon taking the most recently freed face slot (or a new one), after which the fan's
//!   faces are freed in walk order (the second-to-last fan face last, except for 3 segments
//!   where the closing face is last).

use std::collections::HashMap;

use super::Matrix4;
use super::bm::math::{invert_m4, mul_m4_v3, mul_mat3_m4_v3, sin_cos_from_fraction};
use super::bm::{Bm, Face, V3};
use super::trace::ConeTopology;
use crate::mesh::Mesh;
use frap_geom::normals::{dot, face_normal_calc, normalize_v3};

/// `end_fill_type` / `fill_type` of the cone, cylinder and circle operators.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum FillType {
    /// `'NOTHING'`: open ends.
    Nothing,
    /// `'NGON'`: one n-gon per end (default of cone and cylinder).
    #[default]
    Ngon,
    /// `'TRIFAN'`: triangle fans around a centre vertex.
    Trifan,
}

/// Operator properties of `mesh.primitive_cone_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cone {
    /// `vertices`.
    pub vertices: i32,
    /// `radius1` (bottom).
    pub radius1: f32,
    /// `radius2` (top).
    pub radius2: f32,
    /// `depth`.
    pub depth: f32,
    /// `end_fill_type`.
    pub end_fill_type: FillType,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for Cone {
    fn default() -> Self {
        Cone {
            vertices: 32,
            radius1: 1.0,
            radius2: 0.0,
            depth: 2.0,
            end_fill_type: FillType::Ngon,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl Cone {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        // Upper bound: a zero radius merges its ring into one vertex.
        super::tube_estimate(self.vertices, self.end_fill_type)
    }
}

/// The mesh of `mesh.primitive_cone_add(…)`.
pub fn cone(p: &Cone) -> Mesh {
    create_cone(
        "Cone",
        p.vertices,
        p.radius1,
        p.radius2,
        p.depth,
        p.end_fill_type,
        p.calc_uvs,
        &p.matrix,
    )
}

/// `bmo_create_cone_exec` + `dissolve_faces`, named `name`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_cone(
    name: &str,
    segs: i32,
    rad1: f32,
    rad2: f32,
    depth: f32,
    fill: FillType,
    calc_uvs: bool,
    mat: &Matrix4,
) -> Mesh {
    let cap_ends = fill != FillType::Nothing;
    let cap_tris = fill == FillType::Trifan;
    let mut bm = Bm::new();
    if segs <= 0 {
        return bm.to_mesh(name, calc_uvs);
    }
    let topology = ConeTopology {
        segments: segs as u32,
        radius1_zero: rad1 == 0.0,
        radius2_zero: rad2 == 0.0,
        cap_ends,
        cap_tris,
    };
    bm.cone = Some(topology);
    let segs_u = segs as usize;
    let depth_half = 0.5 * depth;
    let mut cent = [None, None];
    if cap_ends {
        cent[0] = Some(bm.vert_create(mul_m4_v3(mat, [0.0, 0.0, -depth_half])));
        cent[1] = Some(bm.vert_create(mul_m4_v3(mat, [0.0, 0.0, depth_half])));
    }
    let mut ring1 = Vec::with_capacity(segs_u);
    let mut ring2 = Vec::with_capacity(segs_u);
    let mut sides = Vec::with_capacity(segs_u);
    let mut fan = [Vec::new(), Vec::new()];
    for i in 0..segs {
        let (sin_phi, cos_phi) = sin_cos_from_fraction(i, segs);
        let v1 = bm.vert_create(mul_m4_v3(
            mat,
            [rad1 * sin_phi, rad1 * cos_phi, -depth_half],
        ));
        let v2 = bm.vert_create(mul_m4_v3(mat, [rad2 * sin_phi, rad2 * cos_phi, depth_half]));
        if i > 0 {
            let (lv1, lv2) = (*ring1.last().unwrap(), *ring2.last().unwrap());
            if let [Some(c1), Some(c2)] = cent {
                fan[0].push(bm.face_create(&[c1, lv1, v1]));
                fan[1].push(bm.face_create(&[c2, v2, lv2]));
            }
            sides.push(bm.face_create(&[lv1, lv2, v2, v1]));
        }
        ring1.push(v1);
        ring2.push(v2);
    }
    let (v1, v2) = (ring1[segs_u - 1], ring2[segs_u - 1]);
    let (fv1, fv2) = (ring1[0], ring2[0]);
    if let [Some(c1), Some(c2)] = cent {
        fan[0].push(bm.face_create(&[c1, v1, fv1]));
        fan[1].push(bm.face_create(&[c2, fv2, v2]));
    }
    bm.face_create(&[v1, v2, fv2, fv1]);

    if calc_uvs {
        calc_uvs_cone(&mut bm, mat, rad2, rad1, segs, cap_ends);
    }

    if rad1 == 0.0 && rad2 == 0.0 {
        return both_ends_collapsed(&bm, &ring1, &ring2, name, calc_uvs, topology);
    }

    let mut merges = Merges::new(&bm);
    if rad1 == 0.0 {
        if let Some(c1) = cent[0] {
            kill_fan_centre(&mut bm, c1);
            cent[0] = None;
        }
        for i in 1..segs_u {
            merges.merge(ring1[i], ring1[i - 1]);
        }
    }
    if rad2 == 0.0 {
        if let Some(c2) = cent[1] {
            kill_fan_centre(&mut bm, c2);
            cent[1] = None;
        }
        for i in 1..segs_u {
            merges.merge(ring2[i - 1], ring2[i]);
        }
    }
    merges.apply(&mut bm);

    if !cap_tris {
        // Bottom fan first (its first face precedes the top's), then the top fan.
        if let Some(c1) = cent[0] {
            let ring: Vec<u32> = ring1.iter().map(|&v| merges.find(v)).collect();
            dissolve_fan(&mut bm, c1, &ring);
        }
        if let Some(c2) = cent[1] {
            let mut ring = vec![ring2[1 % segs_u], ring2[0]];
            ring.extend(ring2[2.min(segs_u)..].iter().rev());
            let ring: Vec<u32> = ring.iter().map(|&v| merges.find(v)).collect();
            dissolve_fan(&mut bm, c2, &ring);
        }
    }
    bm.to_mesh(name, calc_uvs)
}

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc (BM_mesh_calc_uvs_cone).
fn calc_uvs_cone(
    bm: &mut Bm,
    mat: &Matrix4,
    radius_top: f32,
    radius_bottom: f32,
    segments: i32,
    cap_ends: bool,
) {
    let uv_width = 1.0f32 / segments as f32;
    let uv_height = if cap_ends { 0.5f32 } else { 1.0 };
    let uv_center_y = if cap_ends { 0.25f32 } else { 0.5 };
    let uv_center_x_top = if cap_ends { 0.25f32 } else { 0.5 };
    let uv_center_x_bottom = if cap_ends { 0.75f32 } else { 0.5 };
    let uv_radius = if cap_ends { 0.24f32 } else { 0.5 };
    let uv_scale_top = if radius_top != 0.0 {
        uv_radius / radius_top
    } else if radius_bottom != 0.0 {
        uv_radius / radius_bottom
    } else {
        uv_radius
    };
    let uv_scale_bottom = if radius_bottom != 0.0 {
        uv_radius / radius_bottom
    } else {
        uv_scale_top
    };
    let local_up = normalize_v3(mul_mat3_m4_v3(mat, [0.0, 0.0, 1.0])).0;
    let inv_mat = invert_m4(mat);
    let mut x = 1.0f32;
    let mut y = 1.0f32 - uv_height;
    let positions: Vec<V3> = bm
        .verts
        .slots
        .iter()
        .map(|v| v.unwrap_or([0.0; 3]))
        .collect();
    for slot in bm.faces.slots.iter_mut() {
        let Some(f) = slot.as_mut() else { continue };
        if f.verts.len() == 4 && radius_top != 0.0 && radius_bottom != 0.0 {
            for (loop_index, uv) in f.uvs.iter_mut().enumerate() {
                match loop_index {
                    1 => y += uv_height,
                    2 => x -= uv_width,
                    3 => y -= uv_height,
                    _ => {}
                }
                *uv = [x, y];
            }
        } else {
            let verts: Vec<i32> = f.verts.iter().map(|&v| v as i32).collect();
            let no = face_normal_calc(&positions, &verts);
            let top = dot(no, local_up) > 0.0;
            for (k, uv) in f.uvs.iter_mut().enumerate() {
                let co = mul_m4_v3(&inv_mat, positions[f.verts[k] as usize]);
                *uv = if top {
                    [
                        uv_center_x_top + co[0] * uv_scale_top,
                        uv_center_y + co[1] * uv_scale_top,
                    ]
                } else {
                    [
                        uv_center_x_bottom + co[0] * uv_scale_bottom,
                        uv_center_y + co[1] * uv_scale_bottom,
                    ]
                };
            }
        }
    }
}

/// `BM_vert_kill(centre)` of a fan: spokes in creation (disk) order, each spoke's live faces in
/// creation order; then the spokes and the centre.
pub(crate) fn kill_fan_centre(bm: &mut Bm, centre: u32) {
    let spokes: Vec<(u32, [u32; 2])> = bm
        .edges
        .iter()
        .filter(|(_, e)| e.contains(&centre))
        .map(|(i, e)| (i, *e))
        .collect();
    for &(_, [a, b]) in &spokes {
        let faces: Vec<u32> = bm
            .faces
            .iter()
            .filter(|(_, f)| f.verts.contains(&a) && f.verts.contains(&b))
            .map(|(i, _)| i)
            .collect();
        for f in faces {
            bm.face_kill(f);
        }
    }
    for (e, _) in spokes {
        bm.edge_kill_only(e);
    }
    bm.vert_kill_only(centre);
}

/// Vertex merges recorded in collapse order, applied at once.
pub(crate) struct Merges {
    parent: Vec<u32>,
}

impl Merges {
    pub fn new(bm: &Bm) -> Merges {
        Merges {
            parent: (0..bm.verts.slots.len() as u32).collect(),
        }
    }
    /// `BM_edge_collapse(bm, e(kill, into), kill)`.
    pub fn merge(&mut self, kill: u32, into: u32) {
        self.parent[kill as usize] = into;
    }
    pub fn find(&self, mut v: u32) -> u32 {
        while self.parent[v as usize] != v {
            v = self.parent[v as usize];
        }
        v
    }
    fn is_root(&self, v: u32) -> bool {
        self.parent[v as usize] == v
    }

    /// Rewrites faces (removing the loop of the killed vertex of every collapsed pair; faces
    /// left with fewer than 3 loops die), edges (re-pointed, degenerate ones and later
    /// duplicates removed) and kills merged vertices.
    pub fn apply(&self, bm: &mut Bm) {
        let face_ids: Vec<u32> = bm.faces.iter().map(|(i, _)| i).collect();
        for fi in face_ids {
            let f = bm.faces.get(fi).expect("live face").clone();
            let mut verts = f.verts.clone();
            let mut uvs = f.uvs.clone();
            loop {
                let n = verts.len();
                if n < 2 {
                    break;
                }
                let mut removed = false;
                for i in 0..n {
                    let j = (i + 1) % n;
                    let (a, b) = (verts[i], verts[j]);
                    if self.find(a) != self.find(b) {
                        continue;
                    }
                    // The killed vertex of the collapsed pair: the non-surviving one, or the one
                    // merged directly into the other.
                    let a_dies =
                        !self.is_root(a) && (self.is_root(b) || self.parent[a as usize] == b);
                    let b_dies =
                        !self.is_root(b) && (self.is_root(a) || self.parent[b as usize] == a);
                    let kill_a = a_dies || !b_dies;
                    let (kill, keep) = if kill_a { (i, j) } else { (j, i) };
                    if kill == 0 {
                        // `l_first` moves to the surviving loop.
                        verts.rotate_left(keep);
                        uvs.rotate_left(keep);
                        let k = (kill + n - keep) % n;
                        verts.remove(k);
                        uvs.remove(k);
                    } else {
                        verts.remove(kill);
                        uvs.remove(kill);
                    }
                    removed = true;
                    break;
                }
                if !removed {
                    break;
                }
            }
            if verts.len() < 3 {
                bm.face_kill(fi);
            } else {
                let verts = verts.iter().map(|&v| self.find(v)).collect();
                *bm.faces.get_mut(fi).expect("live face") = Face { verts, uvs };
            }
        }
        let edge_ids: Vec<(u32, [u32; 2])> = bm.edges.iter().map(|(i, e)| (i, *e)).collect();
        let mut seen: HashMap<(u32, u32), ()> = HashMap::new();
        for (ei, [a, b]) in edge_ids {
            let (a2, b2) = (self.find(a), self.find(b));
            if a2 == b2 || seen.insert(super::bm::ekey(a2, b2), ()).is_some() {
                bm.edge_kill_only(ei);
            } else if (a2, b2) != (a, b) {
                bm.edge_set_verts(ei, a2, b2);
            }
        }
        for v in 0..self.parent.len() as u32 {
            if !self.is_root(v) {
                bm.vert_kill_only(v);
            }
        }
    }
}

/// `dissolve_faces` on one fan around `centre`: an n-gon over `ring` (already in the n-gon's
/// winding and start corner) takes the most recently freed face slot (or a new one); then the
/// fan's faces die in walk order, the spokes and the centre with them. Each n-gon corner
/// copies the UV of the fan loop on the same directed boundary edge.
pub(crate) fn dissolve_fan(bm: &mut Bm, centre: u32, ring: &[u32]) {
    let region: Vec<u32> = bm
        .faces
        .iter()
        .filter(|(_, f)| f.verts.contains(&centre))
        .map(|(i, _)| i)
        .collect();
    let mut directed: HashMap<(u32, u32), [f32; 2]> = HashMap::new();
    for &fi in &region {
        let f = bm.faces.get(fi).expect("live face");
        let n = f.verts.len();
        for k in 0..n {
            directed.insert((f.verts[k], f.verts[(k + 1) % n]), f.uvs[k]);
        }
    }
    let n = ring.len();
    let uvs: Vec<[f32; 2]> = (0..n)
        .map(|k| {
            directed
                .get(&(ring[k], ring[(k + 1) % n]))
                .copied()
                .unwrap_or([0.0; 2])
        })
        .collect();
    bm.faces.alloc(Face {
        verts: ring.to_vec(),
        uvs,
    });
    let mut kill = region.clone();
    if kill.len() > 3 {
        let second_last = kill.remove(kill.len() - 2);
        kill.push(second_last);
    }
    for f in kill {
        bm.face_kill(f);
    }
    let spokes: Vec<u32> = bm
        .edges
        .iter()
        .filter(|(_, e)| e.contains(&centre))
        .map(|(i, _)| i)
        .collect();
    for e in spokes {
        bm.edge_kill_only(e);
    }
    bm.vert_kill_only(centre);
}

/// Both radii zero: Blender collapses everything to the two end vertices joined by one edge
/// (verified for every fill type: vertices `[bottom, top]`, edge `(top, bottom)`, no faces).
fn both_ends_collapsed(
    bm: &Bm,
    ring1: &[u32],
    ring2: &[u32],
    name: &str,
    calc_uvs: bool,
    topology: ConeTopology,
) -> Mesh {
    let mut out = Bm::new();
    out.cone = Some(topology);
    let b = out.vert_create(bm.co(ring1[0]));
    let t = out.vert_create(bm.co(*ring2.last().expect("segments > 0")));
    out.edge_create(t, b);
    out.to_mesh(name, calc_uvs)
}
