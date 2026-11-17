// SPDX-License-Identifier: GPL-3.0-or-later
//! §9.5 `HostRead` / `Host` (`frap-scene/src/host.rs`, CONTRACT), §7.3 `Evaluator`, hooks.

use std::sync::Arc;

use frap_probe_mesh::Mesh;
use frap_probe_rna::*;

use crate::{ChangeLog, Context, EditState, Main, WindowState};

pub trait HostRead {
    fn main(&self) -> &Main;
    fn win(&self) -> &WindowState;
    fn edit(&self) -> &EditState;
    fn ctx(&self) -> Context<'_>;
    /// Read-only, cached view of a mesh in Edit Mode (§6.3); the original Mesh otherwise.
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh>;
}

pub trait Host: HostRead {
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog);
    fn edit_mut(&mut self) -> &mut EditState;
    fn win_mut(&mut self) -> &mut WindowState;
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator);
    fn mark_time_dirty(&mut self);
    fn apply_pending_animation(&mut self) -> Result<(), RnaError>;
    fn report(&mut self, r: Report);
    fn set_path(&mut self, owner: Root, data_path: &str, index: i32, v: Value) -> Result<(), RnaError>;

    // ---- RFC 0001 additions
    /// F-OP-5: `OpCtx::split_edit` cannot be written on top of the trait above: `ctx()` borrows
    /// `&self` and `edit_mut()` borrows `&mut self` (see the compile_fail doctest in frap-probe-op).
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState);
    /// F-OP-7: render/export work on a COW clone of State with animation applied at `frame`
    /// (§7.3, §19.4) but no contract gives an operator access to such a clone.
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError>;
}

/// F-OP-7: an owned, evaluated view of a State clone (dropped after render/export).
pub struct Snapshot {
    pub host: Box<dyn HostRead + Send>,
    pub eval: Box<dyn Evaluator + Send>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum EvalMode {
    Viewport,
    Render,
}

/// §7.3 (`frap-scene`, CONTRACT). `Mat4` is glam's; arrays here.
#[derive(Clone, Debug)]
pub struct EvaluatedObject {
    pub matrix_world: [[f32; 4]; 4],
    pub mesh: Option<Arc<Mesh>>,
    pub bounds: Option<[[f32; 3]; 2]>,
    pub warnings: Vec<Report>,
}

/// Not defined by the contract (RFC 0001 F-EVAL-1).
#[derive(Clone, Debug, PartialEq)]
pub struct EvalError(pub String);

pub trait Evaluator {
    fn object(&mut self, host: &dyn HostRead, ob: IdRef<ObjectId>, mode: EvalMode) -> Result<Arc<EvaluatedObject>, EvalError>;
    fn depsgraph_update(&mut self, host: &dyn HostRead);
}

// ------------------------------------------------------------------------------ hooks

/// Contract form (§9.5). Unusable for `hook_coll` getters: it needs `&mut dyn Host` for a read
/// (reads must work from `&dyn HostRead`, §12) and returns an owned `Value`, so it cannot hand
/// back a collection proxy that borrows the scene (F-HOOK-1).
///
/// ```compile_fail,E0308
/// use frap_probe_rna::{PropDef, RnaPtr};
/// use frap_probe_scene::{HostRead, RnaGetHook};
/// // A read (no transaction, `&dyn HostRead`, §12) cannot call a contract get hook:
/// fn read(host: &dyn HostRead, hook: RnaGetHook, this: &RnaPtr, p: &'static PropDef) {
///     let _ = hook(host, this, p);
/// }
/// ```
pub type RnaGetHook = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef) -> Result<Value, RnaError>;
/// Amended form (F-HOOK-1).
pub type RnaGetHookRead = for<'h> fn(&'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError>;
pub type RnaSetHook = fn(&mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError>;

pub struct RnaHookReg {
    pub srna: &'static str,
    pub prop: &'static str,
    pub get: Option<RnaGetHookRead>,
    pub set: Option<RnaSetHook>,
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1
#[linkme::distributed_slice]
pub static RNA_HOOKS: [RnaHookReg];

pub fn find_hook(srna: &'static StructDef, prop: &str) -> Option<&'static RnaHookReg> {
    // Walk the base chain so a hook on `ID.name` serves every ID type.
    let mut s = Some(srna);
    while let Some(d) = s {
        if let Some(h) = RNA_HOOKS.iter().find(|h| h.srna == d.identifier && h.prop == prop) {
            return Some(h);
        }
        s = d.base;
    }
    None
}
