// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench shadows (`shading.show_shadows`, `shadow_intensity`, `display.light_direction`,
//! `shadow_shift`, `shadow_focus`): sharp stencil shadow volumes over opaque surfaces.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `workbench_shadow.cc` (`ShadowPass::init`: the world light direction, `shadow_focus`,
//!   `shadow_mul`/`shadow_add`, the far plane the volumes are extruded to);
//! - `draw/intern/mesh_extractors/extract_mesh_ibo_lines_adjacency.cc` (the edge-adjacency
//!   primitives and the mesh's `is_manifold`);
//! - `shaders/workbench_shadow.bsl.hh` (`vertex_main` extrusion, `geometry_main` silhouette
//!   quads with the degenerate-triangle workaround and the double count of manifold edges on
//!   non-manifold meshes, `geometry_main_caps`).
//!
//! Blender emulates a geometry shader on the GPU; frap builds the same triangles on the CPU in
//! world space ([`volume`]) and draws them with Blender's depth-fail stencil operations
//! (`GPU_STENCIL_OP_COUNT_DEPTH_FAIL`: back faces decrement, front faces increment, wrapping).
//! Blender picks the depth-pass or depth-fail method per object with a visibility compute
//! shader; both give the same stencil where both are valid, so frap always uses depth-fail
//! (with caps), which is valid everywhere. A pixel whose stencil is not 0 is in shadow: the
//! resolve multiplies it by `1 - shadow_intensity` instead of the lit factor.

use std::collections::HashMap;

use glam::{Mat3, Mat4, Vec3, Vec4};

/// Shadow settings of one render (`ShadowPass::init`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ShadowParams {
    /// `light_direction_ws`: `display.light_direction` with y and z swapped, x and z negated
    /// ("turn the light in a way where it's more user friendly to control"). Volumes extrude
    /// along it.
    pub direction_ws: Vec3,
    /// `shadow_shift`
    pub shift: f32,
    /// `1 - clamp(shadow_focus, 1e-4, 0.99999) * (1 - shadow_shift)`
    pub focus: f32,
    /// `shadow_intensity`
    pub mul: f32,
    /// `1 - shadow_intensity`
    pub add: f32,
}

impl ShadowParams {
    /// From `display.light_direction`, `shadow_shift`, `shadow_focus` and
    /// `shading.shadow_intensity`.
    pub fn new(light_direction: [f32; 3], shift: f32, focus: f32, intensity: f32) -> Self {
        let d = light_direction;
        let focus = focus.clamp(0.0001, 0.99999);
        ShadowParams {
            direction_ws: Vec3::new(-d[0], d[2], -d[1]),
            shift,
            focus: 1.0 - focus * (1.0 - shift),
            mul: intensity,
            add: 1.0 - intensity,
        }
    }

    /// The values when shadows are off (`shadow_mul = 0`, `shadow_add = 1`).
    pub fn off() -> Self {
        ShadowParams {
            direction_ws: Vec3::new(0.0, 0.0, -1.0),
            shift: 0.0,
            focus: 1.0,
            mul: 0.0,
            add: 1.0,
        }
    }
}

/// The edge-adjacency topology of a triangle mesh: Blender's `GPU_PRIM_LINES_ADJ` index buffer
/// over welded vertices, and whether every edge joins exactly two consistently wound
/// triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct Adjacency {
    /// Welded vertex positions (object space).
    pub positions: Vec<[f32; 3]>,
    /// Triangles over [`Adjacency::positions`].
    pub triangles: Vec<[u32; 3]>,
    /// `(v0, v1, v2, v3)`: edge `v1 v2`, `v0` opposite in the first triangle, `v3` opposite in
    /// the second (`v3 == v0` for a border or non-manifold edge).
    pub lines: Vec<[u32; 4]>,
    /// `is_manifold`
    pub is_manifold: bool,
}

/// Welds corner-split vertices back by position (bit-exact; the corner split copies them) and
/// builds the adjacency (`extract_lines_adjacency`). Triangles whose corners weld together
/// are skipped.
pub fn adjacency(positions: &[[f32; 3]], indices: &[u32]) -> Adjacency {
    let mut weld: HashMap<[u32; 3], u32> = HashMap::new();
    let mut welded = Vec::new();
    let mut remap = Vec::with_capacity(positions.len());
    for p in positions {
        let key = p.map(f32::to_bits);
        let next = welded.len() as u32;
        let id = *weld.entry(key).or_insert_with(|| {
            welded.push(*p);
            next
        });
        remap.push(id);
    }
    let mut triangles = Vec::with_capacity(indices.len() / 3);
    for t in indices.chunks_exact(3) {
        let Some(v) = t
            .iter()
            .map(|&i| remap.get(i as usize).copied())
            .collect::<Option<Vec<u32>>>()
        else {
            continue;
        };
        if v[0] == v[1] || v[1] == v[2] || v[0] == v[2] {
            continue;
        }
        triangles.push([v[0], v[1], v[2]]);
    }

    // `lines_adjacency_triangle`, with welded vertex indices standing for both Blender's vertex
    // and corner indices. Value: opposite vertex + 1, negated when the edge's winding runs
    // high → low; `HANDLED` once two triangles were paired.
    const HANDLED: i64 = i64::MAX - 1;
    let mut edge_hash: HashMap<(u32, u32), i64> = HashMap::new();
    let mut order: Vec<(u32, u32)> = Vec::new();
    let mut lines = Vec::new();
    let mut is_manifold = true;
    for tri in &triangles {
        let mut v = *tri;
        for _ in 0..3 {
            // `rotate_vector`: (a, b, c) → (c, a, b).
            v = [v[2], v[0], v[1]];
            let inv_indices = v[1] > v[2];
            let key = (v[1].min(v[2]), v[1].max(v[2]));
            let new_value = i64::from(v[0]) + 1;
            let new_value = if inv_indices { -new_value } else { new_value };
            match edge_hash.get_mut(&key) {
                None => {
                    edge_hash.insert(key, new_value);
                    order.push(key);
                }
                Some(value) if *value == HANDLED => {
                    // A third triangle on the edge.
                    is_manifold = false;
                }
                Some(value) => {
                    let v_data = *value;
                    *value = HANDLED;
                    let inv_opposite = v_data < 0;
                    let opposite = (v_data.unsigned_abs() - 1) as u32;
                    if inv_opposite == inv_indices {
                        // Don't share the edge if the triangles have non-matching winding.
                        lines.push([v[0], v[1], v[2], v[0]]);
                        lines.push([opposite, v[1], v[2], opposite]);
                        is_manifold = false;
                    } else {
                        lines.push([v[0], v[1], v[2], opposite]);
                    }
                }
            }
        }
    }
    // `create_lines_for_remaining_edges`: border edges.
    for key in order {
        let v_data = edge_hash[&key];
        if v_data == HANDLED {
            continue;
        }
        let (mut v2, mut v3) = key;
        let l1 = (v_data.unsigned_abs() - 1) as u32;
        if v_data < 0 {
            std::mem::swap(&mut v2, &mut v3);
        }
        lines.push([l1, v2, v3, l1]);
        is_manifold = false;
    }
    Adjacency {
        positions: welded,
        triangles,
        lines,
        is_manifold,
    }
}

/// A plane `n · x = d` with `n` pointing out of the view frustum.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Plane {
    /// Outward unit normal.
    pub normal: Vec3,
    /// Offset.
    pub d: f32,
}

/// The far plane of a camera (`frustum_planes_get()[2]` with its normal turned outwards) from
/// Blender's OpenGL-depth window matrix and the view matrix.
pub fn far_plane(gl_winmat: &Mat4, view: &Mat4) -> Plane {
    let pm = *gl_winmat * *view;
    // Inward far plane: row 3 - row 2 (`-w <= z <= w`).
    let row = |i: usize| Vec4::new(pm.x_axis[i], pm.y_axis[i], pm.z_axis[i], pm.w_axis[i]);
    let p = row(3) - row(2);
    let len = p.truncate().length();
    let p = if len > 0.0 { p / len } else { p };
    Plane {
        normal: -p.truncate(),
        d: p.w,
    }
}

struct VertOut {
    lp: Vec3,
    front: Vec3,
    back: Vec3,
}

/// Appends the shadow-volume triangles (world space) of one object to `out`: silhouette
/// quads, then front and back caps (`geometry_main`, `geometry_main_caps`).
pub fn volume(
    adj: &Adjacency,
    model: &Mat4,
    params: &ShadowParams,
    far: &Plane,
    out: &mut Vec<[f32; 3]>,
) {
    let l = params.direction_ws;
    let l_fp = l.dot(far.normal);
    let vert = |i: u32| -> VertOut {
        let lp = Vec3::from(adj.positions[i as usize]);
        let ws = model.transform_point3(lp);
        let mut extrude_distance = 1e5f32;
        if l_fp > 0.0 {
            let signed_distance = far.normal.dot(ws) - far.d;
            extrude_distance = -signed_distance / l_fp;
            // Ensure we don't overlap the far plane.
            extrude_distance -= 1e-3;
        }
        VertOut {
            lp,
            front: ws,
            back: ws + l * extrude_distance,
        }
    };
    // `drw_normal_world_to_object(L)` = `L * mat3(model)`.
    let ls_light = Mat3::from_mat4(*model).transpose() * l;
    let double_manifold = !adj.is_manifold;
    let push = |out: &mut Vec<[f32; 3]>, p: Vec3| out.push(p.to_array());

    for line in &adj.lines {
        let g = line.map(vert);
        let v10 = g[0].lp - g[1].lp;
        let v12 = g[2].lp - g[1].lp;
        let v13 = g[3].lp - g[1].lp;
        let n1 = v12.cross(v10);
        let n2 = v13.cross(v12);
        // DEGENERATE_TRIS_WORKAROUND
        const DEGENERATE_TRIS_EPSILON: f32 = 1e-10;
        let faces_area = [n1.length_squared(), n2.length_squared()];
        let edges_area = [
            v12.length_squared() * v10.length_squared(),
            v13.length_squared() * v12.length_squared(),
        ];
        let degen = [
            faces_area[0] <= DEGENERATE_TRIS_EPSILON * edges_area[0],
            faces_area[1] <= DEGENERATE_TRIS_EPSILON * edges_area[1],
        ];
        if degen[0] && degen[1] {
            continue;
        }
        let facing = [n1.dot(ls_light), n2.dot(ls_light)];
        let mut backface = [facing[0] > 0.0, facing[1] > 0.0];
        let mut is_manifold = line[0] != line[3];
        if !double_manifold && ((degen[0] && backface[1]) || (degen[1] && backface[0])) {
            continue;
        }
        if degen[0] {
            backface[0] = !backface[1];
        }
        if degen[1] {
            backface[1] = !backface[0];
        }
        if degen[0] || degen[1] {
            is_manifold = false;
        }
        if backface[0] == backface[1] {
            continue;
        }
        let invocations = if double_manifold && is_manifold { 2 } else { 1 };
        for _ in 0..invocations {
            // `extrude_edge(backface.x, v1, v2)`: strip (v2.front, v1.front, v2.back, v1.back)
            // as the triangles (0, 1, 2) and (3, 2, 1).
            let (a, b) = if backface[0] {
                (&g[2], &g[1])
            } else {
                (&g[1], &g[2])
            };
            let s = [b.front, a.front, b.back, a.back];
            for i in [0, 1, 2, 3, 2, 1] {
                push(out, s[i]);
            }
        }
    }

    for tri in &adj.triangles {
        let g = tri.map(vert);
        let v10 = g[0].lp - g[1].lp;
        let v12 = g[2].lp - g[1].lp;
        let ng = v12.cross(v10);
        let backface = ng.dot(ls_light) > 0.0;
        let (invert, manifold) = if double_manifold {
            (backface, false)
        } else {
            (false, true)
        };
        if manifold && backface {
            continue;
        }
        for front in [true, false] {
            let (g1, g2) = if invert == front {
                (&g[2], &g[1])
            } else {
                (&g[1], &g[2])
            };
            let pick = |v: &VertOut| if front { v.front } else { v.back };
            push(out, pick(&g[0]));
            push(out, pick(g1));
            push(out, pick(g2));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube as corner-split triangles (each face its own 4 vertices).
    fn cube() -> (Vec<[f32; 3]>, Vec<u32>) {
        let faces: [[[f32; 3]; 4]; 6] = [
            [
                [-1., -1., -1.],
                [-1., 1., -1.],
                [1., 1., -1.],
                [1., -1., -1.],
            ],
            [[-1., -1., 1.], [1., -1., 1.], [1., 1., 1.], [-1., 1., 1.]],
            [
                [-1., -1., -1.],
                [1., -1., -1.],
                [1., -1., 1.],
                [-1., -1., 1.],
            ],
            [[1., 1., -1.], [-1., 1., -1.], [-1., 1., 1.], [1., 1., 1.]],
            [
                [-1., 1., -1.],
                [-1., -1., -1.],
                [-1., -1., 1.],
                [-1., 1., 1.],
            ],
            [[1., -1., -1.], [1., 1., -1.], [1., 1., 1.], [1., -1., 1.]],
        ];
        let mut pos = vec![];
        let mut idx = vec![];
        for f in faces {
            let b = pos.len() as u32;
            pos.extend(f);
            idx.extend([b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        (pos, idx)
    }

    #[test]
    fn params_follow_blender() {
        let p = ShadowParams::new([0.577_350_26; 3], 0.1, 0.0, 0.5);
        assert_eq!(
            p.direction_ws,
            Vec3::new(-0.577_350_26, 0.577_350_26, -0.577_350_26)
        );
        assert_eq!((p.mul, p.add), (0.5, 0.5));
        assert!((p.focus - (1.0 - 0.0001 * 0.9)).abs() < 1e-7);
    }

    #[test]
    fn closed_cube_is_manifold() {
        let (pos, idx) = cube();
        let a = adjacency(&pos, &idx);
        assert_eq!(a.positions.len(), 8);
        assert_eq!(a.triangles.len(), 12);
        assert!(a.is_manifold);
        // 18 edges (12 cube edges + 6 face diagonals), each shared by two triangles.
        assert_eq!(a.lines.len(), 18);
        assert!(a.lines.iter().all(|l| l[0] != l[3]));
    }

    #[test]
    fn open_plane_is_not_manifold() {
        let pos = vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];
        let a = adjacency(&pos, &[0, 1, 2, 0, 2, 3]);
        assert!(!a.is_manifold);
        // The diagonal is shared, the 4 borders are not.
        assert_eq!(a.lines.len(), 5);
        assert_eq!(a.lines.iter().filter(|l| l[0] == l[3]).count(), 4);
    }

    #[test]
    fn cube_volume_has_silhouette_and_caps() {
        let (pos, idx) = cube();
        let a = adjacency(&pos, &idx);
        let params = ShadowParams::new([0.577_350_26; 3], 0.1, 0.0, 0.5);
        let far = Plane {
            normal: Vec3::new(0.0, 0.0, -1.0),
            d: 100.0,
        };
        let mut out = vec![];
        volume(&a, &Mat4::IDENTITY, &params, &far, &mut out);
        assert_eq!(out.len() % 3, 0);
        // A cube lit along a diagonal: 3 lit faces (6 triangles) → 6 front + 6 back caps; its
        // silhouette is a hexagon of 6 edges → 12 side triangles.
        assert_eq!(out.len() / 3, 12 + 12);
        // Back caps lie on the far plane (minus 1e-3) when the light points through it.
        let back_z: Vec<f32> = out.iter().map(|p| p[2]).filter(|z| *z < -50.0).collect();
        assert!(!back_z.is_empty());
        assert!(
            back_z.iter().all(|z| (z + 100.0 - 1e-3).abs() < 1e-2),
            "{back_z:?}"
        );
    }

    #[test]
    fn far_plane_of_a_camera() {
        // Camera at the origin looking down -Z, far = 10.
        let gl = frap_render_scene::build::camera::perspective_m4(-0.1, 0.1, -0.1, 0.1, 0.1, 10.0);
        let f = far_plane(&gl, &Mat4::IDENTITY);
        assert!(
            (f.normal - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-5,
            "{f:?}"
        );
        assert!((f.d - 10.0).abs() < 1e-3, "{f:?}");
    }
}
