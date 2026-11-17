// SPDX-License-Identifier: GPL-3.0-or-later
//! Axis constraints (`constraint_axis`) as the non-interactive transform applies them. Ports of
//! the constraint part of `initTransInfo` (`transform_generics.cc`), `setUserConstraint`,
//! `projection_matrix_calc`, `applyAxisConstraintVec/Size/Rot` and
//! `applyObjectConstraintVec/Size/Rot` (`editors/transform/transform_constraints.cc`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4: when an array `value` is given (translate, resize), Blender constrains to
//! all three axes and ignores `constraint_axis` (`translate(value=(1, 2, 3),
//! constraint_axis=(True, False, False))` moves by (1, 2, 3)); with a LOCAL or GIMBAL
//! orientation and several elements the constraint is per element (each object moves, scales
//! and rotates along its own axes). A scalar `value` (rotate) uses `constraint_axis` as given;
//! rotating with all three axes constrained has no axis (Blender then scales positions about
//! the pivot by `cos(angle)` and leaves rotations alone, verified).

use glam::{Mat3, Vec3};

use crate::core::orient::{OrientType, Orientation};

/// Where a constraint's axes come from (`setUserConstraint`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConstraintSpace {
    /// `setConstraint`: the orientation matrix, shared by every element.
    Orientation,
    /// `setLocalConstraint` with several elements: each element's own axes
    /// (`applyObjectConstraint*`).
    PerElement,
}

/// An active axis constraint (`t->con` with `CON_APPLY`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Constraint {
    /// `CON_AXIS0..2`.
    pub axes: [bool; 3],
    /// Shared or per-element axes.
    pub space: ConstraintSpace,
}

impl Constraint {
    /// The constraint of an operator call: all axes when an array `value` is set
    /// (`t_values_set_is_array`), else `constraint_axis` when set with any axis, else none.
    /// `elements` is `t->data_len_all` (the per-element space needs more than one).
    pub fn from_props(
        value_is_set_array: bool,
        constraint_axis: Option<[bool; 3]>,
        orient: &Orientation,
        elements: usize,
    ) -> Option<Constraint> {
        let axes = if value_is_set_array {
            [true; 3]
        } else {
            constraint_axis.filter(|a| a.iter().any(|b| *b))?
        };
        let space = match orient.kind {
            OrientType::Local | OrientType::Gimbal if elements > 1 => ConstraintSpace::PerElement,
            _ => ConstraintSpace::Orientation,
        };
        Some(Constraint { axes, space })
    }

    fn mask(&self) -> Mat3 {
        let f = |b: bool| if b { 1.0 } else { 0.0 };
        Mat3::from_diagonal(Vec3::new(f(self.axes[0]), f(self.axes[1]), f(self.axes[2])))
    }

    /// `projection_matrix_calc`: projects onto the constrained axes of `orient`.
    pub fn projection(&self, orient: &Orientation) -> Mat3 {
        orient.matrix * (self.mask() * orient.inverse())
    }

    /// `applyAxisConstraintVec` / `applyObjectConstraintVec` without an element: the
    /// projection of a global vector.
    pub fn apply_vec(&self, orient: &Orientation, v: Vec3) -> Vec3 {
        self.projection(orient) * v
    }

    /// `applyObjectConstraintVec` for one element whose axes are `elem_axes`; the shared
    /// space returns `v` unchanged.
    pub fn apply_vec_elem(&self, orient: &Orientation, elem_axes: &Mat3, v: Vec3) -> Vec3 {
        match self.space {
            ConstraintSpace::Orientation => v,
            ConstraintSpace::PerElement => *elem_axes * (orient.inverse() * v),
        }
    }

    /// `applyAxisConstraintSize` (no element): unconstrained axes of the size matrix become 1,
    /// then the matrix is expressed in the orientation. The per-element space does nothing
    /// here (see [`Constraint::apply_size_elem`]).
    pub fn apply_size(&self, orient: &Orientation, smat: Mat3) -> Mat3 {
        match self.space {
            ConstraintSpace::Orientation => {
                orient.matrix * (self.reset_unconstrained(smat) * orient.inverse())
            }
            ConstraintSpace::PerElement => smat,
        }
    }

    /// `applyObjectConstraintSize` for one element: the size matrix in the element's axes.
    pub fn apply_size_elem(&self, elem_axes: &Mat3, smat: Mat3) -> Mat3 {
        match self.space {
            ConstraintSpace::Orientation => smat,
            ConstraintSpace::PerElement => {
                let imat = crate::core::math::invert_m3_or_identity(elem_axes);
                *elem_axes * (self.reset_unconstrained(smat) * imat)
            }
        }
    }

    fn reset_unconstrained(&self, mut smat: Mat3) -> Mat3 {
        for i in 0..3 {
            if !self.axes[i] {
                smat.col_mut(i)[i] = 1.0;
            }
        }
        smat
    }

    /// The rotation axis (`applyAxisConstraintRot` / `applyObjectConstraintRot`): the
    /// constrained axis of `axes`, or the one not constrained of a two-axis plane; three (or
    /// zero) axes give no axis (`Vec3::ZERO`).
    pub fn rot_axis(&self, axes: &Mat3) -> Vec3 {
        match self.axes {
            [true, false, false] | [false, true, true] => axes.x_axis,
            [false, true, false] | [true, false, true] => axes.y_axis,
            [false, false, true] | [true, true, false] => axes.z_axis,
            _ => Vec3::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_value_constrains_every_axis() {
        let c = Constraint::from_props(true, Some([true, false, false]), &Orientation::GLOBAL, 1)
            .expect("constraint");
        assert_eq!(c.axes, [true; 3]);
        assert!(
            c.apply_vec(&Orientation::GLOBAL, Vec3::new(1.0, 2.0, 3.0))
                .abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-6)
        );
        assert!(Constraint::from_props(false, None, &Orientation::GLOBAL, 1).is_none());
        assert!(Constraint::from_props(false, Some([false; 3]), &Orientation::GLOBAL, 1).is_none());
    }

    #[test]
    fn local_is_per_element_only_with_several_elements() {
        let o = Orientation {
            kind: OrientType::Local,
            matrix: Mat3::from_rotation_z(0.5),
        };
        let c = Constraint::from_props(true, None, &o, 2).expect("constraint");
        assert_eq!(c.space, ConstraintSpace::PerElement);
        let c1 = Constraint::from_props(true, None, &o, 1).expect("constraint");
        assert_eq!(c1.space, ConstraintSpace::Orientation);
        let n = Orientation {
            kind: OrientType::Normal,
            ..o
        };
        assert_eq!(
            Constraint::from_props(true, None, &n, 2).map(|c| c.space),
            Some(ConstraintSpace::Orientation)
        );
        // Per element: the value in the active axes is re-expressed in the element's axes.
        let elem = Mat3::from_rotation_z(1.0);
        let v = c.apply_vec_elem(&o, &elem, o.matrix * Vec3::X);
        assert!(v.abs_diff_eq(elem * Vec3::X, 1e-6));
    }

    #[test]
    fn rotation_axis_table() {
        let c = |a| Constraint {
            axes: a,
            space: ConstraintSpace::Orientation,
        };
        let m = Mat3::from_rotation_z(0.3);
        assert_eq!(c([true, false, false]).rot_axis(&m), m.x_axis);
        assert_eq!(c([true, true, false]).rot_axis(&m), m.z_axis);
        assert_eq!(c([true, false, true]).rot_axis(&m), m.y_axis);
        assert_eq!(c([true, true, true]).rot_axis(&m), Vec3::ZERO);
    }

    #[test]
    fn size_resets_unconstrained_axes() {
        let c = Constraint {
            axes: [true, false, false],
            space: ConstraintSpace::Orientation,
        };
        let s = c.apply_size(
            &Orientation::GLOBAL,
            Mat3::from_diagonal(Vec3::new(2.0, 3.0, 4.0)),
        );
        assert!(s.abs_diff_eq(Mat3::from_diagonal(Vec3::new(2.0, 1.0, 1.0)), 1e-6));
    }
}
