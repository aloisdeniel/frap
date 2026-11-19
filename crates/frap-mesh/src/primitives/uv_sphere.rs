// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh.primitive_uv_sphere_add` geometry (ARCHITECTURE §6.1).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
//! (`bmo_create_uvsphere_exec`, `BM_mesh_calc_uvs_sphere`): a half-circle profile of
//! `ring_count + 1` vertices (`sin_cos_from_fraction`), extruded and rotated `segments` times by
//! `-2π / segments` about Z around the last profile point (the extrusions copy vertices in
//! profile order; the rotation is `transform_pivot_set_m4` + `mul_m4_v3`, so each ring is the
//! previous ring rotated in float), then `remove_doubles` with a third of the shortest segment
//! as threshold (`frap_geom::kdtree::find_doubles_by_distance`, Blender's kd-tree clustering:
//! which copy of a seam or pole vertex survives depends on the exact float positions), then the
//! creation matrix (operator `scale` included).
//!
//! **Vertex order and positions are Blender's (T1). Edge and face order are not a property of
//! Blender 5.2:** each extrusion creates its faces (and with them the connecting edges between
//! two meridians) in the iteration order of `extrude_edge_only`'s `boundary_map.out`, a
//! pointer-keyed `GHash`, so three runs of the same Blender build give different edge/face
//! orders and edge orientations (verified on 5.2.4; vertex order and the topology itself are
//! stable). frap replays the whole construction ([`blender_order`]): the profile edges, each
//! extrusion's duplicated edges and its faces with their connecting edges, in the `GHash` order
//! a single contiguous `BLI_mempool` chunk gives (`BLI_ghashutil_ptrhash` of 80-byte
//! `BMEdge_OFlag`s from an address that hashes to bucket 0, modulo buckets), then
//! `weld_verts`' edge re-creation (appended in pool order) and in-place face rebuilds. The result
//! is one of the orders Blender produces, so operators that depend on edge order (DECIMATE,
//! SCREW) give Blender's results. Goldens still compare UV-sphere edges and faces canonically
//! (T2, D-19) and vertices index by index.

use super::Matrix4;
use super::bm::Bm;
use super::bm::math::{
    axis_angle_normalized_to_mat3, len_v3, m4_from_m3, mul_m4_v3, mul_v3_m3v3,
    sin_cos_from_fraction, transform_pivot_set_m4,
};
use crate::mesh::Mesh;
use frap_geom::kdtree::find_doubles_by_distance;

/// Operator properties of `mesh.primitive_uv_sphere_add` that shape the mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct UvSphere {
    /// `segments` (values below 1 act as 1).
    pub segments: i32,
    /// `ring_count` (values below 1 act as 1).
    pub ring_count: i32,
    /// `radius`.
    pub radius: f32,
    /// `calc_uvs`.
    pub calc_uvs: bool,
    /// Creation matrix, operator `scale` included.
    pub matrix: Matrix4,
}

impl Default for UvSphere {
    fn default() -> Self {
        UvSphere {
            segments: 32,
            ring_count: 16,
            radius: 1.0,
            calc_uvs: true,
            matrix: super::IDENTITY,
        }
    }
}

impl UvSphere {
    /// Element counts before building (`--max-elements` pre-check, [`super::Estimate`]).
    pub fn estimate(&self) -> Option<super::Estimate> {
        let s = u64::try_from(self.segments.max(1)).ok()?;
        let r = u64::try_from(self.ring_count.max(1)).ok()?;
        if r < 2 {
            // A single ring collapses to its two poles.
            return super::estimate(Some(2), Some(0), Some(0), Some(0));
        }
        // `s` quads per inner ring band, a triangle fan at each pole.
        let quads = s.checked_mul(r - 2)?;
        super::estimate(
            s.checked_mul(r - 1)?.checked_add(2),
            s.checked_mul(2 * r - 1),
            s.checked_mul(r),
            quads.checked_mul(4)?.checked_add(s.checked_mul(6)?),
        )
    }
}

/// The mesh of `mesh.primitive_uv_sphere_add(…)`.
pub fn uv_sphere(p: &UvSphere) -> Mesh {
    let rad = p.radius;
    let seg = p.segments.max(1) as usize;
    let tot = p.ring_count.max(1) as usize;
    let phid = std::f32::consts::PI / tot as f32;

    // Profile (a = 0 top … tot bottom), then `seg` rotated copies.
    let mut profile = Vec::with_capacity(tot + 1);
    let mut vec = [0.0f32; 3];
    for a in 0..=tot {
        let (sin_phi, cos_phi) = sin_cos_from_fraction(a as i32, 2 * tot as i32);
        vec = [0.0, rad * sin_phi, rad * cos_phi];
        profile.push(vec);
    }
    let angle = (-(std::f64::consts::PI * 2.0) / seg as f64) as f32;
    let cmat = axis_angle_normalized_to_mat3([0.0, 0.0, 1.0], angle);
    let rot = transform_pivot_set_m4(&m4_from_m3(&cmat), vec);
    let ring_len = tot + 1;
    let mut coords: Vec<[f32; 3]> = Vec::with_capacity(ring_len * (seg + 1));
    coords.extend_from_slice(&profile);
    for k in 1..=seg {
        for a in 0..ring_len {
            let prev = coords[(k - 1) * ring_len + a];
            coords.push(mul_m4_v3(&rot, prev));
        }
    }

    // remove_doubles with a third of the shortest segment length.
    let len = 2.0 * rad * libm::sinf(phid / 2.0);
    let v0 = [rad * libm::sinf(phid), 0.0, rad * libm::cosf(phid)];
    let v1 = mul_v3_m3v3(&cmat, v0);
    let len2 = len_v3([v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]]);
    let dist = len.min(len2) / 3.0;
    let target: Vec<i32> = find_doubles_by_distance(&coords, dist, None)
        .unwrap_or_else(|| (0..coords.len() as i32).collect());

    let mut bm = Bm::new();
    let mut slot = vec![u32::MAX; coords.len()];
    for (i, co) in coords.iter().enumerate() {
        if target[i] == i as i32 {
            slot[i] = bm.vert_create(*co);
        }
    }
    let (edge_list, face_list) = blender_order(seg, tot, &target);
    for &[a, b] in &edge_list {
        bm.edge_create_always(slot[a], slot[b]);
    }
    let face_list: Vec<Vec<u32>> = face_list
        .iter()
        .map(|f| f.iter().map(|&i| slot[i]).collect())
        .collect();
    for f in &face_list {
        bm.face_create(f);
    }
    if p.calc_uvs {
        calc_uvs_sphere(&mut bm);
    }
    for co in bm.verts.slots.iter_mut().flatten() {
        *co = mul_m4_v3(&p.matrix, *co);
    }
    bm.to_mesh("Sphere", p.calc_uvs)
}

/// `GHash` bucket count after inserting `n` entries into a fresh `BLI_ghash_ptr_new`
/// (`hashsizes`, grown past `GHASH_LIMIT_GROW` = 3/4 load).
fn ghash_buckets(n: usize) -> usize {
    const SIZES: [usize; 27] = [
        5, 11, 17, 37, 67, 131, 257, 521, 1031, 2053, 4099, 8209, 16411, 32771, 65537, 131101,
        262147, 524309, 1048583, 2097169, 4194319, 8388617, 16777259, 33554467, 67108879,
        134217757, 268435459,
    ];
    SIZES
        .iter()
        .copied()
        .find(|&nb| n <= nb * 3 / 4)
        .unwrap_or(SIZES[SIZES.len() - 1])
}

/// Iteration order of a pointer-keyed `GHash` whose `n` keys are the edges in pool slots
/// `first..first + n`, inserted in slot order: buckets in index order, a bucket's entries most
/// recent first. Edges are 80 bytes apart, so `ptrhash` (address `>> 4`) steps by 5.
fn ghash_order(first: usize, n: usize) -> Vec<usize> {
    let nb = ghash_buckets(n);
    let mut keys: Vec<(usize, usize, usize)> =
        (0..n).map(|a| ((5 * (first + a)) % nb, n - a, a)).collect();
    keys.sort_unstable();
    keys.into_iter().map(|(_, _, a)| a).collect()
}

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
// (bmo_create_uvsphere_exec), bmo_extrude.cc (bmo_extrude_edge_only_exec), bmo_dupe.cc
// (bmo_mesh_copy) and bmo_removedoubles.cc (bmo_weld_verts_exec, remdoubles_createface): the
// edge and face creation order of the sphere, over the pre-weld vertex indices
// (`k * (tot + 1) + a`, profile `k = 0`) mapped through `target`.
/// Edges (in pool order, with Blender's orientation) and faces (corner order from `l_first`).
fn blender_order(seg: usize, tot: usize, target: &[i32]) -> (Vec<[usize; 2]>, Vec<Vec<usize>>) {
    if tot < 2 {
        // A single ring collapses to its two poles (the operator's RNA minimum is 3).
        return (Vec::new(), Vec::new());
    }
    let ring_len = tot + 1;
    let vi = |k: usize, a: usize| k * ring_len + a;
    // Pre-weld edges in pool order and faces in creation order.
    let mut pre: Vec<[usize; 2]> = (1..=tot).map(|a| [vi(0, a - 1), vi(0, a)]).collect();
    let mut faces: Vec<[usize; 4]> = Vec::with_capacity(seg * tot);
    let mut src_first = 0;
    for k in 1..=seg {
        // `duplicate`: the source edges' copies, in pool order (no `use_edge_flip_from_face`).
        let dup_first = pre.len();
        pre.extend((0..tot).map(|a| [vi(k, a), vi(k, a + 1)]));
        // Faces over `boundary_map.out`: the source edges have no face, or one whose loop
        // runs `v1 -> v2`, so `edge_normal_flip` is set and the face is
        // `(e.v2, e.v1, e_new.v1, e_new.v2)`; `BM_edges_from_verts_ensure` adds the
        // connecting edge at `e.v2`, then the one at `e.v1`.
        let mut conn = vec![false; ring_len];
        for a in ghash_order(src_first, tot) {
            let (old, new) = (k - 1, k);
            if !conn[a + 1] {
                conn[a + 1] = true;
                pre.push([vi(new, a + 1), vi(old, a + 1)]);
            }
            if !conn[a] {
                conn[a] = true;
                pre.push([vi(old, a), vi(new, a)]);
            }
            faces.push([vi(old, a + 1), vi(old, a), vi(new, a), vi(new, a + 1)]);
        }
        src_first = dup_first;
    }
    // `weld_verts`: an edge with a merged vertex is re-created between the targets (appended,
    // unless it exists or collapses); the old edges are deleted at the end.
    let t = |i: usize| target[i] as usize;
    let merged = |i: usize| t(i) != i;
    let mut exists: std::collections::HashSet<(usize, usize)> = pre
        .iter()
        .filter(|e| !merged(e[0]) && !merged(e[1]))
        .map(|e| (e[0].min(e[1]), e[0].max(e[1])))
        .collect();
    let mut edges: Vec<[usize; 2]> = pre
        .iter()
        .copied()
        .filter(|e| !merged(e[0]) && !merged(e[1]))
        .collect();
    for e in &pre {
        if !merged(e[0]) && !merged(e[1]) {
            continue;
        }
        let (v1, v2) = (t(e[0]), t(e[1]));
        if v1 != v2 && exists.insert((v1.min(v2), v1.max(v2))) {
            edges.push([v1, v2]);
        }
    }
    // Faces rebuilt in place, skipping collapsed corners (`remdoubles_createface`).
    let faces = faces
        .iter()
        .filter_map(|f| {
            let m: Vec<usize> = f.iter().map(|&i| t(i)).collect();
            let mut out = Vec::with_capacity(4);
            for i in 0..4 {
                if m[i] != m[(i + 1) % 4] {
                    out.push(m[i]);
                }
            }
            let mut uniq = out.clone();
            uniq.sort_unstable();
            uniq.dedup();
            (out.len() >= 3 && uniq.len() == out.len()).then_some(out)
        })
        .collect();
    (edges, faces)
}

// Ported from blender/source/blender/bmesh/operators/bmo_primitive.cc
// (bm_mesh_calc_uvs_sphere_face, BM_mesh_calc_uvs_sphere).
fn calc_uvs_sphere(bm: &mut Bm) {
    let pi = std::f32::consts::PI;
    let positions: Vec<[f32; 3]> = bm
        .verts
        .slots
        .iter()
        .map(|v| v.unwrap_or([0.0; 3]))
        .collect();
    for f in bm.faces.slots.iter_mut().flatten() {
        let n = f.verts.len();
        let (mut avgx, mut avgy) = (0.0f32, 0.0f32);
        if n == 3 {
            for &v in &f.verts {
                avgx += positions[v as usize][0];
                avgy += positions[v as usize][1];
            }
        }
        avgx /= 3.0;
        avgy /= 3.0;
        for (k, &v) in f.verts.iter().enumerate() {
            let co = positions[v as usize];
            let (x, y, z) = (co[0], co[1], co[2]);
            let len = len_v3(co);
            let mut theta = if n == 3 && x.abs() < 0.0001 && y.abs() < 0.0001 {
                libm::atan2f(avgy, avgx)
            } else {
                libm::atan2f(y, x)
            };
            if (theta - pi).abs() < 0.0001 {
                theta = -std::f64::consts::PI as f32;
            }
            let phi = libm::acosf((z / len).clamp(-1.0, 1.0));
            f.uvs[k] = [0.5 + theta / (pi * 2.0), 1.0 - phi / pi];
        }
        let mut max_i = 0;
        for k in 1..n {
            if f.uvs[k][0] > f.uvs[max_i][0] {
                max_i = k;
            }
        }
        for k in 0..n {
            if k != max_i {
                let dx = f.uvs[max_i][0] - f.uvs[k][0];
                if dx > 0.5 {
                    f.uvs[k][0] += 1.0;
                }
            }
        }
    }
    let mut minx = 1.0f32;
    for f in bm.faces.slots.iter().flatten() {
        for uv in &f.uvs {
            minx = uv[0].min(minx);
        }
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        for uv in f.uvs.iter_mut() {
            uv[0] -= minx;
        }
    }
}
