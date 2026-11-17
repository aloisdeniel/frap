// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator definitions and the registration slice (ARCHITECTURE §10.1, CONTRACT frozen at
//! M1 W0). `Report`/`ReportType` live in frap-rna (RFC 0001 F-LAYER-1) and are
//! re-exported here.

use frap_rna::{FrapError, OperatorSpec, PyExc, RnaError};
use frap_scene::Context;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::ctx::OpCtx;
use crate::errors::{
    GPU_UNAVAILABLE, IO_ERROR, IO_UNSUPPORTED_FORMAT, LIMIT_EXCEEDED, OP_EXEC_ERROR,
    OP_NOT_IMPLEMENTED,
};
pub use frap_rna::{OpArgs, Report, ReportType};

/// The invocation pipeline (§10.2): argument conversion with Blender's messages, frap hints,
/// macro execution and the operator metadata queries (`poll()`, `get_rna_type()`, …). Declared
/// here (WP 1.12) so that it is re-exported as `frap_op::pipeline` by `lib.rs`'s `pub use def::*`.
pub mod pipeline;

/// Fidelity tier (§2.1).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// Surface.
    T0,
    /// Index-exact.
    T1,
    /// Canonical.
    T2,
    /// Invariant.
    T3,
    /// Perceptual.
    T4,
}

/// Normal operator or macro (`bpy.types.Macro`, steps in `OperatorSpec::macro_steps`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpKind {
    /// Normal.
    Normal,
    /// Macro: runs its steps with their nested args.
    Macro,
}

/// How completely frap implements the operator (`--describe` `frap.status`/`headless`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Support {
    /// Everything Blender does headless.
    Full,
    /// Some props or paths missing (described).
    Partial(&'static str),
    /// Interactive in Blender, made headless by frap (described).
    HeadlessExtended(&'static str),
}

/// Declarative poll requirement (data, published by `--describe` as `requires`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PollReq {
    /// `context.mode` is OBJECT.
    ModeObject,
    /// `context.mode` is EDIT_MESH.
    ModeEditMesh,
    /// `context.mode` is EDIT_ARMATURE.
    ModeEditArmature,
    /// `context.mode` is POSE.
    ModePose,
    /// An active object exists.
    ActiveObject,
    /// The active object has this type ("MESH", "ARMATURE", …).
    ActiveType(&'static str),
    /// The active object is editable (not linked).
    ActiveEditable,
    /// At least one selected object.
    HasSelectedObjects,
    /// `scene.camera` is set.
    SceneHasCamera,
    /// The scene is editable.
    SceneEditable,
}

impl PollReq {
    /// Stable code published in `error.detail.requires`/`unmet` and `--describe`
    /// (`MODE_EDIT_MESH`, `ACTIVE_TYPE_MESH`, …; §13.5).
    pub fn code(&self) -> String {
        match self {
            PollReq::ModeObject => "MODE_OBJECT".into(),
            PollReq::ModeEditMesh => "MODE_EDIT_MESH".into(),
            PollReq::ModeEditArmature => "MODE_EDIT_ARMATURE".into(),
            PollReq::ModePose => "MODE_POSE".into(),
            PollReq::ActiveObject => "ACTIVE_OBJECT".into(),
            PollReq::ActiveType(t) => format!("ACTIVE_TYPE_{t}"),
            PollReq::ActiveEditable => "ACTIVE_EDITABLE".into(),
            PollReq::HasSelectedObjects => "HAS_SELECTED_OBJECTS".into(),
            PollReq::SceneHasCamera => "SCENE_HAS_CAMERA".into(),
            PollReq::SceneEditable => "SCENE_EDITABLE".into(),
        }
    }
    /// Human-readable condition, in Python terms (`--describe` poll text).
    pub fn text(&self) -> String {
        match self {
            PollReq::ModeObject => "bpy.context.mode == 'OBJECT'".into(),
            PollReq::ModeEditMesh => "bpy.context.mode == 'EDIT_MESH'".into(),
            PollReq::ModeEditArmature => "bpy.context.mode == 'EDIT_ARMATURE'".into(),
            PollReq::ModePose => "bpy.context.mode == 'POSE'".into(),
            PollReq::ActiveObject => {
                "bpy.context.active_object is not None and not hidden (hide_viewport)".into()
            }
            PollReq::ActiveType(t) => format!("bpy.context.active_object.type == '{t}'"),
            PollReq::ActiveEditable => {
                "the active object and its data are local (not linked from a library)".into()
            }
            PollReq::HasSelectedObjects => "bpy.context.selected_objects is not empty".into(),
            PollReq::SceneHasCamera => "bpy.context.scene.camera is not None".into(),
            PollReq::SceneEditable => "bpy.context.scene is local (not linked)".into(),
        }
    }
}

impl std::fmt::Display for PollReq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code())
    }
}

/// Why a poll failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollFail {
    /// Custom message; `None` ⇒ `Operator bpy.ops.X.poll() failed, context is incorrect`.
    pub message: Option<String>,
    /// The failing requirements.
    pub unmet: SmallVec<[PollReq; 2]>,
    /// The exact fix (`bpy.ops.object.mode_set(mode='EDIT')`).
    pub hint: Option<String>,
}

/// Poll: declared requirements plus the check.
#[derive(Copy, Clone)]
pub struct Poll {
    /// Requirements (data).
    pub requires: &'static [PollReq],
    /// The check.
    pub check: fn(&Context) -> Result<(), PollFail>,
}

/// `exec` status.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpStatus {
    /// Commit.
    Finished,
    /// Roll back; `status: "CANCELLED"` with Blender's WARNING reports.
    Cancelled,
}

/// `exec` error (rolls back; `status: "ERROR"`).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum OpError {
    /// Raised as `RuntimeError("Error: <msg>")`.
    Runtime(String),
    /// RNA error.
    Rna(RnaError),
    /// A path of the operator frap does not implement yet (`NotImplementedError`).
    NotImplemented(String),
    /// File I/O.
    Io(String),
    /// A file format frap does not read or write (`IO_UNSUPPORTED_FORMAT`, e.g. a `.blend`
    /// path given to `wm.save_as_mainfile`; WP 1.23).
    UnsupportedFormat(String),
    /// GPU unavailable or failed (`GPU_UNAVAILABLE`).
    Gpu(String),
    /// `--max-elements` / `--stmt-timeout` (`LIMIT_EXCEEDED`).
    Limit(String),
}

impl From<RnaError> for OpError {
    fn from(e: RnaError) -> Self {
        OpError::Rna(e)
    }
}

impl OpError {
    /// Converts to the protocol error.
    pub fn into_frap(self) -> FrapError {
        match self {
            OpError::Runtime(m) => {
                FrapError::new(PyExc::RuntimeError, OP_EXEC_ERROR, format!("Error: {m}"))
            }
            OpError::Rna(e) => e.into_frap("", |_| OP_EXEC_ERROR),
            OpError::NotImplemented(m) => {
                FrapError::new(PyExc::NotImplementedError, OP_NOT_IMPLEMENTED, m)
            }
            OpError::Io(m) => FrapError::new(PyExc::RuntimeError, IO_ERROR, format!("Error: {m}")),
            OpError::UnsupportedFormat(m) => FrapError::new(
                PyExc::RuntimeError,
                IO_UNSUPPORTED_FORMAT,
                format!("Error: {m}"),
            ),
            OpError::Gpu(m) => {
                FrapError::new(PyExc::RuntimeError, GPU_UNAVAILABLE, format!("Error: {m}"))
            }
            OpError::Limit(m) => {
                FrapError::new(PyExc::RuntimeError, LIMIT_EXCEEDED, format!("Error: {m}"))
            }
        }
    }
}

/// Exec result.
pub type OpResult = Result<OpStatus, OpError>;

/// One enum item computed at runtime (dynamic enums: `mode_set.mode`, `modifier_add.type`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumItemOwned {
    /// Identifier.
    pub identifier: SmolStr,
    /// UI name.
    pub name: SmolStr,
    /// Description.
    pub description: SmolStr,
}

/// Items of a dynamic enum in the current context.
pub type DynEnumFn = fn(&Context) -> Vec<EnumItemOwned>;

/// A registered operator.
pub struct OperatorDef {
    /// Dumped spec (generated static or catalog entry).
    pub spec: &'static OperatorSpec,
    /// Declarative requirements + check (§8).
    pub poll: Poll,
    /// Exec; `#[operator]` wraps the typed fn with `P::from_args`.
    pub exec: fn(&mut OpCtx, &OpArgs) -> OpResult,
    /// Normal | Macro.
    pub kind: OpKind,
    /// Dynamic enum props and their item functions.
    pub dyn_enums: &'static [(&'static str, DynEnumFn)],
    /// Fidelity tier (§2.1).
    pub tier: Tier,
    /// Support level.
    pub support: Support,
    /// `frap_note`s for `--describe`.
    pub notes: &'static [&'static str],
}

impl std::fmt::Debug for OperatorDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperatorDef")
            .field("idname", &self.spec.idname)
            .field("kind", &self.kind)
            .field("tier", &self.tier)
            .finish_non_exhaustive()
    }
}

/// Every operator implementation registers here, one file each (§2.2 rule 1).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static OPERATORS: [OperatorDef];
