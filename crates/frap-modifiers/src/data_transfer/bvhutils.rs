// SPDX-License-Identifier: GPL-3.0-or-later
//! BVH trees of a mesh's vertices, edges or corner triangles with Blender's query callbacks
//! (`BVHTreeFromMesh`): the nearest point on a primitive and the (sphere-)ray hit.
//!
//! Ported from blender/source/blender/blenkernel/intern/bvhutils.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `bvhtree_new_common` (`BLI_bvhtree_new(n, 0, 2, 6)`), the
//! `create_tree_from_{verts,edges,tris}` builders with their masks, and the callbacks
//! `mesh_corner_tris_nearest_point`, `mesh_corner_tris_spherecast`, `mesh_edges_nearest_point`,
//! `mesh_edges_spherecast`, `mesh_verts_spherecast` (a vertex tree has no nearest callback: the
//! leaf bounds are the point). The tree itself is `frap_geom::bvh` (WP 8.5).

use frap_geom::bvh::{BvhRay, BvhTree, Nearest, RayHit, isect_ray_tri_watertight_v3};

use super::math::{
    V3, add, closest_on_tri_to_point, closest_to_line_segment, dot_v3v3v3, isect_line_line,
    isect_sweeping_sphere_tri, len_sq_v3v3, len_v3v3, line_point_factor, madd, normal_tri,
    normalized, sub,
};

/// What the tree's primitives are.
#[derive(Clone, Copy)]
enum Kind<'a> {
    /// One leaf per vertex (`bvh_verts`).
    Verts,
    /// One leaf per edge (`bvh_edges`).
    Edges(&'a [[i32; 2]]),
    /// One leaf per corner triangle (`bvh_corner_tris`).
    Tris {
        corner_verts: &'a [i32],
        tris: &'a [[u32; 3]],
    },
}

/// `bke::BVHTreeFromMesh`.
pub(crate) struct MeshTree<'a> {
    tree: Option<BvhTree>,
    positions: &'a [V3],
    kind: Kind<'a>,
}

/// `bvhtree_new_common`: no tree without elements.
fn new_common(n: usize) -> Option<BvhTree> {
    (n > 0).then(|| BvhTree::new(n, 0.0, 2, 6))
}

impl<'a> MeshTree<'a> {
    /// `bvhtree_from_mesh_verts_ex`: the vertices of `mask` (all without one).
    pub(crate) fn verts(positions: &'a [V3], mask: Option<&[bool]>) -> MeshTree<'a> {
        let n = mask.map_or(positions.len(), |m| m.iter().filter(|x| **x).count());
        let mut tree = new_common(n);
        if let Some(t) = tree.as_mut() {
            for (i, p) in positions.iter().enumerate() {
                if mask.is_none_or(|m| m[i]) {
                    t.insert(i, std::slice::from_ref(p));
                }
            }
            t.balance();
        }
        MeshTree {
            tree,
            positions,
            kind: Kind::Verts,
        }
    }

    /// `bvhtree_from_mesh_edges_ex` over every edge.
    pub(crate) fn edges(positions: &'a [V3], edges: &'a [[i32; 2]]) -> MeshTree<'a> {
        let mut tree = new_common(edges.len());
        if let Some(t) = tree.as_mut() {
            for (i, e) in edges.iter().enumerate() {
                t.insert(i, &[positions[e[0] as usize], positions[e[1] as usize]]);
            }
            t.balance();
        }
        MeshTree {
            tree,
            positions,
            kind: Kind::Edges(edges),
        }
    }

    /// `bvhtree_from_mesh_corner_tris_ex`: the triangles of the faces of `faces_mask` (all
    /// without one), `face_offsets` giving each face's triangles
    /// (`face_triangles_range`: `offset − 2·face`, `size − 2` triangles).
    pub(crate) fn tris(
        positions: &'a [V3],
        corner_verts: &'a [i32],
        tris: &'a [[u32; 3]],
        face_offsets: &[i32],
        faces_mask: Option<&[bool]>,
    ) -> MeshTree<'a> {
        let faces_num = face_offsets.len().saturating_sub(1);
        let tri_co = |t: usize| {
            let tri = tris[t];
            [
                positions[corner_verts[tri[0] as usize] as usize],
                positions[corner_verts[tri[1] as usize] as usize],
                positions[corner_verts[tri[2] as usize] as usize],
            ]
        };
        let full = faces_mask.is_none_or(|m| m.iter().filter(|x| **x).count() == faces_num);
        let tree = if full {
            let mut tree = new_common(tris.len());
            if let Some(t) = tree.as_mut() {
                for i in 0..tris.len() {
                    t.insert(i, &tri_co(i));
                }
                t.balance();
            }
            tree
        } else {
            let mask = faces_mask.unwrap_or(&[]);
            let tri_range = |f: usize| {
                let start = face_offsets[f] as usize - 2 * f;
                let size = (face_offsets[f + 1] - face_offsets[f]) as usize;
                start..start + size.saturating_sub(2)
            };
            let n: usize = (0..faces_num)
                .filter(|&f| mask[f])
                .map(|f| tri_range(f).len())
                .sum();
            let mut tree = new_common(n);
            if let Some(t) = tree.as_mut() {
                for f in (0..faces_num).filter(|&f| mask[f]) {
                    for i in tri_range(f) {
                        t.insert(i, &tri_co(i));
                    }
                }
                t.balance();
            }
            tree
        };
        MeshTree {
            tree,
            positions,
            kind: Kind::Tris { corner_verts, tris },
        }
    }

    fn tri(&self, corner_verts: &[i32], tris: &[[u32; 3]], index: usize) -> [V3; 3] {
        let t = tris[index];
        [
            self.positions[corner_verts[t[0] as usize] as usize],
            self.positions[corner_verts[t[1] as usize] as usize],
            self.positions[corner_verts[t[2] as usize] as usize],
        ]
    }

    /// `BLI_bvhtree_find_nearest(tree, co, nearest, nearest_callback, data)`: updates
    /// `nearest` (its `dist_sq` bounds the search).
    pub(crate) fn find_nearest(&self, co: V3, nearest: &mut Nearest) {
        let Some(tree) = &self.tree else {
            return;
        };
        match self.kind {
            Kind::Verts => {
                tree.find_nearest_ex(co, nearest, None, false);
            }
            Kind::Edges(edges) => {
                let mut cb = |index: usize, co: V3, n: &mut Nearest| {
                    let e = edges[index];
                    let t0 = self.positions[e[0] as usize];
                    let t1 = self.positions[e[1] as usize];
                    let nearest_tmp = closest_to_line_segment(co, t0, t1);
                    let dist_sq = len_sq_v3v3(nearest_tmp, co);
                    if dist_sq < n.dist_sq {
                        n.index = Some(index);
                        n.dist_sq = dist_sq;
                        n.co = nearest_tmp;
                        n.no = normalized(sub(t0, t1));
                    }
                };
                tree.find_nearest_ex(co, nearest, Some(&mut cb), false);
            }
            Kind::Tris { corner_verts, tris } => {
                let mut cb = |index: usize, co: V3, n: &mut Nearest| {
                    let [a, b, c] = self.tri(corner_verts, tris, index);
                    let nearest_tmp = closest_on_tri_to_point(co, a, b, c);
                    let dist_sq = len_sq_v3v3(co, nearest_tmp);
                    if dist_sq < n.dist_sq {
                        n.index = Some(index);
                        n.dist_sq = dist_sq;
                        n.co = nearest_tmp;
                        n.no = normal_tri(a, b, c);
                    }
                };
                tree.find_nearest_ex(co, nearest, Some(&mut cb), false);
            }
        }
    }

    /// `BLI_bvhtree_ray_cast(tree, co, dir, radius, hit, raycast_callback, data)` (watertight):
    /// updates `hit` (its `dist` bounds the search).
    pub(crate) fn ray_cast(&self, co: V3, dir: V3, radius: f32, hit: &mut RayHit) {
        let Some(tree) = &self.tree else {
            return;
        };
        match self.kind {
            Kind::Verts => {
                let mut cb = |index: usize, ray: &BvhRay, h: &mut RayHit| {
                    verts_spherecast_do(index, self.positions[index], ray, h);
                };
                tree.ray_cast_ex(co, dir, radius, hit, Some(&mut cb), true);
            }
            Kind::Edges(edges) => {
                let mut cb = |index: usize, ray: &BvhRay, h: &mut RayHit| {
                    let e = edges[index];
                    let v1 = self.positions[e[0] as usize];
                    let v2 = self.positions[e[1] as usize];
                    edges_spherecast(index, v1, v2, ray, h);
                };
                tree.ray_cast_ex(co, dir, radius, hit, Some(&mut cb), true);
            }
            Kind::Tris { corner_verts, tris } => {
                let mut cb = |index: usize, ray: &BvhRay, h: &mut RayHit| {
                    let [a, b, c] = self.tri(corner_verts, tris, index);
                    let dist = if ray.radius == 0.0 {
                        ray_tri_intersection(ray, a, b, c)
                    } else {
                        sphereray_tri_intersection(ray, ray.radius, h.dist, a, b, c)
                    };
                    if dist >= 0.0 && dist < h.dist {
                        h.index = Some(index);
                        h.dist = dist;
                        h.co = madd(ray.origin, ray.direction, dist);
                        h.no = normal_tri(a, b, c);
                    }
                };
                tree.ray_cast_ex(co, dir, radius, hit, Some(&mut cb), true);
            }
        }
    }
}

/// `bvhtree_ray_tri_intersection` (watertight): the distance, `FLT_MAX` for a miss.
fn ray_tri_intersection(ray: &BvhRay, v0: V3, v1: V3, v2: V3) -> f32 {
    let Some(pre) = ray.isect_precalc.as_ref() else {
        return f32::MAX;
    };
    match isect_ray_tri_watertight_v3(ray.origin, pre, v0, v1, v2) {
        Some((dist, _)) => dist,
        None => f32::MAX,
    }
}

/// `bvhtree_sphereray_tri_intersection`
fn sphereray_tri_intersection(
    ray: &BvhRay,
    radius: f32,
    m_dist: f32,
    v0: V3,
    v1: V3,
    v2: V3,
) -> f32 {
    let p1 = madd(ray.origin, ray.direction, m_dist);
    match isect_sweeping_sphere_tri(ray.origin, p1, radius, v0, v1, v2) {
        Some((idist, _)) => idist * m_dist,
        None => f32::MAX,
    }
}

/// `mesh_verts_spherecast_do`
fn verts_spherecast_do(index: usize, v: V3, ray: &BvhRay, hit: &mut RayHit) {
    let r1 = ray.origin;
    let r2 = add(r1, ray.direction);
    let i1 = closest_to_line_segment(v, r1, r2);
    if dot_v3v3v3(r1, i1, r2) >= 0.0 {
        let dist = len_v3v3(r1, i1);
        if dist < hit.dist {
            hit.index = Some(index);
            hit.dist = dist;
            hit.co = i1;
        }
    }
}

/// `mesh_edges_spherecast`
fn edges_spherecast(index: usize, v1: V3, v2: V3, ray: &BvhRay, hit: &mut RayHit) {
    let radius_sq = ray.radius * ray.radius;
    if v1 == v2 {
        verts_spherecast_do(index, v1, ray, hit);
        return;
    }
    let r1 = ray.origin;
    let r2 = add(r1, ray.direction);
    let (n, mut i1, i2) = isect_line_line(v1, v2, r1, r2);
    if n != 0 && dot_v3v3v3(r1, i2, r2) >= 0.0 {
        let dist = len_v3v3(r1, i2);
        if dist < hit.dist {
            let e_fac = line_point_factor(i1, v1, v2);
            if e_fac < 0.0 {
                i1 = v1;
            } else if e_fac > 1.0 {
                i1 = v2;
            }
            if len_sq_v3v3(i1, i2) <= radius_sq {
                hit.index = Some(index);
                hit.dist = dist;
                hit.co = i2;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> (Vec<V3>, Vec<i32>, Vec<[u32; 3]>, Vec<i32>) {
        // Two triangles of the unit square in z = 0.
        let positions = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let corner_verts = vec![0, 1, 2, 3];
        let tris = vec![[0, 1, 2], [0, 2, 3]];
        (positions, corner_verts, tris, vec![0, 4])
    }

    #[test]
    fn nearest_on_triangles_edges_and_vertices() {
        let (p, cv, tris, offsets) = grid();
        let t = MeshTree::tris(&p, &cv, &tris, &offsets, None);
        let mut n = Nearest::within(f32::MAX);
        t.find_nearest([0.8, 0.2, 2.0], &mut n);
        assert_eq!(n.index, Some(0));
        assert_eq!(n.co, [0.8, 0.2, 0.0]);
        assert!((n.dist_sq - 4.0).abs() < 1e-6);

        let edges = vec![[0, 1], [1, 2]];
        let t = MeshTree::edges(&p, &edges);
        let mut n = Nearest::within(f32::MAX);
        t.find_nearest([1.5, 0.5, 0.0], &mut n);
        assert_eq!(n.index, Some(1));
        assert_eq!(n.co, [1.0, 0.5, 0.0]);

        let t = MeshTree::verts(&p, Some(&[false, true, true, true]));
        let mut n = Nearest::within(f32::MAX);
        t.find_nearest([0.1, 0.0, 0.0], &mut n);
        assert_eq!(n.index, Some(1));
    }

    #[test]
    fn rays_and_sphere_rays() {
        let (p, cv, tris, offsets) = grid();
        let t = MeshTree::tris(&p, &cv, &tris, &offsets, None);
        let mut h = RayHit::within(f32::MAX);
        t.ray_cast([0.25, 0.75, 1.0], [0.0, 0.0, -1.0], 0.0, &mut h);
        assert_eq!(h.index, Some(1));
        assert!((h.dist - 1.0).abs() < 1e-6);
        // Beside the square: only the sphere ray reaches it.
        let mut h = RayHit::within(10.0);
        t.ray_cast([1.05, 0.5, 1.0], [0.0, 0.0, -1.0], 0.0, &mut h);
        assert_eq!(h.index, None);
        let mut h = RayHit::within(10.0);
        t.ray_cast([1.05, 0.5, 1.0], [0.0, 0.0, -1.0], 0.1, &mut h);
        assert_eq!(h.index, Some(0));
    }

    #[test]
    fn nearest_triangle_equals_brute_force_on_a_torus() {
        let m = frap_mesh::primitives::torus::torus(&frap_mesh::primitives::torus::Torus {
            major_segments: 12,
            minor_segments: 6,
            ..Default::default()
        });
        let tris = m.looptris();
        let t = MeshTree::tris(
            m.positions(),
            m.corner_verts(),
            &tris,
            m.face_offsets(),
            None,
        );
        let p = m.positions();
        let cv = m.corner_verts();
        let mut seed = 7u32;
        let mut rnd = || {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            (seed >> 8) as f32 / (1u32 << 24) as f32 * 3.0 - 1.5
        };
        for _ in 0..300 {
            let co = [rnd(), rnd(), rnd() * 0.4];
            let mut n = Nearest::within(f32::MAX);
            t.find_nearest(co, &mut n);
            let mut best = f32::MAX;
            for tri in tris.iter() {
                let v = |k: usize| p[cv[tri[k] as usize] as usize];
                let h = closest_on_tri_to_point(co, v(0), v(1), v(2));
                best = best.min(len_sq_v3v3(co, h));
            }
            assert_eq!(n.dist_sq, best, "{co:?}");
        }
    }

    #[test]
    fn masked_triangles_keep_their_global_indices() {
        let p = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [5.0, 0.0, 0.0],
            [6.0, 0.0, 0.0],
            [5.0, 1.0, 0.0],
        ];
        let cv = vec![0, 1, 2, 3, 4, 5, 6];
        let tris = vec![[0, 1, 2], [0, 2, 3], [4, 5, 6]];
        let offsets = vec![0, 4, 7];
        let t = MeshTree::tris(&p, &cv, &tris, &offsets, Some(&[false, true]));
        let mut n = Nearest::within(f32::MAX);
        t.find_nearest([0.0, 0.0, 0.0], &mut n);
        assert_eq!(n.index, Some(2));
    }
}
