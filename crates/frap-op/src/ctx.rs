// SPDX-License-Identifier: GPL-3.0-or-later
//! `OpCtx`: what an operator sees while it runs (ARCHITECTURE §10.1, CONTRACT frozen at M1 W0,
//! amended by RFC 0001 F-OP-5..9).

use std::sync::Arc;

use frap_bmesh::BMesh;
use frap_mesh::Mesh;
use frap_rna::{IdAny, IdRef, MeshId, ObjectId, OpArgs, OperatorSpec, Report, ReportType, Value};
use frap_scene::id::Object;
use frap_scene::{
    Base, BaseFlag, ChangeLog, Context, EditState, EvalMode, EvaluatedObject, Evaluator, Host,
    HostRead, Main, Snapshot,
};
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::def::{OpError, OpResult};
use crate::services::Services;

/// Where a new object goes (`helpers::placement`, §10.3).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Placement {
    /// Object location.
    pub location: [f32; 3],
    /// Object rotation (Euler XYZ, radians).
    pub rotation: [f32; 3],
    /// Scale baked into generated geometry (object scale stays 1, as in Blender).
    pub scale: [f32; 3],
}

/// Reports and results of one exec (F-OP-8).
#[derive(Clone, Debug, Default)]
pub struct OpOutput {
    /// Blender-style reports.
    pub reports: Vec<Report>,
    /// `Response.result`.
    pub result: IndexMap<SmolStr, Value>,
}

/// What an operator sees while it runs.
pub struct OpCtx<'h> {
    host: &'h mut dyn Host,
    services: &'h mut dyn Services,
    reports: Vec<Report>,
    result: IndexMap<SmolStr, Value>,
    op: &'static OperatorSpec,
}

impl<'h> OpCtx<'h> {
    /// Context for one invocation (built by `frap-session`, F-OP-8).
    pub fn new(
        host: &'h mut dyn Host,
        services: &'h mut dyn Services,
        op: &'static OperatorSpec,
    ) -> Self {
        OpCtx {
            host,
            services,
            reports: Vec::new(),
            result: IndexMap::new(),
            op,
        }
    }
    /// Reports and results, after exec (F-OP-8).
    pub fn finish(self) -> OpOutput {
        OpOutput {
            reports: self.reports,
            result: self.result,
        }
    }
    /// The running operator's spec.
    pub fn op(&self) -> &'static OperatorSpec {
        self.op
    }
    /// Context view.
    pub fn ctx(&self) -> Context<'_> {
        self.host.ctx()
    }
    /// `bpy.data`.
    pub fn main(&self) -> &Main {
        self.host.main()
    }
    /// `bpy.data` and the change log, for inserts into any `IdMap` (F-OP-9).
    pub fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
        self.host.main_mut()
    }
    /// Mutable object (COW, generation bump, logged).
    pub fn object_mut(&mut self, r: IdRef<ObjectId>) -> Result<&mut Object, OpError> {
        let (main, log) = self.host.main_mut();
        main.objects
            .get_mut(r, log)
            .ok_or_else(|| OpError::Runtime("object no longer exists".into()))
    }
    /// Mutable mesh (COW, generation bump, logged).
    pub fn mesh_mut(&mut self, r: IdRef<MeshId>) -> Result<&mut Mesh, OpError> {
        let (main, log) = self.host.main_mut();
        main.meshes
            .get_mut(r, log)
            .ok_or_else(|| OpError::Runtime("mesh no longer exists".into()))
    }
    /// Mutable edit BMesh (EditState COW, bumps `bm_gen`, resets the view cache). M2.
    pub fn bmesh_mut(&mut self, me: IdRef<MeshId>) -> Result<&mut BMesh, OpError> {
        let _ = me;
        Err(OpError::NotImplemented(
            "frap: Edit Mode arrives in M2 (bmesh_mut)".into(),
        ))
    }
    /// Read the context (tool settings, cursor, selection) while editing BMeshes (F-OP-5).
    pub fn split_edit(&mut self) -> (Context<'_>, &mut EditState) {
        self.host.split_edit()
    }
    /// Read the state while asking the evaluator for evaluated objects (F-OP-6); the context
    /// is `HostRead::ctx()` of the first element.
    pub fn split_eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        self.host.eval()
    }
    /// Session services (render, image cache, paths).
    pub fn services(&mut self) -> &mut dyn Services {
        &mut *self.services
    }
    /// COW clone of the state with animation applied at `frame`, for render/export (F-OP-7).
    pub fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, OpError> {
        self.host.snapshot_at(frame).map_err(OpError::Rna)
    }
    /// Creates an object (requested name, uniquified), links it to `context.collection`,
    /// and, in the **context view layer only**, deselects every other base, selects it and makes
    /// it active (`ED_object_add_type`). Other view layers of the scene get an unselected base
    /// (Blender's layer-collection sync; exclusion per view layer is WP 1.10/1.12).
    pub fn add_object(
        &mut self,
        name: &str,
        data: Option<IdAny>,
        at: Placement,
    ) -> IdRef<ObjectId> {
        let (coll, scene_ref, view_layer) = {
            let c = self.ctx();
            (c.collection(), c.win.scene, c.view_layer().name.clone())
        };
        let (main, log) = self.host.main_mut();
        let mut ob = Object::new(name, data);
        ob.location = at.location;
        ob.rotation_euler = at.rotation;
        let ob = main.objects.insert(name, ob, log);
        match coll {
            Some(c) => {
                if let Some(c) = main.collections.get_mut(c, log) {
                    c.objects.push(ob);
                }
            }
            None => {
                if let Some(s) = main.scenes.get_mut(scene_ref, log) {
                    s.collection.objects.push(ob);
                }
            }
        }
        if let Some(scene) = main.scenes.get_mut(scene_ref, log) {
            for vl in &mut scene.view_layers {
                if vl.name != view_layer {
                    vl.bases.push(Base {
                        object: ob,
                        flag: BaseFlag::empty(),
                    });
                    continue;
                }
                for b in &mut vl.bases {
                    b.flag.remove(BaseFlag::SELECTED);
                }
                vl.bases.push(Base {
                    object: ob,
                    flag: BaseFlag::SELECTED,
                });
                vl.active = Some(ob);
            }
        }
        log.context_changed = true;
        ob
    }
    /// The evaluated object (evaluates if needed).
    pub fn evaluated(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<EvaluatedObject>, OpError> {
        let (host, eval) = self.host.eval();
        eval.object(host, ob, EvalMode::Viewport)
            .map_err(|e| OpError::Runtime(e.message))
    }
    /// Adds a report.
    pub fn report(&mut self, level: ReportType, msg: impl Into<String>) {
        self.reports.push(Report::new(level, msg));
    }
    /// Sets an entry of `Response.result`.
    pub fn result(&mut self, key: &str, v: impl Into<Value>) {
        self.result.insert(key.into(), v.into());
    }
    /// Runs another operator with already-converted arguments (ops calling ops), without its
    /// poll, as Blender's C operators call each other. Its reports join this operator's; its
    /// `result` does not. An unknown or unimplemented idname is an error of this operator.
    pub fn run_op(&mut self, idname: &str, args: OpArgs) -> OpResult {
        let def = crate::registry::lookup(idname).map_err(|e| match e.code {
            c if c == crate::errors::OP_NOT_IMPLEMENTED => OpError::NotImplemented(e.message),
            _ => OpError::Runtime(e.message),
        })?;
        self.run_def(def, &args, false)
    }

    /// Runs `def` in a sub-context sharing this host and services (macro steps, `run_op`).
    /// Reports are appended; with `merge_result` the sub-operator's `result` entries are too
    /// (later steps of a macro overwrite earlier keys).
    pub(crate) fn run_def(
        &mut self,
        def: &'static crate::def::OperatorDef,
        args: &OpArgs,
        merge_result: bool,
    ) -> OpResult {
        let mut sub = OpCtx::new(&mut *self.host, &mut *self.services, def.spec);
        let r = crate::def::pipeline::execute(def, &mut sub, args);
        let out = sub.finish();
        self.reports.extend(out.reports);
        if merge_result {
            self.result.extend(out.result);
        }
        r
    }
}
