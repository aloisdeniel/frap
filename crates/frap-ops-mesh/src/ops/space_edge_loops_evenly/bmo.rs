// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.space_edge_loops_evenly(bmesh, geom=[], interpolation='CUBIC', factor=0.0,
//! lock_x=False, lock_y=False, lock_z=False)` (new in 5.x), with the `length_parameterize`,
//! `binary_search` and tridiagonal solver helpers it uses.
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_space_edge_loops_evenly.cc,
//! blender/source/blender/blenlib/BLI_length_parameterize.hh, intern/length_parameterize.cc,
//! BLI_binary_search.hh and intern/math_solvers.cc (Blender 5.2.4, GPL-2.0-or-later).
//!
//! The edges of `geom` are walked into chains (a vertex with three or more of them ends a
//! chain; a chain whose ends share an edge of `geom` is closed); chains whose vertices all
//! coincide are skipped. Every chain's vertices move to evenly spaced arc lengths: on the
//! polyline (`LINEAR`) or on a natural cubic spline through the vertices parameterised by arc
//! length (`CUBIC`; periodic for closed chains, Sherman–Morrison). Open chains keep their end
//! vertices; locked axes keep their coordinate; `factor` blends towards the new position.

use frap_bmesh::{BMElem, BMesh, EdgeId, ElemFlag, VertId};

/// `DUPLICATE_POSITION_THRESHOLD`.
const DUPLICATE_POSITION_THRESHOLD: f32 = 1e-6;
/// `SPACE_EPSILON`.
const SPACE_EPSILON: f32 = 1e-8;

/// `SpaceInterpolationMethod`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Interpolation {
    /// `SPACE_EDGE_LOOPS_EVENLY_INTERP_CUBIC`.
    Cubic,
    /// `SPACE_EDGE_LOOPS_EVENLY_INTERP_LINEAR`.
    Linear,
}

/// The input slots.
pub(crate) struct SpaceIn {
    pub geom: Vec<EdgeId>,
    pub interpolation: Interpolation,
    pub factor: f32,
    pub lock: [bool; 3],
}

type V3 = [f32; 3];

/// `SpaceChainData`.
struct Chain {
    verts: Vec<VertId>,
    is_closed: bool,
}

/// `SplineCoeffs`.
#[derive(Copy, Clone, Debug)]
struct SplineCoeffs {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    x: f32,
}

fn distance(a: V3, b: V3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn distance_squared(a: V3, b: V3) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

fn tagged(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::TAG)
}

/// `vert_next_walk_edge`: the first unvisited tagged edge of `v`, `None` at a junction (three
/// or more tagged edges) or a dead end.
fn vert_next_walk_edge(bm: &BMesh, v: VertId, visited: &[bool]) -> Option<EdgeId> {
    let mut e_next = None;
    let mut tagged_count = 0;
    for e in bm.vert_edges(v) {
        if !tagged(bm, e) {
            continue;
        }
        tagged_count += 1;
        if tagged_count >= 3 {
            return None;
        }
        if e_next.is_none() && !visited[e.slot()] {
            e_next = Some(e);
        }
    }
    e_next
}

/// `walk_edges`.
fn walk_edges(bm: &BMesh, start: EdgeId, visited: &mut [bool]) -> Option<Chain> {
    let [sv1, sv2] = bm.edge(start).v;
    let mut verts = vec![sv1, sv2];
    let mut visited_verts = vec![false; bm.verts.capacity()];
    visited_verts[sv1.slot()] = true;
    visited_verts[sv2.slot()] = true;
    visited[start.slot()] = true;

    let mut walk = |mut v_curr: VertId, result: &mut Vec<VertId>, visited: &mut [bool]| {
        while let Some(e_next) = vert_next_walk_edge(bm, v_curr, visited) {
            let v_next = bm.edge_other_vert(e_next, v_curr);
            if visited_verts[v_next.slot()] {
                break;
            }
            v_curr = v_next;
            visited_verts[v_curr.slot()] = true;
            result.push(v_curr);
            visited[e_next.slot()] = true;
        }
    };
    // The initial edge direction (v1 -> v2) is arbitrary: walk from v2 first.
    walk(sv2, &mut verts, visited);
    let mut pre_chain = Vec::new();
    walk(sv1, &mut pre_chain, visited);
    if !pre_chain.is_empty() {
        pre_chain.reverse();
        pre_chain.extend_from_slice(&verts);
        verts = pre_chain;
    }
    // Skip chains where all vertices are at the same location.
    let all_duplicate = verts.windows(2).all(|w| {
        distance_squared(bm.vert(w[0]).co, bm.vert(w[1]).co)
            <= DUPLICATE_POSITION_THRESHOLD * DUPLICATE_POSITION_THRESHOLD
    });
    if all_duplicate {
        return None;
    }
    // Close the ring, ensuring the closing vertex is *not* a junction.
    let (v_first, v_last) = (verts[0], *verts.last().expect("two vertices"));
    let is_closed = match bm.edge_exists(v_first, v_last) {
        Some(ce)
            if tagged(bm, ce)
                && vert_next_walk_edge(bm, v_first, visited) == Some(ce)
                && vert_next_walk_edge(bm, v_last, visited) == Some(ce) =>
        {
            visited[ce.slot()] = true;
            true
        }
        _ => false,
    };
    Some(Chain { verts, is_closed })
}

/// `length_parameterize::sample_at_length` (the hint does not change the result).
fn sample_at_length(lengths: &[f32], sample_length: f32) -> (usize, f32) {
    let total_length = *lengths.last().expect("non-empty");
    if sample_length >= total_length {
        return (lengths.len() - 1, 1.0);
    }
    // `std::upper_bound`.
    let prev_point_index = lengths.partition_point(|&l| l <= sample_length);
    let segment_start = if prev_point_index == 0 {
        0.0
    } else {
        lengths[prev_point_index - 1]
    };
    let segment_end = lengths[prev_point_index];
    let segment_length = segment_end - segment_start;
    // `math::safe_divide(1.0f, segment_length)`.
    let inv = if segment_length == 0.0 {
        0.0
    } else {
        1.0 / segment_length
    };
    let factor = (sample_length - segment_start) * inv;
    (prev_point_index, factor)
}

/// `length_parameterize::sample_uniform`.
fn sample_uniform(lengths: &[f32], include_last_point: bool, count: usize) -> Vec<(usize, f32)> {
    if count == 1 {
        return vec![(0, 0.0)];
    }
    let total_length = *lengths.last().expect("non-empty");
    let step_length = total_length / (count - usize::from(include_last_point)) as f32;
    (0..count)
        .map(|i| {
            // Use minimum to avoid issues with floating point accuracy.
            let sample_length = total_length.min(i as f32 * step_length);
            sample_at_length(lengths, sample_length)
        })
        .collect()
}

/// `BLI_tridiagonal_solve` (Thomas algorithm in double precision).
fn tridiagonal_solve(a: &[f32], b: &[f32], c: &[f32], d: &[f32], x: &mut [f32]) -> bool {
    let count = b.len();
    if count < 1 {
        return false;
    }
    let mut c1 = vec![0.0f64; count];
    let mut d1 = vec![0.0f64; count];
    let mut c_prev = f64::from(c[0]) / f64::from(b[0]);
    let mut d_prev = f64::from(d[0]) / f64::from(b[0]);
    c1[0] = c_prev;
    d1[0] = d_prev;
    for i in 1..count {
        let denum = f64::from(b[i]) - f64::from(a[i]) * c_prev;
        c_prev = f64::from(c[i]) / denum;
        d_prev = (f64::from(d[i]) - f64::from(a[i]) * d_prev) / denum;
        c1[i] = c_prev;
        d1[i] = d_prev;
    }
    let mut x_prev = d_prev;
    x[count - 1] = x_prev as f32;
    for i in (0..count - 1).rev() {
        x_prev = d1[i] - c1[i] * x_prev;
        x[i] = x_prev as f32;
    }
    x_prev.is_finite()
}

/// `BLI_tridiagonal_solve_cyclic` (Sherman–Morrison).
fn tridiagonal_solve_cyclic(a: &[f32], b: &[f32], c: &[f32], d: &[f32], x: &mut [f32]) -> bool {
    let count = b.len();
    if count < 1 {
        return false;
    }
    if count == 1 {
        x[0] = d[0] / (a[0] + b[0] + c[0]);
        return x[0].is_finite();
    }
    if count == 2 {
        let a2 = [0.0, a[1] + c[1]];
        let c2 = [a[0] + c[0], 0.0];
        return tridiagonal_solve(&a2, b, &c2, d, x);
    }
    let (a0, cn) = (a[0], c[count - 1]);
    if a0 == 0.0 && cn == 0.0 {
        return tridiagonal_solve(a, b, c, d, x);
    }
    let mut b2 = b.to_vec();
    b2[0] -= a0;
    b2[count - 1] -= cn;
    let mut tmp = vec![0.0f32; count];
    tmp[0] = a0;
    tmp[count - 1] = cn;
    // Solve for the partial solution and the adjustment vector (`tmp` is both the right-hand
    // side and the result).
    let rhs = tmp.clone();
    let success =
        tridiagonal_solve(a, &b2, c, &rhs, &mut tmp) && tridiagonal_solve(a, &b2, c, d, x);
    if success {
        let coeff = (x[0] + x[count - 1]) / (1.0 + tmp[0] + tmp[count - 1]);
        for i in 0..count {
            x[i] -= coeff * tmp[i];
        }
    }
    success
}

/// `calculate_splines_axis`.
fn calculate_splines_axis(distances: &[f32], coords: &[f32], is_closed: bool) -> Vec<SplineCoeffs> {
    let n = coords.len();
    if n < 2 {
        return Vec::new();
    }
    let num_segments = if is_closed { n } else { n - 1 };
    let mut segment_length = vec![0.0f32; num_segments];
    for (i, sl) in segment_length.iter_mut().enumerate() {
        *sl = distances[i + 1] - distances[i];
        // `!(segment_length[i] > 0.0f)`: NaN too.
        if (*sl).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            *sl = SPACE_EPSILON;
        }
    }
    // Second derivative coefficients; zero at the ends of a natural spline.
    let mut c_vals = vec![0.0f32; n];
    if is_closed {
        let mut lower = vec![0.0f32; n];
        let mut diag = vec![0.0f32; n];
        let mut upper = vec![0.0f32; n];
        let mut rhs = vec![0.0f32; n];
        for i in 0..n {
            let i_prev = (i + n - 1) % n;
            let i_next = (i + 1) % n;
            lower[i] = segment_length[i_prev];
            diag[i] = 2.0 * (segment_length[i_prev] + segment_length[i]);
            upper[i] = segment_length[i];
            rhs[i] = 3.0
                * (((coords[i_next] - coords[i]) / segment_length[i])
                    - ((coords[i] - coords[i_prev]) / segment_length[i_prev]));
        }
        tridiagonal_solve_cyclic(&lower, &diag, &upper, &rhs, &mut c_vals);
    } else {
        let interior = n - 2;
        let mut lower = vec![0.0f32; interior];
        let mut diag = vec![0.0f32; interior];
        let mut upper = vec![0.0f32; interior];
        let mut rhs = vec![0.0f32; interior];
        for i_curr in 0..interior {
            let i_next = i_curr + 1;
            lower[i_curr] = segment_length[i_curr];
            diag[i_curr] = 2.0 * (segment_length[i_curr] + segment_length[i_next]);
            upper[i_curr] = segment_length[i_next];
            rhs[i_curr] = 3.0
                * (((coords[i_next + 1] - coords[i_next]) / segment_length[i_next])
                    - ((coords[i_next] - coords[i_curr]) / segment_length[i_curr]));
        }
        tridiagonal_solve(&lower, &diag, &upper, &rhs, &mut c_vals[1..1 + interior]);
    }
    // Polynomial coefficients per segment.
    (0..num_segments)
        .map(|i| {
            let i_next = if is_closed { (i + 1) % n } else { i + 1 };
            let sl = segment_length[i];
            SplineCoeffs {
                a: coords[i],
                b: ((coords[i_next] - coords[i]) / sl)
                    - (sl * (c_vals[i_next] + 2.0 * c_vals[i])) / 3.0,
                c: c_vals[i],
                d: (c_vals[i_next] - c_vals[i]) / (3.0 * sl),
                x: distances[i],
            }
        })
        .collect()
}

/// `calc_spline_segment`: `binary_search::last_if(knots, value <= target)` clamped to
/// `[0, len - 2]`.
fn calc_spline_segment(knots: &[f32], target: f32) -> usize {
    let idx = knots.partition_point(|&v| v <= target) as i64 - 1;
    idx.clamp(0, knots.len() as i64 - 2) as usize
}

/// `evaluate_cubic`.
fn evaluate_cubic(knots: &[f32], c: &[Vec<SplineCoeffs>; 3], target: f32) -> V3 {
    let seg = calc_spline_segment(knots, target);
    let dt = target - c[0][seg].x;
    let e = |s: &SplineCoeffs| s.a + dt * (s.b + dt * (s.c + dt * s.d));
    [e(&c[0][seg]), e(&c[1][seg]), e(&c[2][seg])]
}

/// `bmo_space_edge_loops_evenly_exec`.
pub(crate) fn space_edge_loops_evenly(bm: &mut BMesh, i: &SpaceIn) {
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::TAG);
    }
    for &e in &i.geom {
        if bm.elem_alive(BMElem::Edge(e)) {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
    }
    // `get_space_input_chains`.
    let mut visited = vec![false; bm.edges.capacity()];
    let mut chains = Vec::new();
    for e in bm.edge_ids() {
        if !tagged(bm, e) || visited[e.slot()] {
            continue;
        }
        if let Some(c) = walk_edges(bm, e, &mut visited) {
            chains.push(c);
        }
    }

    for chain in &chains {
        frap_math::budget::check();
        let n = chain.verts.len();
        // `measure_chain`.
        let positions: Vec<V3> = chain.verts.iter().map(|&v| bm.vert(v).co).collect();
        let mut knots = vec![0.0f32; n + usize::from(chain.is_closed)];
        // `accumulate_lengths`.
        let mut length = 0.0f32;
        for k in 0..n - 1 {
            length += distance(positions[k], positions[k + 1]);
            knots[k + 1] = length;
        }
        if chain.is_closed && n > 1 {
            knots[n] = length + distance(positions[n - 1], positions[0]);
        }
        let samples = sample_uniform(&knots[1..], !chain.is_closed, n);
        let new_positions: Vec<V3> = match i.interpolation {
            Interpolation::Linear => samples
                .iter()
                .map(|&(prev, f)| {
                    // `interpolate_to_masked`: the last index wraps to the first point.
                    let (a, b) = if prev == n - 1 {
                        (positions[n - 1], positions[0])
                    } else {
                        (positions[prev], positions[prev + 1])
                    };
                    [
                        a[0] * (1.0 - f) + b[0] * f,
                        a[1] * (1.0 - f) + b[1] * f,
                        a[2] * (1.0 - f) + b[2] * f,
                    ]
                })
                .collect(),
            Interpolation::Cubic => {
                let axis = |k: usize| positions.iter().map(|p| p[k]).collect::<Vec<f32>>();
                let coeffs = [
                    calculate_splines_axis(&knots, &axis(0), chain.is_closed),
                    calculate_splines_axis(&knots, &axis(1), chain.is_closed),
                    calculate_splines_axis(&knots, &axis(2), chain.is_closed),
                ];
                samples
                    .iter()
                    .map(|&(seg, f)| {
                        let (k0, k1) = (knots[seg], knots[seg + 1]);
                        let target = k0 * (1.0 - f) + k1 * f;
                        evaluate_cubic(&knots, &coeffs, target)
                    })
                    .collect()
            }
        };
        for k in 0..n {
            // The ends of an open chain are anchors.
            if !chain.is_closed && (k == 0 || k == n - 1) {
                continue;
            }
            let mut new_pos = new_positions[k];
            for a in 0..3 {
                if i.lock[a] {
                    new_pos[a] = positions[k][a];
                }
            }
            let t = i.factor;
            let p = positions[k];
            bm.vert_mut(chain.verts[k]).co = [
                p[0] * (1.0 - t) + new_pos[0] * t,
                p[1] * (1.0 - t) + new_pos[1] * t,
                p[2] * (1.0 - t) + new_pos[2] * t,
            ];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tridiagonal_solves_a_small_system() {
        // [2 1 0; 1 2 1; 0 1 2] x = [4 8 8] -> x = [1 2 3].
        let mut x = [0.0f32; 3];
        assert!(tridiagonal_solve(
            &[0.0, 1.0, 1.0],
            &[2.0, 2.0, 2.0],
            &[1.0, 1.0, 0.0],
            &[4.0, 8.0, 8.0],
            &mut x
        ));
        for (a, b) in x.iter().zip([1.0, 2.0, 3.0]) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn uniform_samples_hit_the_ends() {
        let s = sample_uniform(&[1.0, 3.0, 4.0], true, 4);
        assert_eq!(s[0], (0, 0.0));
        assert_eq!(s[3], (2, 1.0));
        // 4/3 lies in the second segment [1, 3].
        assert_eq!(s[1].0, 1);
    }
}
