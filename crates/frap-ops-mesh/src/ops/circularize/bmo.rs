// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.circularize(bmesh, geom=[], factor=0.0, custom_radius=0.0, angle=0.0,
//! fit_method=0, flatten=0.0, regular=False, lock_x/y/z=False, mirror_x/y/z=False)` (new in
//! 5.x).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_circularize.cc (Blender 5.2.4,
//! GPL-2.0-or-later), with `isect_line_tri_v3`, `closest_on_tri_to_point_v3`,
//! `line_point_factor_v3`, `angle_signed_v2v2` (`blenlib/intern/math_geom.cc`,
//! `math_vector.cc`) and Eigen's 3×3 `computeInverseWithCheck` (`math::invert`).
//!
//! The boundary chains of the selection (selected visible edges with at most one selected
//! face, not lying on an enabled mirror plane) are walked; each chain of three or more
//! vertices (four for a closed one) gets a plane (centroid, Newell normal oriented like the
//! vertex normals), a 2D circle (Gauss–Newton least squares, or the largest circle inside
//! for `CONTRACT`; an open chain whose ends lie on a mirror plane is centred between them)
//! and target positions on that circle (evenly spaced with `regular`, rotated by `angle`).
//! With `flatten < 1` the targets are projected back on the surrounding faces (along the
//! plane normal, else onto an edge, else the nearest triangle of the mesh BVH). Locked axes
//! keep their coordinate; `factor` blends towards the target.

use frap_bmesh::ops::intersect::query::face_calc_tessellation;
use frap_bmesh::ops::intersect::tessellate::mesh_calc_tessellation;
use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use frap_geom::bvh::{BvhTree, Nearest};

use super::super::rip_edge::math::{
    V3, add3, angle_normalized_v3v3, closest_to_line_v3, cross3, dot3, interp_v3_v3v3,
    len_squared_v3v3, midpoint3, normalize_and_get_length, normalize3, sub3,
};

/// `NON_LINEAR_LEAST_SQUARES_MAX_ITERATIONS`.
const MAX_ITERATIONS: usize = 500;
/// `MIRROR_LIMIT`.
const MIRROR_LIMIT: f32 = 0.001;
/// `CIRCULARIZE_EPSILON`.
const EPS: f32 = 1e-6;

type V2 = [f32; 2];
/// A 3×3 matrix as Blender's `float3x3` (`m[col][row]`).
type M3 = [[f32; 3]; 3];

/// `FitMethod`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FitMethod {
    /// `FIT_METHOD_LEAST_SQUARE`.
    LeastSquare,
    /// `FIT_METHOD_CONTRACT`.
    Contract,
}

/// The input slots of `circularize`.
pub(crate) struct CircularizeIn {
    pub geom: Vec<BMElem>,
    pub factor: f32,
    pub custom_radius: f32,
    pub angle: f32,
    pub fit_method: FitMethod,
    pub flatten: f32,
    pub regular: bool,
    pub lock: [bool; 3],
    pub mirror: [bool; 3],
}

/// `CircleVert`.
#[derive(Copy, Clone, Debug)]
struct CircleVert {
    v: VertId,
    co_2d: V2,
    target_2d: V2,
}

/// `VertChain`.
struct VertChain {
    verts: Vec<VertId>,
    is_closed: bool,
}

fn len2(a: V2) -> f32 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}
fn normalize2(a: V2) -> V2 {
    let l2 = a[0] * a[0] + a[1] * a[1];
    if l2 > 1.0e-35 {
        let l = l2.sqrt();
        [a[0] / l, a[1] / l]
    } else {
        [0.0; 2]
    }
}
/// `angle_signed_v2v2`.
fn angle_signed_v2v2(v1: V2, v2: V2) -> f32 {
    let perp_dot = (v1[1] * v2[0]) - (v1[0] * v2[1]);
    libm::atan2f(perp_dot, v1[0] * v2[0] + v1[1] * v2[1])
}

/// `float3x3 * float3` (`result += b[c] * a[c]` per column).
fn mul_m3_v3(m: &M3, b: V3) -> V3 {
    let mut r = [0.0f32; 3];
    for (c, col) in m.iter().enumerate() {
        for k in 0..3 {
            r[k] += b[c] * col[k];
        }
    }
    r
}

/// `math::invert(float3x3, success)`: Eigen's `computeInverseWithCheck` for 3×3 (cofactors,
/// `determinant = (cofactors_col0 .* col0).sum()` as `c0 + (c1 + c2)`, threshold 0). `M(i, j)`
/// is row `i`, column `j` (`m[j][i]`).
fn invert_m3(m: &M3) -> Option<M3> {
    let at = |i: usize, j: usize| m[j][i];
    let cof = |i: usize, j: usize| {
        let (i1, i2, j1, j2) = ((i + 1) % 3, (i + 2) % 3, (j + 1) % 3, (j + 2) % 3);
        at(i1, j1) * at(i2, j2) - at(i1, j2) * at(i2, j1)
    };
    let c0 = [cof(0, 0), cof(1, 0), cof(2, 0)];
    let det = c0[0] * at(0, 0) + (c0[1] * at(1, 0) + c0[2] * at(2, 0));
    // `abs(determinant) > 0`: a NaN determinant is not invertible either.
    if det.abs().partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let invdet = 1.0 / det;
    // `result(r, c)` = cofactor(c, r) * invdet; stored as `res[c][r]`.
    let mut res = [[0.0f32; 3]; 3];
    let set = |res: &mut M3, r: usize, c: usize, v: f32| res[c][r] = v;
    set(&mut res, 1, 2, cof(2, 1) * invdet);
    set(&mut res, 2, 1, cof(1, 2) * invdet);
    set(&mut res, 2, 2, cof(2, 2) * invdet);
    set(&mut res, 1, 0, cof(0, 1) * invdet);
    set(&mut res, 1, 1, cof(1, 1) * invdet);
    set(&mut res, 2, 0, cof(0, 2) * invdet);
    // `result.row(0) = cofactors_col0 * invdet`.
    for (c, &v) in c0.iter().enumerate() {
        set(&mut res, 0, c, v * invdet);
    }
    Some(res)
}

/// `add_newell_cross_v3_v3v3`.
fn add_newell_cross(n: &mut V3, v_prev: V3, v_curr: V3) {
    n[0] += (v_prev[1] - v_curr[1]) * (v_prev[2] + v_curr[2]);
    n[1] += (v_prev[2] - v_curr[2]) * (v_prev[0] + v_curr[0]);
    n[2] += (v_prev[0] - v_curr[0]) * (v_prev[1] + v_curr[1]);
}

/// `isect_line_tri_v3`: the line factor of the hit.
fn isect_line_tri_v3(p1: V3, p2: V3, v0: V3, v1: V3, v2: V3) -> Option<f32> {
    let e1 = sub3(v1, v0);
    let e2 = sub3(v2, v0);
    let d = sub3(p2, p1);
    let p = cross3(d, e2);
    let a = dot3(e1, p);
    if a == 0.0 {
        return None;
    }
    let f = 1.0 / a;
    let s = sub3(p1, v0);
    let u = f * dot3(s, p);
    // NaN passes, as in C (`(u < 0) || (u > 1)`).
    #[allow(clippy::manual_range_contains)]
    if u < 0.0 || u > 1.0 {
        return None;
    }
    let q = cross3(s, e1);
    let v = f * dot3(d, q);
    if v < 0.0 || (u + v) > 1.0 {
        return None;
    }
    Some(f * dot3(e2, q))
}

fn madd3(a: V3, b: V3, f: f32) -> V3 {
    [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
}

/// `closest_on_tri_to_point_v3` (Ericson).
fn closest_on_tri_to_point_v3(p: V3, v1: V3, v2: V3, v3: V3) -> V3 {
    let ab = sub3(v2, v1);
    let ac = sub3(v3, v1);
    let ap = sub3(p, v1);
    let d1 = dot3(ab, ap);
    let d2 = dot3(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return v1;
    }
    let bp = sub3(p, v2);
    let d3 = dot3(ab, bp);
    let d4 = dot3(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return v2;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let ab_squared = d1 - d3;
        return if ab_squared == 0.0 {
            v1
        } else {
            madd3(v1, ab, d1 / ab_squared)
        };
    }
    let cp = sub3(p, v3);
    let d5 = dot3(ab, cp);
    let d6 = dot3(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return v3;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let ac_squared = d2 - d6;
        return if ac_squared == 0.0 {
            v1
        } else {
            madd3(v1, ac, d2 / ac_squared)
        };
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let bc_squared = (d4 - d3) + (d5 - d6);
        if bc_squared == 0.0 {
            return v2;
        }
        let r = sub3(v3, v2);
        let f = (d4 - d3) / bc_squared;
        return add3([r[0] * f, r[1] * f, r[2] * f], v2);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    let acw = [ac[0] * w, ac[1] * w, ac[2] * w];
    add3(madd3(v1, ab, v), acw)
}

/// `line_point_factor_v3` (`line_point_factor_v3_ex(p, l1, l2, 0, 0)`).
fn line_point_factor_v3(p: V3, l1: V3, l2: V3) -> f32 {
    let u = sub3(l2, l1);
    let h = sub3(p, l1);
    let d = dot3(u, u);
    if d > 0.0 { dot3(u, h) / d } else { 0.0 }
}

/// `is_valid_boundary_edge`.
fn is_valid_boundary_edge(bm: &BMesh, e: EdgeId, check_axis: [bool; 3]) -> bool {
    let ed = bm.edge(e);
    if !ed.flag.contains(ElemFlag::TAG) || ed.flag.contains(ElemFlag::HIDDEN) {
        return false;
    }
    // Wire edges are not valid boundary edges.
    if ed.l.is_none() {
        return false;
    }
    let mut selected_face_count = 0;
    for f in bm.edge_faces(e) {
        let fl = bm.face(f).flag;
        if !fl.contains(ElemFlag::HIDDEN) && fl.contains(ElemFlag::TAG) {
            selected_face_count += 1;
            if selected_face_count > 1 {
                break;
            }
        }
    }
    if selected_face_count > 1 {
        return false;
    }
    let (c1, c2) = (bm.vert(ed.v[0]).co, bm.vert(ed.v[1]).co);
    for i in 0..3 {
        if check_axis[i] && c1[i].abs() < MIRROR_LIMIT && c2[i].abs() < MIRROR_LIMIT {
            return false;
        }
    }
    true
}

/// `walk_boundary_chain`.
fn walk_boundary_chain(
    bm: &BMesh,
    start: EdgeId,
    visited: &mut [bool],
    check_axis: [bool; 3],
) -> Option<VertChain> {
    let next_edge = |v: VertId, exclude: EdgeId, visited: &[bool]| -> Option<EdgeId> {
        bm.vert_edges(v).into_iter().find(|&e| {
            e != exclude && !visited[e.slot()] && is_valid_boundary_edge(bm, e, check_axis)
        })
    };
    let walk =
        |mut curr_v: VertId, mut curr_e: EdgeId, list: &mut Vec<VertId>, visited: &mut [bool]| {
            while let Some(next_e) = next_edge(curr_v, curr_e, visited) {
                curr_v = bm.edge_other_vert(next_e, curr_v);
                curr_e = next_e;
                list.push(curr_v);
                visited[curr_e.slot()] = true;
            }
        };
    let [v1, v2] = bm.edge(start).v;
    let mut verts = vec![v1, v2];
    visited[start.slot()] = true;
    // The initial edge direction (v1 -> v2) is arbitrary: walk from v2 first.
    walk(v2, start, &mut verts, visited);
    // A closed chain ends on its first vertex: drop the duplicate.
    if verts.len() > 2 && verts.first() == verts.last() {
        if verts.len() < 4 {
            return None;
        }
        verts.pop();
        return Some(VertChain {
            verts,
            is_closed: true,
        });
    }
    // Open: walk the other direction from the start vertex.
    let mut pre_chain = Vec::new();
    walk(v1, start, &mut pre_chain, visited);
    if !pre_chain.is_empty() {
        pre_chain.reverse();
        pre_chain.extend_from_slice(&verts);
        verts = pre_chain;
    }
    if verts.len() < 3 {
        return None;
    }
    Some(VertChain {
        verts,
        is_closed: false,
    })
}

/// `bm_vert_chain_orientation_matrix_calc`: (matrix, centre).
fn orientation_matrix_calc(bm: &BMesh, chain: &[VertId]) -> (M3, V3) {
    let mut center = [0.0f32; 3];
    for &v in chain {
        center = add3(center, bm.vert(v).co);
    }
    let n = chain.len() as f32;
    center = [center[0] / n, center[1] / n, center[2] / n];
    let mut prev = *chain.last().expect("non-empty");
    let mut normal = [0.0f32; 3];
    // Best-fit plane normal for the chain (Newell's method).
    for &curr in chain {
        add_newell_cross(&mut normal, bm.vert(prev).co, bm.vert(curr).co);
        prev = curr;
    }
    let normal = normalize3(normal);
    let mut guess = [1.0f32, 0.0, 0.0];
    // A normal parallel to X would give a zero cross product: switch to Y.
    if dot3(normal, guess).abs() > 0.99 {
        guess = [0.0, 1.0, 0.0];
    }
    let p = normalize3(cross3(normal, guess));
    let q = cross3(normal, p);
    ([p, q, normal], center)
}

/// `calculate_circle_best_fit`: Gauss–Newton on `(cx, cy, r)` from `(0, 0, 1)`.
fn circle_best_fit(verts: &[CircleVert], fixed_center: Option<V2>) -> (V2, f32) {
    if let Some(c) = fixed_center {
        let mut radius = 0.0f32;
        for cv in verts {
            radius += len2(cv.co_2d);
        }
        radius /= verts.len() as f32;
        return (c, radius);
    }
    let mut center = [0.0f32; 2];
    let mut radius = 1.0f32;
    for _ in 0..MAX_ITERATIONS {
        let mut nm: M3 = [[0.0; 3]; 3];
        let mut jtr = [0.0f32; 3];
        for cv in verts {
            let d_vec = [center[0] - cv.co_2d[0], center[1] - cv.co_2d[1]];
            let distance = len2(d_vec);
            if distance < EPS {
                continue;
            }
            let j_row = [d_vec[0] / distance, d_vec[1] / distance, -1.0];
            let residual = radius - distance;
            for row in 0..3 {
                for col in 0..3 {
                    nm[col][row] += j_row[row] * j_row[col];
                }
                jtr[row] += j_row[row] * residual;
            }
        }
        let Some(inv) = invert_m3(&nm) else {
            break;
        };
        let delta = mul_m3_v3(&inv, jtr);
        center[0] += delta[0];
        center[1] += delta[1];
        radius += delta[2];
        // Converged.
        if delta[0].abs() < EPS && delta[1].abs() < EPS && delta[2].abs() < EPS {
            break;
        }
    }
    (center, radius)
}

/// `calculate_circle_inside_fit`: the edge-length weighted centre and the smallest distance.
fn circle_inside_fit(verts: &[CircleVert], fixed_center: Option<V2>) -> (V2, f32) {
    let center = match fixed_center {
        Some(c) => c,
        None => {
            let mut total = 0.0f32;
            let mut center = [0.0f32; 2];
            let mut prev = verts.last().expect("non-empty").co_2d;
            for cv in verts {
                let curr = cv.co_2d;
                let el = len2([prev[0] - curr[0], prev[1] - curr[1]]);
                center[0] += (prev[0] + curr[0]) * el;
                center[1] += (prev[1] + curr[1]) * el;
                total += el;
                prev = curr;
            }
            if total != 0.0 {
                let f = 0.5 / total;
                center = [center[0] * f, center[1] * f];
            }
            center
        }
    };
    let mut radius_sq = f32::MAX;
    for cv in verts {
        let d = [center[0] - cv.co_2d[0], center[1] - cv.co_2d[1]];
        radius_sq = radius_sq.min(d[0] * d[0] + d[1] * d[1]);
    }
    (center, radius_sq.sqrt())
}

/// `calculate_target_locations`.
fn calculate_target_locations(
    verts: &mut [CircleVert],
    center: V2,
    radius: f32,
    is_regular: bool,
    is_closed: bool,
    rotation_angle: f32,
) {
    let mut step = 0.0f32;
    let mut start_angle = 0.0f32;
    let two_pi = 2.0 * std::f32::consts::PI;
    if is_regular {
        let mut total_angle = two_pi;
        let mut divisions = verts.len();
        // An open chain spans the angle its vertices traverse, not a full turn.
        if !is_closed && divisions > 1 {
            total_angle = 0.0;
            divisions = verts.len() - 1;
            let mut vec_prev =
                normalize2([verts[0].co_2d[0] - center[0], verts[0].co_2d[1] - center[1]]);
            for cv in verts.iter().skip(1) {
                let vec_curr = normalize2([cv.co_2d[0] - center[0], cv.co_2d[1] - center[1]]);
                total_angle -= angle_signed_v2v2(vec_prev, vec_curr);
                vec_prev = vec_curr;
            }
            total_angle = total_angle.clamp(-two_pi, two_pi);
        }
        step = total_angle / divisions as f32;
        let (mut sum_sin, mut sum_cos) = (0.0f32, 0.0f32);
        // The circular mean of the angular deviations of every vertex.
        for (i, cv) in verts.iter().enumerate() {
            let vec = [cv.co_2d[0] - center[0], cv.co_2d[1] - center[1]];
            let angle_diff = libm::atan2f(vec[1], vec[0]) - (step * i as f32);
            sum_sin += libm::sinf(angle_diff);
            sum_cos += libm::cosf(angle_diff);
        }
        start_angle = libm::atan2f(sum_sin, sum_cos);
    }
    for (i, cv) in verts.iter_mut().enumerate() {
        let angle = if is_regular {
            start_angle + step * i as f32 - rotation_angle
        } else {
            let vec = [cv.co_2d[0] - center[0], cv.co_2d[1] - center[1]];
            libm::atan2f(vec[1], vec[0]) - rotation_angle
        };
        cv.target_2d = [
            center[0] + libm::cosf(angle) * radius,
            center[1] + libm::sinf(angle) * radius,
        ];
    }
}

/// The mesh triangles for the BVH fallback of `project_on_mesh`.
struct MeshTris {
    tree: BvhTree,
    looptris: Vec<[LoopId; 3]>,
}

/// `project_on_mesh`.
fn project_on_mesh(
    bm: &BMesh,
    bvh: Option<&MeshTris>,
    v: VertId,
    center_pos: V3,
    normal: V3,
    tess_cache: &mut std::collections::HashMap<FaceId, Vec<[VertId; 3]>>,
) -> V3 {
    let vco = bm.vert(v).co;
    let (vec, length) = normalize_and_get_length(sub3(center_pos, vco));
    // Normalisation fails for coincident positions.
    if length == 0.0 {
        return center_pos;
    }
    let angle = angle_normalized_v3v3(vec, normal);
    if angle.abs() < EPS || (std::f32::consts::PI - angle).abs() < EPS {
        return vco;
    }
    let p2 = add3(center_pos, normal);
    let mut best_dist_sq = f32::MAX;
    let mut found: Option<V3> = None;
    let mut test_tri = |a: VertId, b: VertId, c: VertId, found: &mut Option<V3>| {
        if let Some(lambda) =
            isect_line_tri_v3(center_pos, p2, bm.vert(a).co, bm.vert(b).co, bm.vert(c).co)
        {
            let hit = madd3(center_pos, normal, lambda);
            let dist_sq = len_squared_v3v3(center_pos, hit);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                *found = Some(hit);
            }
        }
    };
    for f in bm.vert_faces(v) {
        let fd = bm.face(f);
        if fd.len < 3 || fd.flag.contains(ElemFlag::HIDDEN) {
            continue;
        }
        if fd.len == 3 || fd.len == 4 {
            let l_start = fd.l_first;
            let v1 = bm.lp(bm.lp(l_start).prev).v;
            let v2 = bm.lp(l_start).v;
            let v3 = bm.lp(bm.lp(l_start).next).v;
            test_tri(v1, v2, v3, &mut found);
            if fd.len == 4 {
                let v4 = bm.lp(bm.lp(bm.lp(l_start).next).next).v;
                test_tri(v1, v3, v4, &mut found);
            }
        } else {
            let tris = tess_cache.entry(f).or_insert_with(|| {
                let (loops, idx): (Vec<LoopId>, Vec<[u32; 3]>) = face_calc_tessellation(bm, f);
                idx.iter()
                    .map(|t| {
                        [
                            bm.lp(loops[t[0] as usize]).v,
                            bm.lp(loops[t[1] as usize]).v,
                            bm.lp(loops[t[2] as usize]).v,
                        ]
                    })
                    .collect()
            });
            for t in tris.clone() {
                test_tri(t[0], t[1], t[2], &mut found);
            }
        }
    }
    if let Some(p) = found {
        return p;
    }
    for e in bm.vert_edges(v) {
        let [a, b] = bm.edge(e).v;
        let (closest, _) = closest_to_line_v3(center_pos, bm.vert(a).co, bm.vert(b).co);
        let fac = line_point_factor_v3(closest, bm.vert(a).co, bm.vert(b).co);
        if fac > EPS && fac < 1.0 - EPS {
            let dist_sq = len_squared_v3v3(center_pos, closest);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                found = Some(closest);
            }
        }
    }
    if let Some(p) = found {
        return p;
    }
    if let Some(m) = bvh {
        let mut cb = |index: usize, co: V3, nearest: &mut Nearest| {
            // `nearest_tri_cb` reads the current positions (the tree keeps the bounds it was
            // built with).
            let t = m.looptris[index].map(|l| bm.vert(bm.lp(l).v).co);
            let closest = closest_on_tri_to_point_v3(co, t[0], t[1], t[2]);
            let dist_sq = len_squared_v3v3(co, closest);
            if dist_sq < nearest.dist_sq {
                nearest.dist_sq = dist_sq;
                nearest.index = Some(index);
                nearest.co = closest;
            }
        };
        if let Some(n) = m.tree.find_nearest(center_pos, f32::MAX, &mut cb)
            && n.index.is_some()
        {
            return n.co;
        }
    }
    center_pos
}

/// `bmo_circularize_exec`.
pub(crate) fn circularize(bm: &mut BMesh, i: &CircularizeIn) {
    let check_axis = i.mirror;
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::TAG);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::TAG);
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(ElemFlag::TAG);
    }
    for &g in &i.geom {
        if bm.elem_alive(g) {
            bm.elem_flag_mut(g).insert(ElemFlag::TAG);
        }
    }
    // `bm_vert_chain_extract_from_boundary_edges`.
    let mut visited = vec![false; bm.edges.capacity()];
    let mut chains: Vec<VertChain> = Vec::new();
    for e in bm.edge_ids() {
        if visited[e.slot()] || !is_valid_boundary_edge(bm, e, check_axis) {
            continue;
        }
        if let Some(c) = walk_boundary_chain(bm, e, &mut visited, check_axis) {
            chains.push(c);
        }
    }

    // A BVH of the mesh triangles when the result is projected back on the mesh.
    let bvh = (i.flatten < 1.0).then(|| {
        let looptris = mesh_calc_tessellation(bm);
        let mut tree = BvhTree::new(looptris.len(), 0.0, 8, 8);
        for (k, lt) in looptris.iter().enumerate() {
            let t = lt.map(|l| bm.vert(bm.lp(l).v).co);
            tree.insert(k, &t);
        }
        tree.balance();
        MeshTris { tree, looptris }
    });
    let mut tess_cache = std::collections::HashMap::new();

    for chain_data in &mut chains {
        frap_math::budget::check();
        let chain = &mut chain_data.verts;
        let mut normal_accum = [0.0f32; 3];
        for &v in chain.iter() {
            normal_accum = add3(normal_accum, bm.vert(v).no);
        }
        let (mut mat, mut center_3d) = orientation_matrix_calc(bm, chain);
        // Reverse the winding if the Newell normal opposes the vertex normals.
        if dot3(mat[2], normal_accum) < 0.0 {
            chain.reverse();
            (mat, center_3d) = orientation_matrix_calc(bm, chain);
        }
        let mut mirror_axis = None;
        if !chain_data.is_closed {
            let (s, e) = (
                bm.vert(chain[0]).co,
                bm.vert(*chain.last().expect("chain")).co,
            );
            for k in 0..3 {
                if check_axis[k] && s[k].abs() < MIRROR_LIMIT && e[k].abs() < MIRROR_LIMIT {
                    mirror_axis = Some(k);
                }
            }
        }
        // An open chain on a symmetry plane is centred between its ends.
        if mirror_axis.is_some() {
            let s = bm.vert(chain[0]).co;
            let e = bm.vert(*chain.last().expect("chain")).co;
            center_3d = midpoint3(s, e);
            let p = normalize3(sub3(s, center_3d));
            let q = normalize3(cross3(mat[2], p));
            mat[0] = p;
            mat[1] = q;
        }
        // `project_chain_to_2d`.
        let mut circle_verts: Vec<CircleVert> = chain
            .iter()
            .map(|&v| {
                let vec = sub3(bm.vert(v).co, center_3d);
                CircleVert {
                    v,
                    co_2d: [dot3(vec, mat[0]), dot3(vec, mat[1])],
                    target_2d: [0.0; 2],
                }
            })
            .collect();
        let fixed_center = mirror_axis.map(|_| [0.0f32; 2]);
        let (center_2d, mut radius) = match i.fit_method {
            FitMethod::Contract => circle_inside_fit(&circle_verts, fixed_center),
            FitMethod::LeastSquare => circle_best_fit(&circle_verts, fixed_center),
        };
        if i.custom_radius > 0.0 {
            radius = i.custom_radius;
        }
        calculate_target_locations(
            &mut circle_verts,
            center_2d,
            radius,
            i.regular,
            chain_data.is_closed,
            i.angle,
        );
        let (first, last) = (chain[0], *chain.last().expect("chain"));
        for cv in &circle_verts {
            let target_local = [cv.target_2d[0], cv.target_2d[1], 0.0];
            let mut final_pos = add3(center_3d, mul_m3_v3(&mat, target_local));
            if i.flatten < 1.0 {
                let projected =
                    project_on_mesh(bm, bvh.as_ref(), cv.v, final_pos, mat[2], &mut tess_cache);
                final_pos = interp_v3_v3v3(projected, final_pos, i.flatten);
            }
            // The ends of a mirrored chain go exactly onto the mirror plane.
            if let Some(axis) = mirror_axis
                && (cv.v == first || cv.v == last)
            {
                final_pos[axis] = 0.0;
            }
            let orig = bm.vert(cv.v).co;
            for k in 0..3 {
                if i.lock[k] {
                    final_pos[k] = orig[k];
                }
            }
            bm.vert_mut(cv.v).co = interp_v3_v3v3(orig, final_pos, i.factor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigen_inverse_of_a_3x3() {
        let m: M3 = [[2.0, 0.0, 1.0], [1.0, 3.0, 0.0], [0.0, 1.0, 4.0]];
        let inv = invert_m3(&m).expect("invertible");
        // inv * m = I.
        for (c, mc) in m.iter().enumerate() {
            let col = mul_m3_v3(&inv, *mc);
            for (r, v) in col.iter().enumerate() {
                let want = if r == c { 1.0 } else { 0.0 };
                assert!((v - want).abs() < 1e-6, "{col:?}");
            }
        }
        assert!(invert_m3(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 1.0]]).is_none());
    }

    #[test]
    fn best_fit_finds_the_circle() {
        let verts: Vec<CircleVert> = (0..8)
            .map(|k| {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                CircleVert {
                    v: VertId(0),
                    co_2d: [0.05 + 1.5 * a.cos(), -0.02 + 1.5 * a.sin()],
                    target_2d: [0.0; 2],
                }
            })
            .collect();
        let (c, r) = circle_best_fit(&verts, None);
        // The chain is centred on its centroid, so the start (0, 0, 1) is close.
        assert!(
            (c[0] - 0.05).abs() < 1e-4 && (c[1] + 0.02).abs() < 1e-4,
            "{c:?}"
        );
        assert!((r - 1.5).abs() < 1e-4);
    }
}
