// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.align(bb_quality=True, align_mode='OPT_2', relative_to='OPT_4',
//! align_axis=set())`, tier T1 (`tests/golden/cases/object/transform_m8/align_*.py`,
//! WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object_align.py (`align_objects`,
//! `worldspace_bounds_from_object_bounds`, `worldspace_bounds_from_object_data`), Blender
//! 5.2.4, GPL-2.0-or-later. Poll `context.mode == 'OBJECT'`.
//!
//! Moves the selected objects (their `location`) so that their world bounds line up on each
//! axis of `align_axis`: by their negative sides (`OPT_1`), centers (`OPT_2`) or positive
//! sides (`OPT_3`), relative to the scene origin (`OPT_1`), the 3D cursor (`OPT_2`), the
//! selection's bounds (`OPT_3`) or the active object's (`OPT_4`, which lines the negative
//! sides up with the active object's negative side, and so on). The bounds are the world
//! bounds of the evaluated mesh's vertices (`bb_quality`, meshes) or of the `bound_box`
//! corners (other objects and `bb_quality=False`; all zero for objects without bounds); the
//! world matrices are those before the operator. The Python arithmetic is kept: matrix
//! products in single precision with double sums (`mathutils`), the offsets in double
//! precision, stored as float. Nothing selected → WARNING `No objects with bound-box selected`
//! and `CANCELLED`. With `relative_to='OPT_4'`, an axis and no active object among the
//! selection, the add-on fails on an unbound variable; frap answers that `UnboundLocalError`
//! as a `RuntimeError` before changing anything (Blender keeps the axes it already moved).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;
use frap_spec_ops_object::object::align::{AlignMode, RelativeTo};
use glam::Mat4;

use crate::support::xform::Worlds;

/// `matrix @ Vector(v)` (mathutils `column_vector_multiplication`): float products summed in
/// double precision, rounded to float.
fn mul_point(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    let c = m.to_cols_array_2d();
    let v4 = [v[0], v[1], v[2], 1.0];
    std::array::from_fn(|row| {
        let mut dot = 0.0f64;
        for (col, x) in v4.iter().enumerate() {
            dot += f64::from(c[col][row] * x);
        }
        dot as f32
    })
}

/// `(Vector((left, front, up)), Vector((right, back, down)))` of a point set (the last point
/// starts both, as the add-on initializes from the 8th corner or the last vertex).
fn bounds(points: &[[f32; 3]]) -> ([f64; 3], [f64; 3]) {
    let last = points.last().copied().unwrap_or([0.0; 3]);
    let (mut left, mut right) = (last[0], last[0]);
    let (mut front, mut back) = (last[1], last[1]);
    let (mut down, mut up) = (last[2], last[2]);
    for p in points {
        left = left.min_by_lt(p[0]);
        right = right.max_by_gt(p[0]);
        front = front.min_by_lt(p[1]);
        back = back.max_by_gt(p[1]);
        down = down.min_by_lt(p[2]);
        up = up.max_by_gt(p[2]);
    }
    let f = f64::from;
    ([f(left), f(front), f(up)], [f(right), f(back), f(down)])
}

/// The add-on's `if val < left: left = val` comparisons (NaN never replaces).
trait PyMinMax {
    fn min_by_lt(self, v: Self) -> Self;
    fn max_by_gt(self, v: Self) -> Self;
}

impl<T: PartialOrd + Copy> PyMinMax for T {
    fn min_by_lt(self, v: T) -> T {
        if v < self { v } else { self }
    }
    fn max_by_gt(self, v: T) -> T {
        if v > self { v } else { self }
    }
}

fn unbound(name: &str) -> OpError {
    OpError::Runtime(format!(
        "Python: UnboundLocalError: cannot access local variable '{name}' where it is not \
         associated with a value (object.align with relative_to='OPT_4' needs the active \
         object among the selection)"
    ))
}

#[operator(poll = poll::OBJECTMODE, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::Align) -> OpResult {
    let axes = [
        p.align_axis.value.contains("X"),
        p.align_axis.value.contains("Y"),
        p.align_axis.value.contains("Z"),
    ];
    let mode = p.align_mode.value;
    let relative = p.relative_to.value;
    let bb_quality = p.bb_quality.value;
    let (objects, active, cursor, worlds) = {
        let c = cx.ctx();
        (
            c.selected_objects(),
            c.active_object(),
            c.cursor().location,
            Worlds::of(c.main),
        )
    };
    if objects.is_empty() {
        cx.report(ReportType::Warning, "No objects with bound-box selected");
        return Ok(OpStatus::Cancelled);
    }
    // World bounds of every object (both loops of the add-on compute the same ones).
    let mut gbb: Vec<([f64; 3], [f64; 3])> = Vec::with_capacity(objects.len());
    for &ob in &objects {
        let m = worlds.get(ob);
        let is_mesh = cx.main().objects.get(ob).map(|o| o.object_type()) == Some("MESH");
        let ev = cx.evaluated(ob)?;
        let points: Vec<[f32; 3]> = if bb_quality && is_mesh {
            match ev.mesh.as_deref().map(|me| me.positions()) {
                Some(pos) if !pos.is_empty() => pos.iter().map(|v| mul_point(&m, *v)).collect(),
                _ => vec![mul_point(&m, [0.0; 3])],
            }
        } else {
            frap_eval::bound_box(&ev)
                .iter()
                .map(|v| mul_point(&m, *v))
                .collect()
        };
        gbb.push(bounds(&points));
    }
    // Active center and half size; selection bounds.
    let mut center_active: Option<[f64; 3]> = None;
    let mut size_active: Option<[f64; 3]> = None;
    let (mut lfu_sel, mut rbd_sel) = gbb[0];
    for (i, &ob) in objects.iter().enumerate() {
        let (lfu, rbd) = gbb[i];
        if Some(ob) == active {
            center_active = Some(std::array::from_fn(|k| (lfu[k] + rbd[k]) / 2.0));
            size_active = Some([
                (rbd[0] - lfu[0]) / 2.0,
                (rbd[1] - lfu[1]) / 2.0,
                (lfu[2] - rbd[2]) / 2.0,
            ]);
        }
        if i > 0 {
            lfu_sel[0] = lfu_sel[0].min_by_lt(lfu[0]);
            lfu_sel[1] = lfu_sel[1].min_by_lt(lfu[1]);
            lfu_sel[2] = lfu_sel[2].max_by_gt(lfu[2]);
            rbd_sel[0] = rbd_sel[0].max_by_gt(rbd[0]);
            rbd_sel[1] = rbd_sel[1].max_by_gt(rbd[1]);
            rbd_sel[2] = rbd_sel[2].min_by_lt(rbd[2]);
        }
    }
    let center_sel: [f64; 3] = std::array::from_fn(|k| (lfu_sel[k] + rbd_sel[k]) / 2.0);
    // Main loop: one axis at a time, the location read live.
    let mut writes: Vec<(usize, usize, f32)> = Vec::new();
    for (i, &ob) in objects.iter().enumerate() {
        let (lfu, rbd) = gbb[i];
        let center: [f64; 3] = std::array::from_fn(|k| (lfu[k] + rbd[k]) / 2.0);
        let positive = [rbd[0], rbd[1], lfu[2]];
        let negative = [lfu[0], lfu[1], rbd[2]];
        let loc = cx.main().objects.get(ob).map_or([0.0; 3], |o| o.location);
        for k in (0..3).filter(|k| axes[*k]) {
            let l = f64::from(loc[k]);
            let obj = if relative == RelativeTo::Opt4 {
                match mode {
                    AlignMode::Opt1 => {
                        l - negative[k] - size_active.ok_or_else(|| unbound("size_active_x"))?[k]
                    }
                    AlignMode::Opt3 => {
                        l - positive[k] + size_active.ok_or_else(|| unbound("size_active_x"))?[k]
                    }
                    AlignMode::Opt2 => l - center[k],
                }
            } else {
                match mode {
                    AlignMode::Opt1 => l - negative[k],
                    AlignMode::Opt3 => l - positive[k],
                    AlignMode::Opt2 => l - center[k],
                }
            };
            let v = match relative {
                RelativeTo::Opt1 => obj,
                RelativeTo::Opt2 => obj + f64::from(cursor[k]),
                RelativeTo::Opt3 => obj + center_sel[k],
                RelativeTo::Opt4 => {
                    obj + center_active.ok_or_else(|| unbound("center_active_x"))?[k]
                }
            };
            writes.push((i, k, v as f32));
        }
    }
    for (i, k, v) in writes {
        cx.object_mut(objects[i])?.location[k] = v;
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_start_from_the_last_point() {
        let (lfu, rbd) = bounds(&[[1.0, 2.0, 3.0], [-1.0, 5.0, 0.5]]);
        assert_eq!(lfu, [-1.0, 2.0, 3.0]);
        assert_eq!(rbd, [1.0, 5.0, 0.5]);
        let p = mul_point(&Mat4::from_translation(glam::Vec3::X), [1.0, 1.0, 1.0]);
        assert_eq!(p, [2.0, 1.0, 1.0]);
    }
}
