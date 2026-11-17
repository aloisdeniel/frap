// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-op` (§10.1): `OperatorDef`, `Poll`, `Services`, `OpCtx`.
//!
//! # Contract failures demonstrated as compile_fail doctests
//!
//! F-OP-5 — `OpCtx::split_edit` cannot be implemented from the §9.5 `Host` methods
//! (`ctx()` borrows the host shared, `edit_mut()` mutably):
//!
//! ```compile_fail,E0502
//! use frap_probe_scene::{Context, EditState, Host};
//! fn split_edit<'a>(host: &'a mut dyn Host) -> (Context<'a>, &'a mut EditState) {
//!     let ctx = host.ctx();
//!     (ctx, host.edit_mut())
//! }
//! ```
//!
//! F-OP-6 — the contract `split_eval() -> (Context, &mut dyn Evaluator)` is unusable:
//! `Evaluator::object` needs a `&dyn HostRead`, and a `Context` is not one (it has no
//! `EditState`, so it cannot implement `HostRead::edit`/`mesh_view`):
//!
//! ```compile_fail,E0277
//! use frap_probe_op::OpCtx;
//! use frap_probe_rna::{IdRef, ObjectId};
//! use frap_probe_scene::EvalMode;
//! fn eval(cx: &mut OpCtx<'_>, ob: IdRef<ObjectId>) {
//!     let (ctx, ev) = cx.split_eval();
//!     let _ = ev.object(&ctx, ob, EvalMode::Viewport);
//! }
//! ```
// RFC 0001 F-LINK-1: linkme registration is incompatible with `forbid(unsafe_code)`.
#![deny(unsafe_code)]

use std::sync::Arc;

use frap_probe_mesh::Mesh;
use frap_probe_render_scene::{RenderError, RenderService};
use frap_probe_rna::*;
use frap_probe_scene::{BMesh, Context, EditState, EvalMode, EvaluatedObject, Evaluator, Host, HostRead, Main, Object, Snapshot};
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

// ------------------------------------------------------------------------------ OperatorDef

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PollReq {
    ModeObject,
    ModeEditMesh,
    ActiveObject,
    ActiveType(&'static str),
    HasSelectedObjects,
    SceneHasCamera,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PollFail {
    pub message: Option<String>,
    pub unmet: SmallVec<[PollReq; 2]>,
    pub hint: Option<String>,
}

#[derive(Copy, Clone)]
pub struct Poll {
    pub requires: &'static [PollReq],
    pub check: fn(&Context) -> Result<(), PollFail>,
}

pub mod poll {
    use super::*;
    fn ok(_: &Context) -> Result<(), PollFail> {
        Ok(())
    }
    pub const ALWAYS: Poll = Poll { requires: &[], check: ok };
    fn objectmode(c: &Context) -> Result<(), PollFail> {
        if c.mode() == frap_probe_scene::ContextMode::Object {
            Ok(())
        } else {
            Err(PollFail {
                message: None,
                unmet: smallvec::smallvec![PollReq::ModeObject],
                hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
            })
        }
    }
    pub const OBJECTMODE: Poll = Poll {
        requires: &[PollReq::ModeObject],
        check: objectmode,
    };
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OpStatus {
    Finished,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpError {
    Runtime(String),
    Rna(RnaError),
    NotImplemented(String),
    Io(String),
    Gpu(String),
    Limit(String),
}

impl From<RnaError> for OpError {
    fn from(e: RnaError) -> Self {
        OpError::Rna(e)
    }
}
impl From<RenderError> for OpError {
    fn from(e: RenderError) -> Self {
        OpError::Gpu(format!("{e:?}"))
    }
}

pub type OpResult = Result<OpStatus, OpError>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OpKind {
    Normal,
    Macro,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tier {
    T0,
    T1,
    T2,
    T3,
    T4,
}
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Support {
    Full,
    Partial(&'static str),
    HeadlessExtended(&'static str),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumItemOwned {
    pub identifier: SmolStr,
    pub name: String,
    pub description: String,
}
pub type DynEnumFn = fn(&Context) -> Vec<EnumItemOwned>;

pub struct OperatorDef {
    pub spec: &'static OperatorSpec,
    pub poll: Poll,
    pub exec: fn(&mut OpCtx, &OpArgs) -> OpResult,
    pub kind: OpKind,
    pub dyn_enums: &'static [(&'static str, DynEnumFn)],
    pub tier: Tier,
    pub support: Support,
    pub notes: &'static [&'static str],
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1
#[linkme::distributed_slice]
pub static OPERATORS: [OperatorDef];

pub fn lookup(idname: &str) -> Option<&'static OperatorDef> {
    OPERATORS.iter().find(|d| d.spec.idname == idname)
}

/// §10.3 `placement()` output (probe subset).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Placement {
    pub location: [f32; 3],
    pub rotation: [f32; 3],
}

// --------------------------------------------------------------------------------- Services

/// Decoded pixels by (IdKey, gen). Probe: empty.
pub trait ImageCache {
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
/// `//` relative to bpy.data.filepath, cwd. Probe: cwd only.
#[derive(Clone, Debug, Default)]
pub struct PathResolver {
    pub cwd: std::path::PathBuf,
}

/// Side-effect services that live outside the snapshot (never rolled back).
pub trait Services {
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError>;
    fn images(&mut self) -> &mut dyn ImageCache;
    fn paths(&self) -> &PathResolver;
}

// ------------------------------------------------------------------------------------ OpCtx

pub struct OpCtx<'h> {
    host: &'h mut dyn Host,
    services: &'h mut dyn Services,
    reports: Vec<Report>,
    result: IndexMap<SmolStr, Value>,
    op: &'static OperatorSpec,
}

/// What a finished invocation hands back to the session (RFC 0001 F-OP-8: the contract has no
/// constructor and no way to take reports/result out of an `OpCtx`).
#[derive(Debug)]
pub struct OpOutput {
    pub reports: Vec<Report>,
    pub result: IndexMap<SmolStr, Value>,
}

fn op_err(msg: &str) -> OpError {
    OpError::Runtime(msg.into())
}

impl<'h> OpCtx<'h> {
    pub fn new(host: &'h mut dyn Host, services: &'h mut dyn Services, op: &'static OperatorSpec) -> Self {
        OpCtx {
            host,
            services,
            reports: vec![],
            result: IndexMap::new(),
            op,
        }
    }
    pub fn finish(self) -> OpOutput {
        OpOutput {
            reports: self.reports,
            result: self.result,
        }
    }
    pub fn spec(&self) -> &'static OperatorSpec {
        self.op
    }

    pub fn ctx(&self) -> Context<'_> {
        self.host.ctx()
    }
    pub fn main(&self) -> &Main {
        self.host.main()
    }
    /// RFC 0001 F-OP-9: not in §10.1. `object_mut`/`mesh_mut` cover two ID types; operators such
    /// as `material.new`, `camera_add` (Camera data) or `collection.create` insert into other maps.
    pub fn main_mut(&mut self) -> (&mut Main, &mut frap_probe_scene::ChangeLog) {
        self.host.main_mut()
    }
    /// Link to the context collection (probe: active view layer), select only it, make active.
    pub fn add_object(&mut self, name: &str, data: Option<IdAny>, at: Placement) -> IdRef<ObjectId> {
        let scene = self.host.win().scene;
        let (main, log) = self.host.main_mut();
        let mut ob = Object::new(name, data);
        ob.location = at.location;
        ob.rotation_euler = at.rotation;
        let r = main.objects.insert(name, ob, log);
        let sc = main.scenes.get_mut(scene, log).expect("invariant: window scene");
        let vl = &mut sc.view_layers[0];
        for b in &mut vl.bases {
            b.selected = false;
        }
        vl.bases.push(frap_probe_scene::Base { object: r, selected: true });
        vl.active = Some(r);
        log.context_changed = true;
        r
    }
    pub fn object_mut(&mut self, r: IdRef<ObjectId>) -> Result<&mut Object, OpError> {
        let (main, log) = self.host.main_mut();
        main.objects.get_mut(r, log).ok_or_else(|| op_err("object not found"))
    }
    pub fn mesh_mut(&mut self, r: IdRef<MeshId>) -> Result<&mut Mesh, OpError> {
        let (main, log) = self.host.main_mut();
        main.meshes.get_mut(r, log).ok_or_else(|| op_err("mesh not found"))
    }
    /// EditState COW, bumps bm_gen.
    pub fn bmesh_mut(&mut self, me: IdRef<MeshId>) -> Result<&mut BMesh, OpError> {
        let em = self
            .host
            .edit_mut()
            .meshes
            .get_mut(&me.key())
            .ok_or_else(|| op_err("mesh not in edit mode"))?;
        em.bm_gen = next_gen();
        Ok(Arc::make_mut(&mut em.bm))
    }
    /// Read the context (tool settings, cursor, selection) while editing BMeshes.
    /// Needs `Host::split_edit` (F-OP-5).
    pub fn split_edit(&mut self) -> (Context<'_>, &mut EditState) {
        self.host.split_edit()
    }
    /// Contract signature (§10.1). Compiles, but the result cannot drive the evaluator (F-OP-6).
    pub fn split_eval(&mut self) -> (Context<'_>, &mut dyn Evaluator) {
        let (hr, ev) = self.host.eval();
        (hr.ctx(), ev)
    }
    /// Amended form (F-OP-6): `HostRead` gives `ctx()` as well.
    pub fn split_eval_host(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        self.host.eval()
    }
    pub fn services(&mut self) -> &mut dyn Services {
        &mut *self.services
    }
    pub fn evaluated(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<EvaluatedObject>, OpError> {
        let (hr, ev) = self.host.eval();
        ev.object(hr, ob, EvalMode::Viewport).map_err(|e| OpError::Runtime(e.0))
    }
    /// F-OP-7: COW clone at a frame, for render/export.
    pub fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, OpError> {
        Ok(self.host.snapshot_at(frame)?)
    }
    pub fn report(&mut self, level: ReportType, msg: impl Into<String>) {
        self.reports.push(Report {
            level,
            message: msg.into(),
        })
    }
    pub fn result(&mut self, key: &str, v: impl Into<Value>) {
        self.result.insert(key.into(), v.into());
    }
    /// Ops calling ops: a nested `OpCtx` reborrowing the same host and services.
    pub fn run_op(&mut self, idname: &str, args: OpArgs) -> OpResult {
        let def = lookup(idname).ok_or_else(|| OpError::NotImplemented(idname.into()))?;
        let mut sub = OpCtx::new(&mut *self.host, &mut *self.services, def.spec);
        let r = (def.exec)(&mut sub, &args);
        let out = sub.finish();
        self.reports.extend(out.reports);
        r
    }
}
