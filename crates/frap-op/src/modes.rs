// SPDX-License-Identifier: GPL-3.0-or-later
//! Extension points for mode switching and transforms (ARCHITECTURE §8, §10.3; CONTRACT frozen
//! at M1 W0). `object.mode_set` and `transform.translate/rotate/resize` are owned once and
//! dispatch to these slices; later milestones add handler files, never edit the dispatchers.

use frap_rna::{IdRef, ObjectId, OpArgs};
use frap_scene::{ContextMode, ObjectMode};

use crate::ctx::OpCtx;
use crate::def::OpResult;

/// Enters or leaves one mode for one object type (`frap-ops-object/src/modes/<mode>.rs`:
/// `object.rs` M1, `edit_mesh.rs` M2, `edit_armature.rs` and `pose.rs` M6).
#[derive(Debug)]
pub struct ModeHandler {
    /// The `Object.mode` this handler switches to/from.
    pub mode: ObjectMode,
    /// Object type it applies to ("MESH", "ARMATURE"; "" = any).
    pub object_type: &'static str,
    /// Enter the mode for these objects (multi-object edit: every selected object of the
    /// active object's type, deduplicated by data).
    pub enter: fn(&mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult,
    /// Leave the mode for these objects (e.g. write edit BMeshes back with `to_mesh`).
    pub exit: fn(&mut OpCtx, objects: &[IdRef<ObjectId>]) -> OpResult,
}

/// Every mode handler. A missing handler gives `OP_NOT_IMPLEMENTED` naming the mode.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static MODE_HANDLERS: [ModeHandler];

/// Which transform operator is running.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TransformOp {
    /// `transform.translate`
    Translate,
    /// `transform.rotate`
    Rotate,
    /// `transform.resize`
    Resize,
}

/// Applies a transform operator in one context mode (`frap-ops-transform/src/<mode>/`:
/// `object/` M1, `edit/` M2, `pose/` M6). The handler reads the operator's converted args
/// through the shared orientation/pivot/proportional core of frap-ops-transform.
#[derive(Debug)]
pub struct TransformModeHandler {
    /// The context mode handled.
    pub mode: ContextMode,
    /// Apply `op` with the converted `args` of `transform.<op>`.
    pub apply: fn(&mut OpCtx, op: TransformOp, args: &OpArgs) -> OpResult,
}

/// Every transform mode handler.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static TRANSFORM_MODE_HANDLERS: [TransformModeHandler];
