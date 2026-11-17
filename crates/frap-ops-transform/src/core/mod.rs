// SPDX-License-Identifier: GPL-3.0-or-later
//! Orientation (GLOBAL/LOCAL/NORMAL/GIMBAL/VIEW/CURSOR/PARENT), pivot and proportional-editing
//! core shared by every mode (WP 1.21). Mode handlers (`object/` M1, `edit/` M2, `pose/` M6)
//! build their element data and call into these modules; nothing here knows about a mode.
//!
//! Provenance: ports of Blender 5.2.4 (GPL-2.0-or-later) `editors/transform/transform.cc`,
//! `transform_generics.cc` (`initTransInfo`, `calculateCenter*`), `transform_orientations.cc`
//! (`ED_transform_calc_orientation_from_type_ex`, `gimbal_axis_object`),
//! `transform_constraints.cc` (`setUserConstraint`, `applyAxisConstraint*`,
//! `applyObjectConstraint*`), `transform_mode_rotate.cc`, `transform_mode_resize.cc`,
//! `transform_mode_translate.cc` and `transform_data.cc` (`calculatePropRatio`), and
//! `blenlib/intern/math_matrix_c.cc` / `math_rotation_c.cc` for the helpers in [`math`](crate::core::math).

pub mod constraint;
pub mod math;
pub mod orient;
pub mod params;
pub mod pivot;
pub mod proportional;

pub use constraint::{Constraint, ConstraintSpace};
pub use orient::{OrientType, Orientation};
pub use params::TransformParams;
pub use pivot::PivotPoint;
pub use proportional::Falloff;

use frap_op::prelude::*;
use frap_op::{TRANSFORM_MODE_HANDLERS, TransformOp};

/// The transform dispatcher (owned once, §10.3): runs the `TRANSFORM_MODE_HANDLERS` entry of
/// the context mode. A mode without a handler answers `NotImplementedError`
/// (`OP_NOT_IMPLEMENTED`) naming it, before anything changes.
pub fn dispatch(cx: &mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult {
    let mode = cx.ctx().mode();
    let Some(h) = TRANSFORM_MODE_HANDLERS.iter().find(|h| h.mode == mode) else {
        return Err(OpError::NotImplemented(format!(
            "frap: {} is not implemented in {} mode yet (no transform mode handler)",
            cx.op().idname,
            mode.as_str()
        )));
    };
    (h.apply)(cx, op, args)
}
