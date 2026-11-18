// SPDX-License-Identifier: GPL-3.0-or-later
//! Edit-mode transforms (`TRANSFORM_MODE_HANDLERS` entry for `EDIT_MESH`; ARCHITECTURE §10.3,
//! §22.4, WP 2.12): `transform.translate/rotate/resize` on the selected elements of every mesh
//! in Edit Mode, in world space, with the shared orientation/pivot/proportional core
//! (`crate::core`; pivots incl. `ACTIVE_ELEMENT` and `INDIVIDUAL_ORIGINS`,
//! `orient_type='NORMAL'`, proportional falloffs incl. connected and projected distances), and
//! the engine the edit-only transform operators share (`shrink_fatten`, `edge_slide`,
//! `vert_slide`, `tosphere`, `edge_crease`, `edge_bevelweight`, `vert_crease`, `push_pull`,
//! `mirror`, `transform.transform`; `vertex_random` has its own loop).
//!
//! The dispatchers (`core::dispatch`, `ops/{translate,rotate,resize}.rs`) stay with WP 1.21:
//! the handler is found through the `TRANSFORM_MODE_HANDLERS` slice.
//!
//! Verified on Blender 5.2.4 (goldens `tests/golden/cases/edit/transform/`): values are world
//! vectors (a translate of a scaled or rotated object moves its vertices by the world vector);
//! `ACTIVE_ELEMENT` uses the active element of the selection history (or the active face);
//! `INDIVIDUAL_ORIGINS` transforms each island (selected edges connected through vertices, or
//! selected faces sharing an edge in face select mode) about its center, and with
//! `orient_type='NORMAL'` constrains along each island's own axes; proportional editing
//! measures world distances to the nearest selected vertex (or geodesic distances with
//! `use_proportional_connected`). Mesh symmetry editing is not reachable (frap does not bind
//! `Mesh.use_mirror_x/y/z`). Auto Merge (`tool_settings.use_mesh_automerge`) welds the selected
//! vertices within `double_threshold` after the transform (`EDBM_automerge`); Auto Merge with
//! split (`use_automerge_and_split=True`, forced by `mesh.extrude_manifold`) is refused
//! (`NotImplementedError` with a hint).

/// Auto Merge with split (`EDBM_automerge_and_split`; M2 leftover, WP 4.6, M4 W0 stub).
pub mod automerge;
pub mod bmq;
pub mod islands;
pub mod mathx;
pub mod orient;
pub mod slide;
pub mod trans;

use frap_op::prelude::*;
use frap_op::{TRANSFORM_MODE_HANDLERS, TransformModeHandler, TransformOp};
use frap_rna::{OpArgs, OpProps, Prop};
use frap_scene::ContextMode;
use frap_spec_ops_transform::transform::{Resize, Rotate, Translate};
use glam::{Mat3, Vec3};

use crate::core::proportional::Falloff;
pub use trans::{Mode, PropEdit, Request};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(TRANSFORM_MODE_HANDLERS)]
static EDIT_MESH: TransformModeHandler = TransformModeHandler {
    mode: ContextMode::EditMesh,
    apply,
};

/// Operators frap implements in Edit Mode only (`tosphere`, `push_pull`, `mirror`, other
/// `transform.transform` modes): `NotImplementedError` (`OP_NOT_IMPLEMENTED`) naming the mode
/// otherwise, before anything changes.
pub fn require_edit_mesh(cx: &OpCtx) -> Result<(), OpError> {
    let mode = cx.ctx().mode();
    if mode == ContextMode::EditMesh {
        return Ok(());
    }
    Err(OpError::NotImplemented(format!(
        "frap: {} is not implemented in {} mode yet (Edit Mode on meshes only)",
        cx.op().idname,
        mode.as_str()
    )))
}

/// `Prop` → `Option` by is-set.
pub fn set<T: Clone>(p: &Prop<T>) -> Option<T> {
    p.is_set.then(|| p.value.clone())
}

/// `orient_matrix` (RNA rows) as columns.
pub fn matrix(p: &Prop<[[f32; 3]; 3]>) -> Option<Mat3> {
    set(p).map(|a| Mat3::from_cols_array_2d(&a).transpose())
}

/// Proportional editing of an operator call (only when `use_proportional_edit` was set True).
pub fn proportional(
    on: &Prop<bool>,
    falloff: &str,
    size: f32,
    connected: bool,
    projected: bool,
) -> Option<PropEdit> {
    (on.is_set && on.value).then(|| PropEdit {
        falloff: Falloff::from_identifier(falloff),
        size,
        connected,
        projected,
    })
}

/// The `orient_axis` index of an identifier.
pub fn axis_index(s: &str) -> usize {
    match s {
        "X" => 0,
        "Y" => 1,
        _ => 2,
    }
}

/// Decodes `transform.translate/rotate/resize` into an edit-mode request.
pub fn request_from_args(op: TransformOp, args: &OpArgs) -> Request {
    match op {
        TransformOp::Rotate => {
            let p = Rotate::from_args(args);
            let mut r = Request::new(Mode::Rotate);
            if let Some(v) = set(&p.value) {
                r.values[0] = v;
            }
            r.orient_axis = axis_index(p.orient_axis.value.identifier());
            r.orient_type = set(&p.orient_type).map(|s| s.to_string());
            r.orient_matrix = matrix(&p.orient_matrix);
            r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
            r.constraint_axis = set(&p.constraint_axis);
            r.has_constraint_prop = true;
            r.center_override = set(&p.center_override).map(Vec3::from_array);
            r.proportional = proportional(
                &p.use_proportional_edit,
                p.proportional_edit_falloff.value.identifier(),
                p.proportional_size.value,
                p.use_proportional_connected.value,
                p.use_proportional_projected.value,
            );
            r
        }
        TransformOp::Resize => {
            let p = Resize::from_args(args);
            let mut r = Request::new(Mode::Resize);
            if let Some(v) = set(&p.value) {
                r.values[..3].copy_from_slice(&v);
                r.values_is_array = true;
            }
            r.orient_type = set(&p.orient_type).map(|s| s.to_string());
            r.orient_matrix = matrix(&p.orient_matrix);
            r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
            r.constraint_axis = set(&p.constraint_axis);
            r.has_constraint_prop = true;
            r.center_override = set(&p.center_override).map(Vec3::from_array);
            r.proportional = proportional(
                &p.use_proportional_edit,
                p.proportional_edit_falloff.value.identifier(),
                p.proportional_size.value,
                p.use_proportional_connected.value,
                p.use_proportional_projected.value,
            );
            r
        }
        _ => {
            let p = Translate::from_args(args);
            let mut r = Request::new(Mode::Translate);
            if let Some(v) = set(&p.value) {
                r.values[..3].copy_from_slice(&v);
                r.values_is_array = true;
            }
            r.orient_type = set(&p.orient_type).map(|s| s.to_string());
            r.orient_matrix = matrix(&p.orient_matrix);
            r.orient_matrix_type = set(&p.orient_matrix_type).map(|s| s.to_string());
            r.constraint_axis = set(&p.constraint_axis);
            r.has_constraint_prop = true;
            r.proportional = proportional(
                &p.use_proportional_edit,
                p.proportional_edit_falloff.value.identifier(),
                p.proportional_size.value,
                p.use_proportional_connected.value,
                p.use_proportional_projected.value,
            );
            r.use_automerge_and_split = set(&p.use_automerge_and_split);
            r
        }
    }
}

/// `use_automerge_and_split=True` (`T_AUTOSPLIT`, `EDBM_automerge_and_split`: welds and splits
/// edges and faces at the crossings) is not implemented: the error says how to get there.
pub fn automerge_and_split_not_implemented() -> OpError {
    OpError::Frap(Box::new(
        frap_rna::FrapError::new(
            frap_rna::PyExc::NotImplementedError,
            frap_op::errors::OP_NOT_IMPLEMENTED,
            "frap: edit-mode transforms with use_automerge_and_split=True (Auto Merge with \
             split, EDBM_automerge_and_split) are not implemented yet",
        )
        .with_hint(
            "transform without use_automerge_and_split (mesh.extrude_manifold: use \
             mesh.extrude_region_move(MESH_OT_extrude_region={\"use_dissolve_ortho_edges\": True}, \
             TRANSFORM_OT_translate={\"value\": ...})), then weld with \
             mesh.remove_doubles(threshold=0.001); or set \
             bpy.context.scene.tool_settings.use_mesh_automerge = True, which merges without \
             splitting",
        ),
    ))
}

/// Refuses the options that change the result and are not implemented in Edit Mode.
fn unsupported(op: TransformOp, args: &OpArgs) -> Result<(), OpError> {
    let what = match op {
        TransformOp::Translate => {
            let p = Translate::from_args(args);
            if p.texture_space.value {
                Some("texture_space=True")
            } else if p.use_automerge_and_split.is_set
                && p.use_automerge_and_split.value
                && !automerge::IMPLEMENTED
            {
                return Err(automerge_and_split_not_implemented());
            } else {
                None
            }
        }
        TransformOp::Resize => {
            let p = Resize::from_args(args);
            p.texture_space.value.then_some("texture_space=True")
        }
        _ => None,
    };
    match what {
        Some(w) => Err(OpError::NotImplemented(format!(
            "frap: edit-mode transforms with {w} are not implemented yet"
        ))),
        None => Ok(()),
    }
}

/// The edit-mesh handler.
fn apply(cx: &mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult {
    unsupported(op, args)?;
    if op == TransformOp::Translate && Translate::from_args(args).cursor_transform.value {
        return trans::translate_cursor(cx, &request_from_args(op, args));
    }
    trans::run(cx, request_from_args(op, args))
}
