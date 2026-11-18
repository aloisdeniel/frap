// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_convexhull_2d` (Andrew's monotone chain over points sorted by `y` then `x`, with the
//! `USE_CONVEX_CROSS_PRODUCT_ENSURE` clean-up) and `BLI_convexhull_aabb_fit_points_2d` (rotating
//! calipers: the angle whose axis-aligned bounds have the smallest area).
//!
//! Ported from blender/source/blender/blenlib/intern/convexhull_2d.cc (Blender 5.2.4).

use crate::param::bli::{V2, normalize2_div};

/// `is_left`.
#[inline]
fn is_left(p0: V2, p1: V2, p2: V2) -> f32 {
    ((p1[0] - p0[0]) * (p2[1] - p0[1])) - ((p2[0] - p0[0]) * (p1[1] - p0[1]))
}

/// `convexhull_2d_stack_finalize`.
fn stack_finalize(points: &[V2], r_points: &[usize], range: &mut [i64; 2]) {
    let mut bot = range[0];
    let mut top = range[1];
    while top - bot >= 2 {
        let p = |i: i64| points[r_points[i as usize]];
        if is_left(p(top - 1), p(bot), p(top)) >= 0.0 {
            top -= 1;
            continue;
        }
        if is_left(p(top), p(bot + 1), p(bot)) >= 0.0 {
            bot += 1;
            continue;
        }
        break;
    }
    range[0] = bot;
    range[1] = top;
}

/// `convexhull_2d_stack_push`.
#[inline]
fn stack_push(points: &[V2], r_points: &mut [usize], top: &mut i64, index: usize) {
    while *top > 0
        && is_left(
            points[r_points[(*top - 1) as usize]],
            points[index],
            points[r_points[*top as usize]],
        ) >= 0.0
    {
        *top -= 1;
    }
    *top += 1;
    r_points[*top as usize] = index;
}

/// `convexhull_2d_sorted_impl`: returns `top` (the last valid index of `r_points`).
fn sorted_impl(points: &[V2], r_points: &mut [usize]) -> i64 {
    let n = points.len();
    let mut top: i64 = -1;
    let minmin = 0usize;
    let maxmax = n - 1;

    let xmin = points[0][0];
    let mut i = 1usize;
    while i <= maxmax {
        if points[i][0] != xmin {
            break;
        }
        i += 1;
    }
    let minmax = i - 1;
    if minmax == maxmax {
        // Degenerate case: all x-coords == X-min.
        stack_push(points, r_points, &mut top, minmin);
        if points[minmax][1] != points[minmin][1] {
            stack_push(points, r_points, &mut top, minmax);
        }
        return top;
    }

    let xmax = points[maxmax][0];
    let mut j: i64 = maxmax as i64 - 1;
    while j >= 0 {
        if points[j as usize][0] != xmax {
            break;
        }
        j -= 1;
    }
    let maxmin = (j + 1) as usize;

    // Compute the lower hull on the stack.
    stack_push(points, r_points, &mut top, minmin);
    let mut i = minmax;
    loop {
        i += 1;
        if i > maxmin {
            break;
        }
        if i < maxmin && is_left(points[minmin], points[maxmin], points[i]) >= 0.0 {
            continue;
        }
        while top > 0 {
            if is_left(
                points[r_points[(top - 1) as usize]],
                points[r_points[top as usize]],
                points[i],
            ) > 0.0
            {
                break;
            }
            top -= 1;
        }
        stack_push(points, r_points, &mut top, i);
    }

    if maxmax != maxmin {
        stack_push(points, r_points, &mut top, maxmax);
    }

    let bot = top;
    let mut i = maxmin as i64;
    loop {
        i -= 1;
        if i < minmax as i64 {
            break;
        }
        let iu = i as usize;
        if iu > minmax && is_left(points[maxmax], points[minmax], points[iu]) >= 0.0 {
            continue;
        }
        while top > bot {
            if is_left(
                points[r_points[(top - 1) as usize]],
                points[r_points[top as usize]],
                points[iu],
            ) > 0.0
            {
                break;
            }
            top -= 1;
        }
        if points[iu][0] == points[r_points[0]][0] && points[iu][1] == points[r_points[0]][1] {
            return top; // Special case (mgomes).
        }
        stack_push(points, r_points, &mut top, iu);
    }

    if minmax != minmin && r_points[0] != minmin {
        stack_push(points, r_points, &mut top, minmin);
    }
    top
}

/// `BLI_convexhull_2d`: indices of the hull points, counter-clockwise.
pub fn convexhull_2d(points: &[V2]) -> Vec<usize> {
    let n = points.len();
    if n < 2 {
        return (0..n).collect();
    }
    let mut map: Vec<usize> = (0..n).collect();
    // `std::sort` by `y` then `x`; only identical points compare equal.
    map.sort_by(|&a, &b| {
        let (a, b) = (points[a], points[b]);
        a[1].partial_cmp(&b[1])
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(
                a[0].partial_cmp(&b[0])
                    .unwrap_or(core::cmp::Ordering::Equal),
            )
    });
    let sorted: Vec<V2> = map.iter().map(|&i| points[i]).collect();
    let mut r_points = vec![0usize; n];
    let top = sorted_impl(&sorted, &mut r_points);
    let mut range = [0i64, top];
    stack_finalize(&sorted, &r_points, &mut range);
    (range[0]..=range[1])
        .map(|i| map[r_points[i as usize]])
        .collect()
}

/// `sincos_rotate_cw_x`.
#[inline]
fn rotate_cw_x(sincos: V2, p: V2) -> f32 {
    (sincos[0] * p[0]) + (sincos[1] * p[1])
}

/// `sincos_rotate_cw_y`.
#[inline]
fn rotate_cw_y(sincos: V2, p: V2) -> f32 {
    (sincos[1] * p[0]) - (sincos[0] * p[1])
}

/// `sincos_canonical`: the equivalent angle in `[0, 90)` degrees.
fn sincos_canonical(sincos: V2) -> V2 {
    if sincos[0] < 0.0 {
        if sincos[1] < 0.0 {
            [-sincos[0], -sincos[1]]
        } else if sincos[0] == -1.0 && sincos[1] == 0.0 {
            [-sincos[0], sincos[1]]
        } else {
            [sincos[1], -sincos[0]]
        }
    } else if sincos[1] < 0.0 {
        [-sincos[1], sincos[0]]
    } else if sincos[0] == 0.0 && sincos[1] == 1.0 {
        [sincos[1], sincos[0]]
    } else {
        sincos
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct AngleCanonical {
    sincos: V2,
    sincos_canonical: V2,
    index: i64,
}

/// `hull_angle_canonical_cmp`.
fn canonical_cmp(a: &AngleCanonical, b: &AngleCanonical) -> i32 {
    if a.sincos_canonical[0] < b.sincos_canonical[0] {
        return -1;
    }
    if a.sincos_canonical[0] > b.sincos_canonical[0] {
        return 1;
    }
    if a.sincos_canonical[1] > b.sincos_canonical[1] {
        return -1;
    }
    if a.sincos_canonical[1] < b.sincos_canonical[1] {
        return 1;
    }
    if a.index > b.index {
        return -1;
    }
    if a.index < b.index {
        return 1;
    }
    0
}

#[derive(Copy, Clone, Debug, Default)]
struct HullAngleStep {
    next: Option<usize>,
    angle: AngleCanonical,
    index: i64,
    index_max: i64,
}

/// `HullAngleIter`: `axis[axis * 2 + minmax]`.
struct HullAngleIter<'a> {
    axis_ordered: Option<usize>,
    axis: [HullAngleStep; 4],
    points_hull: &'a [V2],
}

impl HullAngleIter<'_> {
    /// `hull_angle_insert_ordered`.
    fn insert_ordered(&mut self, insert: usize) {
        let mut prev: Option<usize> = None;
        let mut iter = self.axis_ordered;
        while let Some(it) = iter {
            if canonical_cmp(&self.axis[it].angle, &self.axis[insert].angle) > 0 {
                prev = Some(it);
                iter = self.axis[it].next;
            } else {
                break;
            }
        }
        match prev {
            None => self.axis_ordered = Some(insert),
            Some(p) => self.axis[p].next = Some(insert),
        }
        self.axis[insert].next = iter;
    }

    /// `convexhull_2d_angle_iter_step_on_axis`.
    fn step_on_axis(&mut self, h: usize) -> bool {
        let n = self.points_hull.len() as i64;
        while self.axis[h].index != self.axis[h].index_max {
            let i_curr = self.axis[h].index;
            let i_next = (i_curr + 1) % n;
            let a = self.points_hull[i_next as usize];
            let b = self.points_hull[i_curr as usize];
            let (sincos, len) = normalize2_div([a[0] - b[0], a[1] - b[1]]);
            self.axis[h].index = i_next;
            if len != 0.0 {
                self.axis[h].angle.sincos = sincos;
                self.axis[h].angle.sincos_canonical = sincos_canonical(sincos);
                self.axis[h].angle.index = i_curr;
                return true;
            }
        }
        self.axis[h].index = -1;
        false
    }

    /// `convexhull_2d_angle_iter_init`.
    fn new(points_hull: &[V2]) -> HullAngleIter<'_> {
        let n = points_hull.len() as i64;
        let mut hiter = HullAngleIter {
            axis_ordered: None,
            axis: [HullAngleStep::default(); 4],
            points_hull,
        };
        let mut range = [[0.0f32; 2]; 2];
        for axis in 0..2 {
            range[axis][0] = points_hull[0][axis];
            range[axis][1] = points_hull[0][axis];
        }
        for (i, p) in points_hull.iter().enumerate().skip(1) {
            for axis in 0..2 {
                if range[axis][0] < p[axis] {
                    range[axis][0] = p[axis];
                    hiter.axis[axis * 2].index = i as i64;
                }
                if range[axis][1] > p[axis] {
                    range[axis][1] = p[axis];
                    hiter.axis[axis * 2 + 1].index = i as i64;
                }
            }
        }
        // Step backwards over any collinear points.
        for axis in 0..2 {
            for i in 0..2 {
                let h = axis * 2 + i;
                let i_orig = hiter.axis[h].index;
                let mut i_curr = i_orig;
                loop {
                    let i_prev = (i_curr + n - 1) % n;
                    if i_prev == i_orig {
                        break;
                    }
                    let a = points_hull[i_curr as usize];
                    let b = points_hull[i_prev as usize];
                    let (sincos_test, len) = normalize2_div([a[0] - b[0], a[1] - b[1]]);
                    if len != 0.0 {
                        if sincos_test[axis].abs() > 0.5 {
                            break;
                        }
                        if sincos_canonical(sincos_test)[0] != 1.0 {
                            break;
                        }
                    }
                    i_curr = i_prev;
                    hiter.axis[h].index = i_curr;
                }
            }
        }
        hiter.axis[0].index_max = hiter.axis[2].index; // West to south.
        hiter.axis[2].index_max = hiter.axis[1].index; // South to east.
        hiter.axis[1].index_max = hiter.axis[3].index; // East to north.
        hiter.axis[3].index_max = hiter.axis[0].index; // North to west.

        for axis in 0..2 {
            for i in 0..2 {
                let h = axis * 2 + i;
                hiter.axis[h].angle.index = hiter.axis[h].index;
                if hiter.step_on_axis(h) {
                    hiter.insert_ordered(h);
                }
            }
        }
        hiter
    }

    /// `convexhull_2d_angle_iter_step`.
    fn step(&mut self) {
        let Some(h) = self.axis_ordered else { return };
        self.axis_ordered = self.axis[h].next;
        if self.step_on_axis(h) {
            self.insert_ordered(h);
        }
    }
}

/// `convexhull_2d_compute_extent_on_axis<Axis, AxisSign>`.
fn extent_on_axis(points_hull: &[V2], axis: usize, sign: i32, sincos: V2, index: &mut i64) -> f32 {
    let n = points_hull.len() as i64;
    let value = |i: i64| {
        let p = points_hull[i as usize];
        if axis == 0 {
            rotate_cw_x(sincos, p)
        } else {
            rotate_cw_y(sincos, p)
        }
    };
    let index_init = *index;
    let mut index_best = index_init;
    let mut value_best = value(index_best);
    for count in 1..n {
        let index_test = (index_init + count) % n;
        let value_test = value(index_test);
        if if sign == -1 {
            value_test > value_best
        } else {
            value_test < value_best
        } {
            break;
        }
        value_best = value_test;
        index_best = index_test;
    }
    *index = index_best;
    value_best
}

/// `convexhull_aabb_fit_hull_2d`.
fn aabb_fit_hull_2d(points_hull: &[V2]) -> f32 {
    let mut area_best = f32::MAX;
    let mut sincos_best: V2 = [0.0, 1.0];
    let mut index_best = i64::from(i32::MAX);

    let mut hull_iter = HullAngleIter::new(points_hull);
    // [axis] = (min, max).
    let mut bounds_index = [
        [hull_iter.axis[1].angle.index, hull_iter.axis[0].angle.index],
        [hull_iter.axis[2].angle.index, hull_iter.axis[3].angle.index],
    ];
    while let Some(h) = hull_iter.axis_ordered {
        let angle = hull_iter.axis[h].angle;
        let sc = angle.sincos_canonical;
        let b0min = extent_on_axis(points_hull, 0, -1, sc, &mut bounds_index[0][0]);
        let b0max = extent_on_axis(points_hull, 0, 1, sc, &mut bounds_index[0][1]);
        let b1min = extent_on_axis(points_hull, 1, -1, sc, &mut bounds_index[1][0]);
        let b1max = extent_on_axis(points_hull, 1, 1, sc, &mut bounds_index[1][1]);
        let area_test = (b0max - b0min) * (b1max - b1min);
        if area_test < area_best || (area_test == area_best && angle.index < index_best) {
            area_best = area_test;
            sincos_best = angle.sincos;
            index_best = angle.index;
        }
        hull_iter.step();
    }
    if area_best != f32::MAX {
        libm::atan2(f64::from(sincos_best[0]), f64::from(sincos_best[1])) as f32
    } else {
        0.0
    }
}

/// `BLI_convexhull_aabb_fit_points_2d`: the angle (radians) to rotate `points` by so their
/// axis-aligned bounding box has the smallest area.
pub fn convexhull_aabb_fit_points_2d(points: &[V2]) -> f32 {
    let hull = convexhull_2d(points);
    if hull.len() > 1 {
        let points_hull: Vec<V2> = hull.iter().map(|&i| points[i]).collect();
        aabb_fit_hull_2d(&points_hull)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_hull_is_ccw() {
        let pts = [[0.0, 0.0], [1.0, 0.0], [0.5, 0.5], [1.0, 1.0], [0.0, 1.0]];
        let h = convexhull_2d(&pts);
        assert_eq!(h.len(), 4);
        assert!(!h.contains(&2));
        // Counter-clockwise: positive signed area.
        let mut a = 0.0;
        for i in 0..h.len() {
            let (p, q) = (pts[h[i]], pts[h[(i + 1) % h.len()]]);
            a += p[0] * q[1] - q[0] * p[1];
        }
        assert!(a > 0.0);
    }

    #[test]
    fn fit_angle_of_rotated_rectangle() {
        let ang = 0.3f32;
        let (s, c) = (libm::sinf(ang), libm::cosf(ang));
        let rect = [[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [0.0, 1.0]];
        let pts: Vec<V2> = rect
            .iter()
            .map(|p| [c * p[0] - s * p[1], s * p[0] + c * p[1]])
            .collect();
        let fit = convexhull_aabb_fit_points_2d(&pts);
        // Rotating counter-clockwise by `fit` (`angle_to_mat2`) aligns the rectangle.
        let r = (fit + ang).rem_euclid(core::f32::consts::FRAC_PI_2);
        assert!(
            r < 1e-5 || core::f32::consts::FRAC_PI_2 - r < 1e-5,
            "{fit} vs {ang}"
        );
    }

    #[test]
    fn degenerate_inputs() {
        assert!(convexhull_2d(&[]).is_empty());
        assert_eq!(convexhull_2d(&[[1.0, 1.0]]), vec![0]);
        assert_eq!(
            convexhull_aabb_fit_points_2d(&[[0.0, 0.0], [0.0, 0.0]]),
            0.0
        );
    }
}
