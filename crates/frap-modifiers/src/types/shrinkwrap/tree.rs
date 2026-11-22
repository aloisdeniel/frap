// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShrinkwrapTreeData`: a target mesh with its BVH (corner triangles, or vertices for
//! `NEAREST_VERTEX`), normals and boundary data, and the queries SHRINKWRAP runs on it
//! (nearest surface point, target-normal projection, ray casts, the smooth normal at a hit
//! and the snap of a point to the surface).
//!
//! Ported from blender/source/blender/blenkernel/intern/shrinkwrap.cc
//! (`BKE_shrinkwrap_init_tree`, `shrinkwrap_build_boundary_data`, `merge_vert_dir`,
//! `BKE_shrinkwrap_project_normal`, the target-project solver, `target_project_edge`,
//! `mesh_corner_tris_target_project`, `BKE_shrinkwrap_find_nearest_surface`,
//! `BKE_shrinkwrap_compute_smooth_normal`, `shrinkwrap_snap_with_side`,
//! `BKE_shrinkwrap_snap_point_to_surface`), blender/source/blender/blenkernel/intern/
//! bvhutils.cc (`bvhtree_new_common`, `create_tree_from_verts`, `create_tree_from_tris`,
//! `mesh_corner_tris_nearest_point`, `mesh_corner_tris_spherecast`) and
//! blender/source/blender/blenkernel/intern/mesh_tessellate.cc
//! (`corner_tri_get_real_edges`), Blender 5.2.4, GPL-2.0-or-later. The BVH is
//! `frap_geom::bvh` (a port of `BLI_kdopbvh`, WP 8.5), so queries visit and tie-break
//! triangles as Blender's do.

use frap_geom::bvh::isect_ray_tri_watertight_v3;
use frap_geom::bvh::{BVH_RAYCAST_DIST_MAX, BvhRay, BvhTree, Nearest, RayHit};
use frap_mesh::{Mesh, NormalsDomain};

use super::geom::{
    M3, Newton3d, SpaceTransform, V3, closest_on_tri_to_point, cross, dot, interp,
    interp_weights_tri, interp3, len_manhattan, len_sq, madd, newton3d_solve, normal_tri,
    normalized, sub,
};

/// `MOD_SHRINKWRAP_CULL_TARGET_FRONTFACE`
pub(crate) const CULL_FRONTFACE: u8 = 1 << 3;
/// `MOD_SHRINKWRAP_CULL_TARGET_BACKFACE`
pub(crate) const CULL_BACKFACE: u8 = 1 << 4;
/// `MOD_SHRINKWRAP_CULL_TARGET_MASK`
pub(crate) const CULL_MASK: u8 = CULL_FRONTFACE | CULL_BACKFACE;

/// The wrap method (`shrinkType`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Method {
    NearestSurface,
    Project,
    NearestVertex,
    TargetProject,
}

impl Method {
    pub(crate) fn from_identifier(s: &str) -> Method {
        match s {
            "PROJECT" => Method::Project,
            "NEAREST_VERTEX" => Method::NearestVertex,
            "TARGET_PROJECT" => Method::TargetProject,
            _ => Method::NearestSurface,
        }
    }
}

/// The snap mode (`shrinkMode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    OnSurface,
    Inside,
    Outside,
    OutsideSurface,
    AboveSurface,
}

impl Mode {
    pub(crate) fn from_identifier(s: &str) -> Mode {
        match s {
            "INSIDE" => Mode::Inside,
            "OUTSIDE" => Mode::Outside,
            "OUTSIDE_SURFACE" => Mode::OutsideSurface,
            "ABOVE_SURFACE" => Mode::AboveSurface,
            _ => Mode::OnSurface,
        }
    }
}

/// `ShrinkwrapBoundaryVertData`
#[derive(Copy, Clone, Debug, Default)]
struct BoundaryVert {
    /// Average direction of the boundary edges that meet here.
    direction: V3,
    /// The direction closest to `direction` orthogonal to the vertex normal.
    normal_plane: V3,
}

/// `ShrinkwrapBoundaryData` (empty when the mesh has no boundary).
#[derive(Clone, Debug, Default)]
struct Boundary {
    edge_is_boundary: Vec<bool>,
    tri_has_boundary: Vec<bool>,
    vert_boundary_id: Vec<i32>,
    boundary_verts: Vec<BoundaryVert>,
}

impl Boundary {
    fn has_boundary(&self) -> bool {
        !self.edge_is_boundary.is_empty()
    }
}

/// `ShrinkwrapTreeData`
pub(crate) struct Tree {
    bvh: BvhTree,
    positions: Vec<V3>,
    edges: Vec<[i32; 2]>,
    corner_verts: Vec<i32>,
    corner_edges: Vec<i32>,
    corner_tris: Vec<[u32; 3]>,
    tri_faces: Vec<u32>,
    vert_normals: Vec<V3>,
    face_normals: Vec<V3>,
    corner_normals: Vec<V3>,
    /// `sharp_face` (empty without the attribute).
    sharp_faces: Vec<bool>,
    boundary: Option<Boundary>,
    /// A vertex tree (`NEAREST_VERTEX`): no nearest callback (the leaf bounds are the point).
    verts_only: bool,
}

/// `BKE_shrinkwrap_needs_normals`
fn needs_normals(method: Method, mode: Mode) -> bool {
    method == Method::TargetProject
        || (method != Method::NearestVertex && mode == Mode::AboveSurface)
}

impl Tree {
    /// `BKE_shrinkwrap_init_tree`; `None` when the mesh has no vertices (or no faces, except
    /// for `NEAREST_VERTEX`).
    pub(crate) fn new(mesh: &Mesh, method: Method, mode: Mode) -> Option<Tree> {
        if mesh.verts_num() == 0 {
            return None;
        }
        let positions = mesh.positions().to_vec();
        let mut t = Tree {
            bvh: BvhTree::new(1, 0.0, 2, 6),
            positions,
            edges: mesh.edges().to_vec(),
            corner_verts: mesh.corner_verts().to_vec(),
            corner_edges: mesh.corner_edges().to_vec(),
            corner_tris: Vec::new(),
            tri_faces: Vec::new(),
            vert_normals: mesh.vert_normals().to_vec(),
            face_normals: Vec::new(),
            corner_normals: Vec::new(),
            sharp_faces: mesh
                .bool_attribute("sharp_face", frap_mesh::Domain::Face)
                .map(<[bool]>::to_vec)
                .unwrap_or_default(),
            boundary: None,
            verts_only: false,
        };
        if method == Method::NearestVertex {
            // `create_tree_from_verts` (`bvhtree_new_common`: epsilon 0, 2 children, 6-DOP).
            let mut bvh = BvhTree::new(t.positions.len(), 0.0, 2, 6);
            for (i, p) in t.positions.iter().enumerate() {
                bvh.insert(i, std::slice::from_ref(p));
            }
            bvh.balance();
            t.bvh = bvh;
            t.verts_only = true;
            return Some(t);
        }
        if mesh.faces_num() == 0 {
            return None;
        }
        t.corner_tris = mesh.looptris().to_vec();
        t.tri_faces = mesh.looptri_faces();
        if t.corner_tris.is_empty() {
            return None;
        }
        // `create_tree_from_tris`
        let mut bvh = BvhTree::new(t.corner_tris.len(), 0.0, 2, 6);
        for (i, tri) in t.corner_tris.iter().enumerate() {
            let co = t.tri_coords(tri);
            bvh.insert(i, &co);
        }
        bvh.balance();
        t.bvh = bvh;
        if needs_normals(method, mode) {
            t.face_normals = mesh.face_normals().to_vec();
            if mesh.normals_domain() == NormalsDomain::Corner {
                t.corner_normals = mesh.corner_normals().to_vec();
            }
        }
        if method == Method::TargetProject {
            t.boundary = Some(build_boundary(mesh, &t));
        }
        Some(t)
    }

    fn tri_verts(&self, tri: &[u32; 3]) -> [usize; 3] {
        [
            self.corner_verts[tri[0] as usize] as usize,
            self.corner_verts[tri[1] as usize] as usize,
            self.corner_verts[tri[2] as usize] as usize,
        ]
    }

    fn tri_coords(&self, tri: &[u32; 3]) -> [V3; 3] {
        let v = self.tri_verts(tri);
        [
            self.positions[v[0]],
            self.positions[v[1]],
            self.positions[v[2]],
        ]
    }

    /// `BLI_bvhtree_find_nearest(tree, co, nearest, treeData->nearest_callback)`: the vertex
    /// tree has no callback; the triangle tree's is `mesh_corner_tris_nearest_point`.
    pub(crate) fn find_nearest(&self, co: V3, nearest: &mut Nearest) {
        if self.verts_only {
            self.bvh.find_nearest_ex(co, nearest, None, false);
            return;
        }
        let mut cb = |index: usize, co: V3, nearest: &mut Nearest| {
            let [a, b, c] = self.tri_coords(&self.corner_tris[index]);
            let p = closest_on_tri_to_point(co, a, b, c);
            let dist_sq = len_sq(co, p);
            if dist_sq < nearest.dist_sq {
                nearest.index = Some(index);
                nearest.dist_sq = dist_sq;
                nearest.co = p;
                nearest.no = normal_tri(a, b, c);
            }
        };
        self.bvh.find_nearest_ex(co, nearest, Some(&mut cb), false);
    }

    /// `BKE_shrinkwrap_find_nearest_surface`
    pub(crate) fn find_nearest_surface(&self, nearest: &mut Nearest, co: V3, method: Method) {
        if method == Method::TargetProject {
            let mut cb = |index: usize, co: V3, nearest: &mut Nearest| {
                self.target_project(index, co, nearest)
            };
            self.bvh.find_nearest_ex(co, nearest, Some(&mut cb), true);
            if nearest.index.is_none() {
                // Fall back to the simple nearest.
                self.find_nearest(co, nearest);
            }
        } else {
            self.find_nearest(co, nearest);
        }
    }

    /// `BKE_shrinkwrap_project_normal`: casts a ray from `vert` along `dir` (local space) on
    /// this tree; on a hit within `hit.dist` (kept in the target's space) updates `hit` with the
    /// local-space hit point and normal. `options` holds the culling flags.
    pub(crate) fn project_normal(
        &self,
        options: u8,
        vert: V3,
        dir: V3,
        transf: &SpaceTransform,
        hit: &mut RayHit,
    ) -> bool {
        let co = transf.apply(vert);
        let no = transf.apply_normal(dir);
        let mut hit_tmp = *hit;
        hit_tmp.index = None;
        let mut cb = |index: usize, ray: &BvhRay, hit: &mut RayHit| {
            // `mesh_corner_tris_spherecast` with radius 0 (`bvhtree_ray_tri_intersection`).
            let [a, b, c] = self.tri_coords(&self.corner_tris[index]);
            let dist = match &ray.isect_precalc {
                Some(pre) => isect_ray_tri_watertight_v3(ray.origin, pre, a, b, c)
                    .map_or(f32::MAX, |(l, _)| l),
                None => f32::MAX,
            };
            if dist >= 0.0 && dist < hit.dist {
                hit.index = Some(index);
                hit.dist = dist;
                hit.co = madd(ray.origin, ray.direction, dist);
                hit.no = normal_tri(a, b, c);
            }
        };
        self.bvh
            .ray_cast_ex(co, no, 0.0, &mut hit_tmp, Some(&mut cb), true);
        if hit_tmp.index.is_none() {
            return false;
        }
        // Invert the normal first so face culling works on rotated objects.
        hit_tmp.no = transf.invert_normal(hit_tmp.no);
        if options & CULL_MASK != 0 {
            let d = dot(dir, hit_tmp.no);
            if (options & CULL_FRONTFACE != 0 && d <= 0.0)
                || (options & CULL_BACKFACE != 0 && d >= 0.0)
            {
                return false;
            }
        }
        hit_tmp.co = transf.invert(hit_tmp.co);
        *hit = hit_tmp;
        true
    }

    /// `BKE_shrinkwrap_compute_smooth_normal`: the normal at `hit_co` on corner triangle
    /// `tri_idx` (local space through `transform` when given).
    pub(crate) fn smooth_normal(
        &self,
        transform: Option<&SpaceTransform>,
        tri_idx: usize,
        hit_co: V3,
        hit_no: V3,
    ) -> V3 {
        let tri = &self.corner_tris[tri_idx];
        let face = self.tri_faces[tri_idx] as usize;
        // Blender 5.2.4 interpolates when there is no `sharp_face` or the face is sharp.
        if self.sharp_faces.is_empty() || self.sharp_faces[face] {
            let v = self.tri_verts(tri);
            let no = if !self.corner_normals.is_empty() {
                [
                    self.corner_normals[tri[0] as usize],
                    self.corner_normals[tri[1] as usize],
                    self.corner_normals[tri[2] as usize],
                ]
            } else {
                [
                    self.vert_normals[v[0]],
                    self.vert_normals[v[1]],
                    self.vert_normals[v[2]],
                ]
            };
            let tmp_co = match transform {
                Some(t) => t.apply(hit_co),
                None => hit_co,
            };
            let w = interp_weights_tri(
                self.positions[v[0]],
                self.positions[v[1]],
                self.positions[v[2]],
                tmp_co,
            );
            let r = interp3(no[0], no[1], no[2], w);
            match transform {
                Some(t) => t.invert_normal(r),
                None => normalized(r),
            }
        } else if !self.face_normals.is_empty() {
            let r = self.face_normals[face];
            match transform {
                Some(t) => t.invert_normal(r),
                None => r,
            }
        } else {
            hit_no
        }
    }

    /// `BKE_shrinkwrap_snap_point_to_surface`
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn snap_point_to_surface(
        &self,
        transform: Option<&SpaceTransform>,
        mode: Mode,
        hit_idx: usize,
        hit_co: V3,
        hit_no: V3,
        goal_dist: f32,
        point_co: V3,
    ) -> V3 {
        match mode {
            Mode::OnSurface => {
                if goal_dist != 0.0 {
                    snap_with_side(point_co, hit_co, hit_no, goal_dist, 0.0, true)
                } else {
                    hit_co
                }
            }
            Mode::Inside => snap_with_side(point_co, hit_co, hit_no, goal_dist, -1.0, false),
            Mode::Outside => snap_with_side(point_co, hit_co, hit_no, goal_dist, 1.0, false),
            Mode::OutsideSurface => {
                if goal_dist != 0.0 {
                    snap_with_side(point_co, hit_co, hit_no, goal_dist, 1.0, true)
                } else {
                    hit_co
                }
            }
            Mode::AboveSurface => {
                if goal_dist != 0.0 {
                    let n = self.smooth_normal(transform, hit_idx, hit_co, hit_no);
                    madd(hit_co, n, goal_dist)
                } else {
                    hit_co
                }
            }
        }
    }

    // --------------------------------------------------------------------- target project

    /// `mesh_corner_tris_target_project`: the target-normal projection callback.
    fn target_project(&self, index: usize, co: V3, nearest: &mut Nearest) {
        let tri = &self.corner_tris[index];
        let v = self.tri_verts(tri);
        let vtri_co = [
            self.positions[v[0]],
            self.positions[v[1]],
            self.positions[v[2]],
        ];
        // The closest point first: bail out if it is worse than the current solution.
        let raw_hit_co = closest_on_tri_to_point(co, vtri_co[0], vtri_co[1], vtri_co[2]);
        let dist_sq = len_sq(co, raw_hit_co);
        if dist_sq >= nearest.dist_sq {
            return;
        }
        let vtri_no = [
            self.vert_normals[v[0]],
            self.vert_normals[v[1]],
            self.vert_normals[v[2]],
        ];
        if let Some((hit_co, hit_no)) = solve_point_tri(&vtri_co, &vtri_no, co, raw_hit_co, dist_sq)
        {
            update_hit(nearest, index, co, hit_co, hit_no);
        } else if let Some(b) = &self.boundary
            && b.has_boundary()
            && b.tri_has_boundary[index]
        {
            let edges = self.real_edges(tri);
            for e in edges {
                if e >= 0 && b.edge_is_boundary[e as usize] {
                    self.target_project_edge(b, index, co, nearest, e as usize);
                }
            }
        }
    }

    /// `corner_tri_get_real_edges`
    fn real_edges(&self, tri: &[u32; 3]) -> [i32; 3] {
        let mut real = [-1i32; 3];
        let mut i = 2;
        for i_next in 0..3 {
            let c1 = tri[i] as usize;
            let c2 = tri[i_next] as usize;
            let v1 = self.corner_verts[c1];
            let v2 = self.corner_verts[c2];
            let e = self.corner_edges[c1];
            let edge = self.edges[e as usize];
            let is_real = (v1 == edge[0] && v2 == edge[1]) || (v1 == edge[1] && v2 == edge[0]);
            real[i] = if is_real { e } else { -1 };
            i = i_next;
        }
        real
    }

    /// `target_project_edge`: a boundary edge as an infinitely thin cylinder.
    fn target_project_edge(
        &self,
        b: &Boundary,
        index: usize,
        co: V3,
        nearest: &mut Nearest,
        eidx: usize,
    ) {
        let edge = self.edges[eidx];
        let vedge_co = [
            self.positions[edge[0] as usize],
            self.positions[edge[1] as usize],
        ];
        let bid1 = b.vert_boundary_id[edge[0] as usize];
        let bid2 = b.vert_boundary_id[edge[1] as usize];
        if bid1 < 0 || bid2 < 0 {
            return;
        }
        let bv1 = &b.boundary_verts[bid1 as usize];
        let bv2 = &b.boundary_verts[bid2 as usize];
        let mut vedge_dir = [bv1.normal_plane, bv2.normal_plane];
        let dir = sub(vedge_co[1], vedge_co[0]);
        if dot(bv1.direction, dir) < 0.0 {
            vedge_dir[0] = [-vedge_dir[0][0], -vedge_dir[0][1], -vedge_dir[0][2]];
        }
        if dot(bv2.direction, dir) < 0.0 {
            vedge_dir[1] = [-vedge_dir[1][0], -vedge_dir[1][1], -vedge_dir[1][2]];
        }
        // Solve `lerp(d0,d1,x) · (co - lerp(v0,v1,x)) = 0`.
        let d0v0 = dot(vedge_dir[0], vedge_co[0]);
        let d0v1 = dot(vedge_dir[0], vedge_co[1]);
        let d1v0 = dot(vedge_dir[1], vedge_co[0]);
        let d1v1 = dot(vedge_dir[1], vedge_co[1]);
        let d0co = dot(vedge_dir[0], co);
        let a = d0v1 - d0v0 + d1v0 - d1v1;
        if a == 0.0 {
            return;
        }
        let bq = 2.0 * d0v0 - d0v1 - d0co - d1v0 + dot(vedge_dir[1], co);
        let c = d0co - d0v0;
        let det = bq * bq - 4.0 * a * c;
        if det < 0.0 {
            return;
        }
        let epsilon = 1e-6f32;
        let sdet = det.sqrt();
        let mut i: i32 = if det > 0.0 { 2 } else { 0 };
        while i >= 0 {
            let mut x = (-bq + (i - 1) as f32 * sdet) / (2.0 * a);
            if x >= -epsilon && x <= 1.0 + epsilon {
                x = x.clamp(0.0, 1.0);
                let vedge_no = [
                    self.vert_normals[edge[0] as usize],
                    self.vert_normals[edge[1] as usize],
                ];
                let hit_co = interp(vedge_co[0], vedge_co[1], x);
                let hit_no = interp(vedge_no[0], vedge_no[1], x);
                update_hit(nearest, index, co, hit_co, hit_no);
            }
            i -= 2;
        }
    }
}

/// `merge_vert_dir`: accumulates an edge into a vertex's average boundary direction.
fn merge_vert_dir(vdata: &mut [BoundaryVert], status: &mut [i8], index: usize, dir: V3, side: i8) {
    let d = &mut vdata[index].direction;
    let invert = if status[index] >= 0 {
        status[index] == side
    } else {
        dot(*d, dir) < 0.0
    };
    for k in 0..3 {
        if invert {
            d[k] -= dir[k];
        } else {
            d[k] += dir[k];
        }
    }
    status[index] = if status[index] == 0 { side } else { -1 };
}

/// `shrinkwrap_build_boundary_data`
fn build_boundary(mesh: &Mesh, t: &Tree) -> Boundary {
    let edges = &t.edges;
    // Faces per edge, up to 2.
    let mut edge_mode = vec![0i8; edges.len()];
    for &e in &t.corner_edges {
        if edge_mode[e as usize] < 2 {
            edge_mode[e as usize] += 1;
        }
    }
    let edge_is_boundary: Vec<bool> = edge_mode.iter().map(|&m| m == 1).collect();
    if !edge_is_boundary.iter().any(|&b| b) {
        return Boundary::default();
    }
    let tri_has_boundary: Vec<bool> = t
        .corner_tris
        .iter()
        .map(|tri| {
            t.real_edges(tri)
                .iter()
                .any(|&e| e >= 0 && edge_is_boundary[e as usize])
        })
        .collect();
    let n = mesh.verts_num();
    let mut vert_boundary_id = vec![0i32; n];
    for (i, e) in edges.iter().enumerate() {
        if edge_is_boundary[i] {
            vert_boundary_id[e[0] as usize] = 1;
            vert_boundary_id[e[1] as usize] = 1;
        }
    }
    let mut boundary_verts_num = 0;
    for id in &mut vert_boundary_id {
        *id = if *id != 0 {
            boundary_verts_num += 1;
            boundary_verts_num - 1
        } else {
            -1
        };
    }
    let mut boundary_verts = vec![BoundaryVert::default(); boundary_verts_num as usize];
    // `Array<int8_t>(n)` leaves the statuses uninitialised in Blender; `merge_vert_dir` is
    // written for zero ("no edge yet"), which a fresh allocation normally holds.
    let mut vert_status = vec![0i8; boundary_verts_num as usize];
    for (i, e) in edges.iter().enumerate() {
        if edge_is_boundary[i] {
            let dir = normalized(sub(t.positions[e[1] as usize], t.positions[e[0] as usize]));
            let b0 = vert_boundary_id[e[0] as usize] as usize;
            let b1 = vert_boundary_id[e[1] as usize] as usize;
            merge_vert_dir(&mut boundary_verts, &mut vert_status, b0, dir, 1);
            merge_vert_dir(&mut boundary_verts, &mut vert_status, b1, dir, 2);
        }
    }
    for (i, &bidx) in vert_boundary_id.iter().enumerate() {
        if bidx >= 0 {
            let vd = &mut boundary_verts[bidx as usize];
            vd.direction = normalized(vd.direction);
            let tmp = cross(t.vert_normals[i], vd.direction);
            vd.normal_plane = normalized(cross(tmp, t.vert_normals[i]));
        }
    }
    Boundary {
        edge_is_boundary,
        tri_has_boundary,
        vert_boundary_id,
        boundary_verts,
    }
}

/// `update_hit`
fn update_hit(nearest: &mut Nearest, index: usize, co: V3, hit_co: V3, hit_no: V3) -> bool {
    let dist_sq = len_sq(hit_co, co);
    if dist_sq < nearest.dist_sq {
        nearest.index = Some(index);
        nearest.dist_sq = dist_sq;
        nearest.co = hit_co;
        nearest.no = normalized(hit_no);
        return true;
    }
    false
}

/// `TargetProjectTriData` with the equation system of the target-project Newton solve.
struct TargetProjectTri<'a> {
    vtri_co: &'a [V3; 3],
    vtri_no: &'a [V3; 3],
    point_co: V3,
    n0_minus_n2: V3,
    n1_minus_n2: V3,
    c0_minus_c2: V3,
    c1_minus_c2: V3,
    co_interp: V3,
    no_interp: V3,
}

/// `target_project_tri_clamp`: clamps barycentric weights to the triangle.
fn tri_clamp(x: &mut V3) {
    x[0] = if x[0] < 0.0 { 0.0 } else { x[0] };
    x[1] = if x[1] < 0.0 { 0.0 } else { x[1] };
    if x[0] + x[1] > 1.0 {
        x[0] /= x[0] + x[1];
        x[1] = 1.0 - x[0];
    }
}

impl Newton3d for TargetProjectTri<'_> {
    /// `target_project_tri_deviation`
    fn delta(&mut self, x: V3) -> V3 {
        let w = [x[0], x[1], 1.0 - x[0] - x[1]];
        self.co_interp = interp3(self.vtri_co[0], self.vtri_co[1], self.vtri_co[2], w);
        self.no_interp = interp3(self.vtri_no[0], self.vtri_no[1], self.vtri_no[2], w);
        sub(madd(self.co_interp, self.no_interp, x[2]), self.point_co)
    }

    /// `target_project_tri_jacobian`
    fn jacobian(&mut self, x: V3) -> M3 {
        let mut r2 = self.vtri_no[2];
        r2 = madd(r2, self.n0_minus_n2, x[0]);
        r2 = madd(r2, self.n1_minus_n2, x[1]);
        [
            madd(self.c0_minus_c2, self.n0_minus_n2, x[2]),
            madd(self.c1_minus_c2, self.n1_minus_n2, x[2]),
            r2,
        ]
    }

    /// `target_project_tri_correct`: keeps the step within the triangle.
    fn correct(&mut self, x: V3, step: &mut V3, x_next: &mut V3) -> bool {
        // Insignificant correction threshold.
        let epsilon = 1e-5f32;
        // Dot product threshold for a step 'clearly' pointing outside.
        let dir_epsilon = 0.5f32;
        let sqrt2 = std::f32::consts::SQRT_2;
        let (mut fixed, mut locked) = (false, false);
        // Clamp against the diagonal first (the step is subtracted).
        let sum = x[0] + x[1];
        let sstep = -(step[0] + step[1]);
        if sum + sstep > 1.0 {
            let ldist = 1.0 - sum;
            if ldist < epsilon * sqrt2 {
                // Already at the boundary: slide along it.
                let step_len = (step[0] * step[0] + step[1] * step[1]).sqrt();
                // Abort if the solution is clearly outside the domain.
                if step_len > epsilon && sstep > step_len * dir_epsilon * sqrt2 {
                    return false;
                }
                // Project the new position onto the diagonal.
                let f = (sum + sstep - 1.0) * 0.5;
                step[0] += f;
                step[1] += f;
                fixed = true;
                locked = true;
            } else {
                // Scale a significant step down to arrive at the boundary.
                let f = ldist / sstep;
                for s in step.iter_mut() {
                    *s *= f;
                }
                fixed = true;
            }
        }
        // Weight 0 and 1 boundary checks, along the axes.
        for i in 0..2 {
            if step[i] > x[i] {
                if x[i] < epsilon {
                    let step_len = (step[0] * step[0] + step[1] * step[1]).sqrt();
                    if step_len > epsilon && (locked || step[i] > step_len * dir_epsilon) {
                        return false;
                    }
                    // Reset precision errors to stay at the boundary.
                    step[i] = x[i];
                    fixed = true;
                } else {
                    let f = x[i] / step[i];
                    for s in step.iter_mut() {
                        *s *= f;
                    }
                    fixed = true;
                }
            }
        }
        if fixed {
            *x_next = sub(x, *step);
            tri_clamp(x_next);
        }
        true
    }
}

/// `target_project_solve_point_tri`: the point of the triangle whose interpolated normal
/// passes through `point_co`, as `(co, normal)`.
fn solve_point_tri(
    vtri_co: &[V3; 3],
    vtri_no: &[V3; 3],
    point_co: V3,
    hit_co: V3,
    hit_dist_sq: f32,
) -> Option<(V3, V3)> {
    let dist = hit_dist_sq.sqrt();
    let magnitude_estimate =
        dist + len_manhattan(vtri_co[0]) + len_manhattan(vtri_co[1]) + len_manhattan(vtri_co[2]);
    let epsilon = magnitude_estimate * 1.0e-6;
    // Initial solution: barycentric weights plus the distance along the normal.
    let mut x = interp_weights_tri(vtri_co[0], vtri_co[1], vtri_co[2], hit_co);
    let r_hit_no = interp3(vtri_no[0], vtri_no[1], vtri_no[2], x);
    let tmp = sub(point_co, hit_co);
    x[2] = if dot(tmp, r_hit_no) < 0.0 {
        -dist
    } else {
        dist
    };
    let mut data = TargetProjectTri {
        vtri_co,
        vtri_no,
        point_co,
        n0_minus_n2: sub(vtri_no[0], vtri_no[2]),
        n1_minus_n2: sub(vtri_no[1], vtri_no[2]),
        c0_minus_c2: sub(vtri_co[0], vtri_co[2]),
        c1_minus_c2: sub(vtri_co[1], vtri_co[2]),
        co_interp: [0.0; 3],
        no_interp: [0.0; 3],
    };
    tri_clamp(&mut x);
    let (ok, _) = newton3d_solve(&mut data, epsilon, 20, x);
    ok.then_some((data.co_interp, data.no_interp))
}

/// `shrinkwrap_snap_with_side`: helper of `INSIDE`, `OUTSIDE` and `OUTSIDE_SURFACE`.
fn snap_with_side(
    point_co: V3,
    hit_co: V3,
    hit_no: V3,
    goal_dist: f32,
    mut forcesign: f32,
    forcesnap: bool,
) -> V3 {
    let mut delta = sub(point_co, hit_co);
    let dist = dot(delta, delta).sqrt();
    // Exactly on the surface: push out along the normal.
    if dist < f32::EPSILON {
        if forcesnap || goal_dist > 0.0 {
            return madd(hit_co, hit_no, goal_dist * forcesign);
        }
        return hit_co;
    }
    // `signf`
    let d = dot(delta, hit_no);
    let dsign = if d < 0.0 { -1.0 } else { 1.0 };
    if forcesign == 0.0 {
        forcesign = dsign;
    }
    // On the wrong side or too close: move to the correct side.
    if forcesnap || dsign * dist * forcesign < goal_dist {
        let f = dsign / dist;
        for x in &mut delta {
            *x *= f;
        }
        // At very small distances blend in the hit normal to stabilise the math.
        let dist_epsilon = (goal_dist.abs() + len_manhattan(hit_co)) * 1e-4;
        if dist < dist_epsilon {
            delta = interp(hit_no, delta, dist / dist_epsilon);
        }
        return madd(hit_co, delta, goal_dist * forcesign);
    }
    point_co
}

/// `BVHTreeNearest` before a search: nothing found yet within `FLT_MAX`.
pub(crate) fn nearest_none() -> Nearest {
    Nearest::within(f32::MAX)
}

/// `BVHTreeRayHit` before a projection (`hit->dist = BVH_RAYCAST_DIST_MAX`).
pub(crate) fn hit_none() -> RayHit {
    RayHit::within(BVH_RAYCAST_DIST_MAX)
}
