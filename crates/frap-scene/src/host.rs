// SPDX-License-Identifier: GPL-3.0-or-later
//! `HostRead` / `Host`, `Evaluator`, RNA hooks and RNA functions (ARCHITECTURE §7.3, §9.5;
//! CONTRACT frozen at M1 W0, amended by RFC 0001 F-OP-5..7, F-HOOK-1, F-SES-1).
//!
//! `Host` is implemented by `frap_session::HostImpl`, a field of `Session` disjoint from its
//! `Services` (F-SES-1), so an `OpCtx` can borrow both.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{
    BitSet, Field, FuncDef, IdRef, MeshId, ObjectId, PropDef, Report, RnaError, RnaPtr, Root,
    StructDef, Value,
};
use indexmap::IndexMap;

use crate::context::{Context, EditState, Preferences, WindowState};
use crate::data::{ChangeLog, Main};
use crate::eval::EvaluatedObject;

/// Read access to the session state (no transaction needed, §12).
pub trait HostRead {
    /// `bpy.data`
    fn main(&self) -> &Main;
    /// Window state.
    fn win(&self) -> &WindowState;
    /// Edit-mode state.
    fn edit(&self) -> &EditState;
    /// Context view.
    fn ctx(&self) -> Context<'_>;
    /// Read-only, cached view of a mesh in Edit Mode (§6.3); the original Mesh otherwise.
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh>;
}

/// Write access to the session state (inside a transaction).
pub trait Host: HostRead {
    /// `bpy.data` plus the statement's change log.
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog);
    /// Edit-mode state.
    fn edit_mut(&mut self) -> &mut EditState;
    /// Context and edit state at once (disjoint fields of the state; F-OP-5).
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState);
    /// Window state.
    fn win_mut(&mut self) -> &mut WindowState;
    /// Preferences (`bpy.context.preferences`): `wm.open_mainfile` restores the glTF add-on
    /// preferences saved in a `.frap` and `wm.read_factory_settings` resets them (§15, WP 1.23).
    /// `None` (the default) for hosts without preferences of their own (test hosts).
    fn prefs_mut(&mut self) -> Option<&mut Preferences> {
        None
    }
    /// Split borrow: the state (read) and the evaluator.
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator);
    /// COW clone of the state with pending animation applied at `frame` (scene frame if
    /// `None`) and a fresh evaluator, for render and export (§7.3, §19.4; F-OP-7).
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError>;
    /// `frame_current` setter: marks time dirty only (§7.3).
    fn mark_time_dirty(&mut self);
    /// Applies pending animation at an evaluation boundary (§7.3).
    fn apply_pending_animation(&mut self) -> Result<(), RnaError>;
    /// Adds a report to the current statement's response.
    fn report(&mut self, r: Report);
    /// Animation write through an ID-relative path; `owner` is `Root::Id` or `Root::Embedded`.
    fn set_path(
        &mut self,
        owner: Root,
        data_path: &str,
        index: i32,
        v: Value,
    ) -> Result<(), RnaError>;
}

/// An owned, evaluated view of a state clone (dropped after render/export; F-OP-7).
pub struct Snapshot {
    /// The cloned state.
    pub host: Box<dyn HostRead + Send>,
    /// A fresh evaluator for it (its cache must not mix clone and original generations).
    pub eval: Box<dyn Evaluator + Send>,
}

/// Evaluation mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EvalMode {
    /// Viewport visibility (`show_viewport`).
    Viewport,
    /// Render visibility (`show_render`).
    Render,
}

/// Evaluation failure (F-EVAL-1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvalError {
    /// Message (Blender wording where Blender has one).
    pub message: String,
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for EvalError {}

/// Depsgraph-lite (§7.3), implemented by `frap_eval::EvalCache`.
pub trait Evaluator {
    /// Evaluated object (cached by generation fingerprint).
    fn object(
        &mut self,
        host: &dyn HostRead,
        ob: IdRef<ObjectId>,
        mode: EvalMode,
    ) -> Result<Arc<EvaluatedObject>, EvalError>;
    /// `view_layer.update()`, `evaluated_depsgraph_get()`.
    fn depsgraph_update(&mut self, host: &dyn HostRead);
}

/// Arguments of an RNA function call, bound against `FuncDef.params` (positional and keyword,
/// keyword-only markers enforced, each coerced; §9.5).
#[derive(Clone, Debug)]
pub struct FuncArgs {
    /// The function.
    pub func: &'static FuncDef,
    /// Values in parameter order, defaults filled in.
    pub values: IndexMap<&'static str, Value>,
    /// Indices of the parameters the caller passed.
    pub is_set: BitSet,
}

impl FuncArgs {
    /// Value of a parameter.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }
}

/// An RNA function implementation (`select_set`, `materials.new`, `keyframe_insert`, …).
pub type RnaFuncImpl =
    fn(&mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError>;
/// A setter with side effects (`rotation_mode`, `parent`, `name`, `frame_current`, …).
pub type RnaSetHook =
    fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError>;
/// A getter that needs more than its own struct (`material_slots`, `matrix_world`, `users`):
/// reads take `&dyn HostRead` and may return a proxy borrowing the scene (F-HOOK-1).
pub type RnaGetHook = for<'h> fn(
    &'h dyn HostRead,
    this: &RnaPtr,
    prop: &'static PropDef,
) -> Result<Field<'h>, RnaError>;

/// Registration of one RNA function (one file per function, `funcs/<struct>_<func>.rs`).
#[derive(Debug)]
pub struct RnaFuncReg {
    /// Struct identifier ("Object", "BlendDataMaterials").
    pub srna: &'static str,
    /// Function identifier ("select_set").
    pub func: &'static str,
    /// Implementation.
    pub imp: RnaFuncImpl,
}

/// Registration of a get and/or set hook for one `(struct, property)`.
#[derive(Debug)]
pub struct RnaHookReg {
    /// Struct identifier; lookup walks the base chain (a hook on `ID.name` serves every ID).
    pub srna: &'static str,
    /// Property identifier.
    pub prop: &'static str,
    /// Get hook.
    pub get: Option<RnaGetHook>,
    /// Set hook.
    pub set: Option<RnaSetHook>,
}

/// Every RNA function implementation.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static RNA_FUNCS: [RnaFuncReg];

/// Every RNA hook.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static RNA_HOOKS: [RnaHookReg];

/// Finds the hook of `(srna, prop)`, walking the base chain.
pub fn find_hook(srna: &'static StructDef, prop: &str) -> Option<&'static RnaHookReg> {
    let mut s = Some(srna);
    while let Some(d) = s {
        if let Some(h) = RNA_HOOKS
            .iter()
            .find(|h| h.srna == d.identifier && h.prop == prop)
        {
            return Some(h);
        }
        s = d.base;
    }
    None
}

/// Finds the implementation of `(srna, func)`, walking the base chain.
pub fn find_func(srna: &'static StructDef, func: &str) -> Option<&'static RnaFuncReg> {
    let mut s = Some(srna);
    while let Some(d) = s {
        if let Some(f) = RNA_FUNCS
            .iter()
            .find(|f| f.srna == d.identifier && f.func == func)
        {
            return Some(f);
        }
        s = d.base;
    }
    None
}
