// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator definitions and the registration slice (ARCHITECTURE §10.1, CONTRACT frozen at
//! M1 W0). `Report`/`ReportType` live in frap-rna (RFC 0001 F-LAYER-1) and are
//! re-exported here.

use frap_rna::{FrapError, OperatorSpec, PyExc, RnaError};
use frap_scene::Context;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::ctx::OpCtx;
use crate::errors::{GPU_UNAVAILABLE, IO_ERROR, LIMIT_EXCEEDED, OP_EXEC_ERROR, OP_NOT_IMPLEMENTED};
pub use frap_rna::{OpArgs, Report, ReportType};

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

/// Every operator implementation registers here, one file each (§2.2 rule 1).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static OPERATORS: [OperatorDef];
