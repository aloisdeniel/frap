// SPDX-License-Identifier: GPL-3.0-or-later
//! The mode-independent arguments of `transform.translate/rotate/resize` (what `initTransInfo`
//! reads from the operator), decoded from the converted `OpArgs` through the generated props
//! structs of `frap-spec-ops-transform`. Mode handlers receive the raw `OpArgs`
//! (`TransformModeHandler::apply`) and call [`TransformParams::from_args`].

use frap_op::TransformOp;
use frap_rna::{OpArgs, OpProps, Prop};
use frap_spec_ops_transform::transform::{Resize, Rotate, Translate};
use glam::{Mat3, Vec3};

use crate::core::proportional::Falloff;

/// Decoded transform arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct TransformParams {
    /// Which operator.
    pub op: TransformOp,
    /// `value` (translate/resize: the vector; rotate: the angle in `value.x`), when set
    /// (`T_INPUT_IS_VALUES_FINAL`). Unset means the identity transform.
    pub value: Option<Vec3>,
    /// `orient_axis` of rotate (0 = X … 2 = Z); 2 for the others.
    pub orient_axis: usize,
    /// `orient_type`, when set.
    pub orient_type: Option<String>,
    /// `orient_matrix` (columns are axes), when set.
    pub orient_matrix: Option<Mat3>,
    /// `orient_matrix_type` (its value; default `GLOBAL`).
    pub orient_matrix_type: String,
    /// `constraint_axis`, when set.
    pub constraint_axis: Option<[bool; 3]>,
    /// `center_override`, when set (rotate, resize).
    pub center_override: Option<Vec3>,
    /// `use_proportional_edit` with its falloff and size (only when set to True: the scene's
    /// settings are used by modal transforms only).
    pub proportional: Option<(Falloff, f32)>,
    /// `cursor_transform` (translate).
    pub cursor_transform: bool,
    /// `texture_space` (translate, resize).
    pub texture_space: bool,
    /// `translate_origin` (translate).
    pub translate_origin: bool,
    /// `mirror` (edit-mode mirror editing; ignored in object mode).
    pub mirror: bool,
}

fn set<T: Clone>(p: &Prop<T>) -> Option<T> {
    p.is_set.then(|| p.value.clone())
}

fn matrix(p: &Prop<[[f32; 3]; 3]>) -> Option<Mat3> {
    // RNA matrix values arrive row by row: rows → columns.
    set(p).map(|a| Mat3::from_cols_array_2d(&a).transpose())
}

fn proportional(on: &Prop<bool>, falloff: &str, size: f32) -> Option<(Falloff, f32)> {
    (on.is_set && on.value).then(|| (Falloff::from_identifier(falloff), size))
}

impl TransformParams {
    /// Decodes the arguments of `transform.<op>`.
    pub fn from_args(op: TransformOp, args: &OpArgs) -> TransformParams {
        match op {
            TransformOp::Rotate => {
                let p = Rotate::from_args(args);
                TransformParams {
                    op,
                    value: set(&p.value).map(|a| Vec3::new(a, 0.0, 0.0)),
                    orient_axis: match p.orient_axis.value.identifier() {
                        "X" => 0,
                        "Y" => 1,
                        _ => 2,
                    },
                    orient_type: set(&p.orient_type).map(|s| s.to_string()),
                    orient_matrix: matrix(&p.orient_matrix),
                    orient_matrix_type: p.orient_matrix_type.value.to_string(),
                    constraint_axis: set(&p.constraint_axis),
                    center_override: set(&p.center_override).map(Vec3::from_array),
                    proportional: proportional(
                        &p.use_proportional_edit,
                        p.proportional_edit_falloff.value.identifier(),
                        p.proportional_size.value,
                    ),
                    cursor_transform: false,
                    texture_space: false,
                    translate_origin: false,
                    mirror: p.mirror.value,
                }
            }
            TransformOp::Resize => {
                let p = Resize::from_args(args);
                TransformParams {
                    op,
                    value: set(&p.value).map(Vec3::from_array),
                    orient_axis: 2,
                    orient_type: set(&p.orient_type).map(|s| s.to_string()),
                    orient_matrix: matrix(&p.orient_matrix),
                    orient_matrix_type: p.orient_matrix_type.value.to_string(),
                    constraint_axis: set(&p.constraint_axis),
                    center_override: set(&p.center_override).map(Vec3::from_array),
                    proportional: proportional(
                        &p.use_proportional_edit,
                        p.proportional_edit_falloff.value.identifier(),
                        p.proportional_size.value,
                    ),
                    cursor_transform: false,
                    texture_space: p.texture_space.value,
                    translate_origin: false,
                    mirror: p.mirror.value,
                }
            }
            _ => {
                let p = Translate::from_args(args);
                TransformParams {
                    op: TransformOp::Translate,
                    value: set(&p.value).map(Vec3::from_array),
                    orient_axis: 2,
                    orient_type: set(&p.orient_type).map(|s| s.to_string()),
                    orient_matrix: matrix(&p.orient_matrix),
                    orient_matrix_type: p.orient_matrix_type.value.to_string(),
                    constraint_axis: set(&p.constraint_axis),
                    center_override: None,
                    proportional: proportional(
                        &p.use_proportional_edit,
                        p.proportional_edit_falloff.value.identifier(),
                        p.proportional_size.value,
                    ),
                    cursor_transform: p.cursor_transform.value,
                    texture_space: p.texture_space.value,
                    translate_origin: p.translate_origin.value,
                    mirror: p.mirror.value,
                }
            }
        }
    }

    /// Whether `value` is a set array (translate, resize): Blender then constrains to every
    /// axis (`t_values_set_is_array`).
    pub fn value_is_set_array(&self) -> bool {
        self.value.is_some() && self.op != TransformOp::Rotate
    }

    /// The angle of rotate (0 when unset).
    pub fn angle(&self) -> f32 {
        self.value.map_or(0.0, |v| v.x)
    }
}
