// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.bound_box` and `Object.dimensions` from an evaluated object (§7.3).
//!
//! Provenance: Blender 5.2.4 (GPL-2.0-or-later), `blenkernel/intern/object.cc`
//! (`BKE_object_dimensions_get`, `BKE_object_dimensions_set_ex`, `BKE_boundbox_init_from_minmax`)
//! and `blenlib/intern/math_matrix_c.cc` (`mat4_to_size`).

use frap_math::matrix::len_v3;
use frap_scene::EvaluatedObject;

/// `Object.bound_box`: the eight corners of the local bounds in Blender's order
/// (`BKE_boundbox_init_from_minmax`: x is min for corners 0–3, y is min for 0, 1, 4, 5, z is
/// min for 0, 3, 4, 7); all zero for an object without bounds (empties, cameras, lights, a mesh
/// without vertices).
pub fn bound_box(ob: &EvaluatedObject) -> [[f32; 3]; 8] {
    let Some([lo, hi]) = ob.bounds else {
        return [[0.0; 3]; 8];
    };
    let x = [lo[0], lo[0], lo[0], lo[0], hi[0], hi[0], hi[0], hi[0]];
    let y = [lo[1], lo[1], hi[1], hi[1], lo[1], lo[1], hi[1], hi[1]];
    let z = [lo[2], hi[2], hi[2], lo[2], lo[2], hi[2], hi[2], lo[2]];
    std::array::from_fn(|i| [x[i], y[i], z[i]])
}

/// `mat4_to_size` of `matrix_world`: the lengths of its first three columns.
fn world_size(ob: &EvaluatedObject) -> [f32; 3] {
    let m = ob.matrix_world.to_cols_array_2d();
    [
        len_v3([m[0][0], m[0][1], m[0][2]]),
        len_v3([m[1][0], m[1][1], m[1][2]]),
        len_v3([m[2][0], m[2][1], m[2][2]]),
    ]
}

/// `Object.dimensions` (`BKE_object_dimensions_get`): the bounds' extent times the world
/// matrix's axis lengths (always positive, parent scale included); zero without bounds.
pub fn dimensions(ob: &EvaluatedObject) -> [f32; 3] {
    let Some([lo, hi]) = ob.bounds else {
        return [0.0; 3];
    };
    let s = world_size(ob);
    std::array::from_fn(|i| s[i].abs() * (hi[i] - lo[i]))
}

/// The `scale` that `Object.dimensions = value` writes (`BKE_object_dimensions_set` with no
/// axis mask, as `rna_Object_dimensions_set` calls it): per axis, `value / extent` with the
/// sign of the current scale; an axis whose result is not finite (zero extent) keeps its scale,
/// and so does every axis of an object without bounds.
pub fn scale_for_dimensions(ob: &EvaluatedObject, value: [f32; 3], scale: [f32; 3]) -> [f32; 3] {
    let Some([lo, hi]) = ob.bounds else {
        return scale;
    };
    std::array::from_fn(|i| {
        let s = (value[i] / (hi[i] - lo[i])).copysign(scale[i]);
        if s.is_finite() { s } else { scale[i] }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    fn eo(bounds: Option<[[f32; 3]; 2]>, m: Mat4) -> EvaluatedObject {
        EvaluatedObject {
            matrix_world: m,
            mesh: None,
            bounds,
            pose: None,
            modifier_status: Vec::new(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn no_bounds_reads_zero() {
        let o = eo(None, Mat4::from_scale(glam::Vec3::splat(2.0)));
        assert_eq!(dimensions(&o), [0.0; 3]);
        assert_eq!(bound_box(&o), [[0.0; 3]; 8]);
        assert_eq!(scale_for_dimensions(&o, [1.0; 3], [3.0; 3]), [3.0; 3]);
    }

    #[test]
    fn dimensions_use_absolute_world_scale() {
        let o = eo(
            Some([[-1.0; 3], [1.0; 3]]),
            Mat4::from_scale(glam::Vec3::new(1.0, -2.0, 3.0)),
        );
        assert_eq!(dimensions(&o), [2.0, 4.0, 6.0]);
        assert_eq!(bound_box(&o)[0], [-1.0; 3]);
        assert_eq!(bound_box(&o)[6], [1.0; 3]);
    }

    #[test]
    fn dimensions_setter_keeps_sign_and_skips_flat_axes() {
        let o = eo(Some([[-1.0, -1.0, 0.0], [1.0, 1.0, 0.0]]), Mat4::IDENTITY);
        assert_eq!(
            scale_for_dimensions(&o, [4.0, 1.0, 5.0], [1.0, -3.0, 7.0]),
            [2.0, -0.5, 7.0]
        );
    }
}
