// SPDX-License-Identifier: GPL-3.0-or-later
//! `Session`: executor (IR → calls), transactions and response building (ARCHITECTURE §11.6,
//! §12, §13). One statement is one transaction; errors leave the state unchanged.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use std::collections::BTreeMap;
use std::sync::Arc;

use frap_eval::EvalCache;
use frap_imbuf::DecodedImage;
use frap_lang::{Command, Expr, Lit, PathExpr, Stmt, UnOp};
use frap_mesh::Mesh;
use frap_op::{ImageCache, OpCtx, OpError, OpStatus, PathResolver, Services};
use frap_render_scene::{GpuOptions, RenderError, RenderService};
use frap_rna::ptr::{IdNames, id_repr};
use frap_rna::{
    CoerceCx, Field, FrapError, IdAny, IdCode, IdKey, IdRef, ImageId, Input, MeshId, PropFlags,
    PyExc, Report, RnaError, RnaErrorKind, RnaPtr, Root, SetOutcome, Span, Value, coerce,
};
use frap_scene::{
    ChangeLog, Context, EditState, Evaluator, Host, HostRead, Main, Preferences, Snapshot,
    WindowState,
};
use indexmap::IndexMap;
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

use crate::errors::*;
use crate::resolve::{Target, item_type, resolve, srna_of};
use crate::response::{Response, Status, info, rna_json, value_json};

/// Everything a rollback restores (§12).
#[derive(Clone)]
pub(crate) struct State {
    main: Main,
    win: WindowState,
    edit: EditState,
    prefs: Preferences,
    time_dirty: bool,
}

impl HostRead for State {
    fn main(&self) -> &Main {
        &self.main
    }
    fn win(&self) -> &WindowState {
        &self.win
    }
    fn edit(&self) -> &EditState {
        &self.edit
    }
    fn ctx(&self) -> Context<'_> {
        Context {
            main: &self.main,
            win: &self.win,
            prefs: &self.prefs,
        }
    }
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh> {
        // Edit-mode views (§6.3) arrive with M2; in object mode the view is the original.
        self.main
            .meshes
            .get_arc(mesh)
            .expect("invariant: mesh_view of an existing mesh")
    }
}

/// How a session starts (§12, §13.1 `--empty` / `--file`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Startup {
    /// Blender's factory startup (§5.7; default).
    #[default]
    Factory,
    /// `frap --empty` = `wm.read_homefile(use_empty=True)` (`startup::empty()`, WP 1.10).
    Empty,
    /// `frap --file <path.frap>` (`.frap` load, WP 1.23).
    File(PathBuf),
}

/// Response verbosity (`--verbosity`, §13.1).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Verbosity {
    /// Drops `context` and `edit_mesh`.
    Minimal,
    /// The default envelope (§13.2).
    #[default]
    Normal,
    /// Adds evaluated element counts and `modifier_status`.
    Full,
}

/// Resource limits (§12): exceeding one raises `RuntimeError` code `LIMIT_EXCEEDED` with
/// rollback.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// `--max-elements` (default 50,000,000 corners).
    pub max_elements: u64,
    /// `--stmt-timeout` (cooperative checks in long loops; `None` = no timeout).
    pub stmt_timeout: Option<std::time::Duration>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_elements: 50_000_000,
            stmt_timeout: None,
        }
    }
}

/// Session options (§12). `#[non_exhaustive]`: construct with `SessionOptions::default()`
/// and assign fields (`opts.cwd = …`), so later milestones can add options without breaking
/// front-ends.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct SessionOptions {
    /// Working directory for relative paths.
    pub cwd: PathBuf,
    /// GPU selection for the lazily created render service.
    pub gpu: GpuOptions,
    /// Initial state.
    pub startup: Startup,
    /// Response verbosity.
    pub verbosity: Verbosity,
    /// `--max-items`: names listed in a `$coll` value (default 100, §13.3).
    pub max_items: usize,
    /// `--max-array`: scalars in a `$array` value (default 65,536, §13.3).
    pub max_array: usize,
    /// Resource limits.
    pub limits: Limits,
    /// `--fail-fast`: the front-end stops after the first failed statement (exit code 1).
    pub fail_fast: bool,
}

impl Default for SessionOptions {
    fn default() -> Self {
        SessionOptions {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            gpu: GpuOptions::from_env(),
            startup: Startup::Factory,
            verbosity: Verbosity::Normal,
            max_items: 100,
            max_array: 65_536,
            limits: Limits::default(),
            fail_fast: false,
        }
    }
}

type RenderFactory = fn(&GpuOptions) -> Result<Box<dyn RenderService>, RenderError>;

#[cfg(feature = "gpu")]
fn render_factory() -> RenderFactory {
    frap_render::new_service
}

#[cfg(not(feature = "gpu"))]
fn render_factory() -> RenderFactory {
    fn unavailable(_: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError> {
        Err(RenderError::NoAdapter(
            "frap was built without the gpu feature".into(),
        ))
    }
    unavailable
}

/// Decoded pixels by `(IdKey, generation)` (outside the snapshot).
#[derive(Default)]
struct ImageCacheImpl {
    entries: BTreeMap<(IdKey, u64), Arc<DecodedImage>>,
}

impl ImageCache for ImageCacheImpl {
    fn get(&self, image: IdRef<ImageId>, generation: u64) -> Option<Arc<DecodedImage>> {
        self.entries.get(&(image.key(), generation)).cloned()
    }
    fn put(&mut self, image: IdRef<ImageId>, generation: u64, pixels: Arc<DecodedImage>) {
        self.entries.insert((image.key(), generation), pixels);
    }
    fn evict(&mut self, key: IdKey) {
        self.entries.retain(|(k, _), _| *k != key);
    }
}

/// Side-effect services, never snapshotted (§12).
struct SessionServices {
    render: Option<Box<dyn RenderService>>,
    render_factory: RenderFactory,
    gpu: GpuOptions,
    images: ImageCacheImpl,
    paths: PathResolver,
}

impl Services for SessionServices {
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError> {
        if self.render.is_none() {
            let svc = (self.render_factory)(&self.gpu).map_err(|e| OpError::Gpu(e.to_string()))?;
            self.render = Some(svc);
        }
        Ok(self
            .render
            .as_deref_mut()
            .expect("invariant: render service just created"))
    }
    fn images(&mut self) -> &mut dyn ImageCache {
        &mut self.images
    }
    fn paths(&self) -> &PathResolver {
        &self.paths
    }
}

/// The `Host` implementation: a field of `Session` disjoint from its services, so an `OpCtx`
/// can borrow both (RFC 0001 F-SES-1).
pub(crate) struct HostImpl {
    state: State,
    eval: EvalCache,
    log: ChangeLog,
    reports: Vec<Report>,
}

impl HostRead for HostImpl {
    fn main(&self) -> &Main {
        &self.state.main
    }
    fn win(&self) -> &WindowState {
        &self.state.win
    }
    fn edit(&self) -> &EditState {
        &self.state.edit
    }
    fn ctx(&self) -> Context<'_> {
        self.state.ctx()
    }
    fn mesh_view(&self, mesh: IdRef<MeshId>) -> Arc<Mesh> {
        self.state.mesh_view(mesh)
    }
}

impl Host for HostImpl {
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
        (&mut self.state.main, &mut self.log)
    }
    fn edit_mut(&mut self) -> &mut EditState {
        &mut self.state.edit
    }
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState) {
        let s = &mut self.state;
        (
            Context {
                main: &s.main,
                win: &s.win,
                prefs: &s.prefs,
            },
            &mut s.edit,
        )
    }
    fn win_mut(&mut self) -> &mut WindowState {
        &mut self.state.win
    }
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        (&self.state, &mut self.eval)
    }
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError> {
        // Animation (M6) is applied on the clone at `frame`; until then the clone is as-is.
        let _ = frame;
        Ok(Snapshot {
            host: Box::new(self.state.clone()),
            eval: Box::new(EvalCache::new()),
        })
    }
    fn mark_time_dirty(&mut self) {
        self.state.time_dirty = true;
    }
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        // No animation before M6 (§7.3): nothing is pending.
        self.state.time_dirty = false;
        Ok(())
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn set_path(
        &mut self,
        owner: Root,
        data_path: &str,
        index: i32,
        v: Value,
    ) -> Result<(), RnaError> {
        let _ = (owner, data_path, index, v);
        Err(RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::NotImplementedError,
            "frap: animation writes arrive in M6",
        ))
    }
}

/// A frap session (one scene state, many statements).
pub struct Session {
    host: HostImpl,
    services: SessionServices,
    opts: SessionOptions,
    seq: u64,
}

fn not_yet(what: &str) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!("frap M0: {what} is not supported yet"),
    )
}

impl Session {
    /// Starts from Blender's factory startup (§5.7).
    pub fn new(opts: SessionOptions) -> Result<Session, FrapError> {
        frap_ops_all::link();
        let (main, win) = match &opts.startup {
            Startup::Factory => frap_scene::startup::factory(),
            Startup::Empty => return Err(not_yet("the empty startup (WP 1.10)")),
            Startup::File(_) => return Err(not_yet("opening a .frap file at startup (WP 1.23)")),
        };
        Ok(Session {
            host: HostImpl {
                state: State {
                    main,
                    win,
                    edit: EditState::default(),
                    prefs: Preferences::default(),
                    time_dirty: false,
                },
                eval: EvalCache::new(),
                log: ChangeLog::default(),
                reports: Vec::new(),
            },
            services: SessionServices {
                render: None,
                render_factory: render_factory(),
                gpu: opts.gpu.clone(),
                images: ImageCacheImpl::default(),
                paths: PathResolver {
                    cwd: opts.cwd.clone(),
                    blend_dir: None,
                },
            },
            opts,
            seq: 0,
        })
    }

    /// The options this session was started with.
    pub fn options(&self) -> &SessionOptions {
        &self.opts
    }

    /// Read-only access to `bpy.data` (tests, schema export).
    pub fn main(&self) -> &Main {
        &self.host.state.main
    }

    /// Runs a complete source unit; one response per statement.
    pub fn exec_source(&mut self, src: &str) -> Vec<Response> {
        self.exec_source_at(src, 1)
    }

    /// Like [`Session::exec_source`], with line numbers starting at `first_line`.
    pub fn exec_source_at(&mut self, src: &str, first_line: u32) -> Vec<Response> {
        frap_lang::parse_complete_at(src, first_line)
            .into_iter()
            .map(|r| match r {
                Ok(stmt) => self.exec(&stmt),
                Err(e) => {
                    self.seq += 1;
                    let line = e.span.map(|s| s.line).unwrap_or(first_line);
                    self.error_response("invalid", None, line, e)
                }
            })
            .collect()
    }

    /// A response for input that never became a statement (e.g. invalid UTF-8 on a line).
    pub fn reject(&mut self, line: u32, e: FrapError) -> Response {
        self.seq += 1;
        self.error_response("invalid", None, line, e)
    }

    /// Executes one parsed statement.
    pub fn exec(&mut self, stmt: &Stmt) -> Response {
        self.seq += 1;
        let r = match &stmt.cmd {
            Command::Invoke {
                idname,
                positional,
                args,
            } => self.exec_invoke(stmt, idname, positional, args),
            Command::Get { target } => self.exec_get(stmt, target),
            Command::Set { target, value, .. } => self.exec_set(stmt, target, value),
            Command::Noop { .. } => Ok(self.base("noop", stmt.line, Status::Finished)),
            _ => Err(("noop", None, not_yet("this statement kind"))),
        };
        r.unwrap_or_else(|(kind, op, e)| {
            self.error_response(kind, op, stmt.line, e.with_span(stmt.span))
        })
    }

    /// Invokes an operator by idname with keyword inputs (front-end-free entry point, §11.6).
    pub fn invoke(&mut self, idname: &str, kwargs: IndexMap<String, Input>) -> Response {
        self.seq += 1;
        let line = 1;
        let fake = Stmt {
            cmd: Command::Noop {
                reason: frap_lang::NoopReason::Pass,
            },
            span: Span::default(),
            line,
            form: frap_lang::Form::Bpy,
        };
        let kw: Vec<(String, Input, Span)> = kwargs
            .into_iter()
            .map(|(k, v)| (k, v, Span::default()))
            .collect();
        self.invoke_inputs(&fake, idname, vec![], kw)
            .unwrap_or_else(|(kind, op, e)| self.error_response(kind, op, line, e))
    }

    /// Reads a property path (§11.6; the path uses the shared path grammar).
    pub fn get(&mut self, path: &str) -> Response {
        let _ = path;
        todo!("WP 1.14")
    }

    /// Assigns a property path (§11.6).
    pub fn set(&mut self, path: &str, value: Input) -> Response {
        let _ = (path, value);
        todo!("WP 1.14")
    }

    /// Calls an RNA function path (§11.6).
    pub fn call(
        &mut self,
        path: &str,
        args: Vec<Input>,
        kwargs: IndexMap<String, Input>,
    ) -> Response {
        let _ = (path, args, kwargs);
        todo!("WP 1.14")
    }

    // ------------------------------------------------------------------ helpers

    fn base(&self, kind: &'static str, line: u32, status: Status) -> Response {
        Response {
            id: None,
            seq: self.seq,
            line,
            kind,
            op: None,
            status,
            path: None,
            func: None,
            value: None,
            rna: None,
            reports: vec![],
            result: None,
            changes: None,
            error: None,
            context: (self.opts.verbosity != Verbosity::Minimal).then(|| self.context_json()),
            edit_mesh: None,
            hints: vec![],
        }
    }

    fn error_response(
        &self,
        kind: &'static str,
        op: Option<SmolStr>,
        line: u32,
        e: FrapError,
    ) -> Response {
        let mut r = self.base(kind, line, Status::Error);
        r.op = op;
        r.error = Some(e);
        r
    }

    fn context_json(&self) -> J {
        let main = &self.host.state.main;
        let ctx = self.host.state.ctx();
        let name = |r| main.objects.name_of(r).map(J::from).unwrap_or(J::Null);
        json!({
            "mode": ctx.mode().as_str(),
            "active_object": ctx.active_object().map(name).unwrap_or(J::Null),
            "selected_objects": ctx.selected_objects().into_iter().map(name).collect::<Vec<_>>(),
            "frame_current": ctx.scene().frame_current,
        })
    }

    fn repr_id(&self, id: IdAny) -> String {
        match self.host.state.main.id_name(id) {
            Some((n, lib)) => id_repr(id.code, &n, lib.as_deref()),
            None => id_repr(id.code, "<removed>", None),
        }
    }

    fn changes_json(&self, log: &ChangeLog) -> J {
        let rank = |c: IdCode| {
            [
                IdCode::OB,
                IdCode::ME,
                IdCode::MA,
                IdCode::CA,
                IdCode::LA,
                IdCode::GR,
                IdCode::SC,
            ]
            .iter()
            .position(|x| *x == c)
            .unwrap_or(99)
        };
        let mut added = log.added.clone();
        added.sort_by_key(|a| rank(a.code)); // stable: object before its data
        let added_j: Vec<J> = added.iter().map(|a| J::from(self.repr_id(*a))).collect();
        let modified: Vec<J> = log
            .modified
            .iter()
            .filter(|m| !log.added.contains(m))
            .map(|m| J::from(self.repr_id(*m)))
            .collect();
        let removed: Vec<J> = log
            .removed
            .iter()
            .map(|(a, n)| J::from(id_repr(a.code, n, None)))
            .collect();
        let renamed: Vec<J> = log
            .renamed
            .iter()
            .map(|(a, old)| json!({"from": id_repr(a.code, old, None), "to": self.repr_id(*a)}))
            .collect();
        json!({"added": added_j, "removed": removed, "modified": modified, "renamed": renamed})
    }

    fn eval(&self, e: &Expr) -> Result<Input, FrapError> {
        Ok(match e {
            Expr::Lit(l, _) => match l {
                Lit::Int(i) => Input::Int(*i),
                Lit::Float(f) => Input::Float(*f),
                Lit::Str(s) => Input::Str(s.clone()),
                Lit::Bool(b) => Input::Bool(*b),
                Lit::None => Input::None,
            },
            Expr::Bare(s, _) => Input::Bare(s.clone()),
            Expr::Tuple(v, _) => {
                Input::Tuple(v.iter().map(|x| self.eval(x)).collect::<Result<_, _>>()?)
            }
            Expr::List(v, _) => {
                Input::List(v.iter().map(|x| self.eval(x)).collect::<Result<_, _>>()?)
            }
            Expr::Unary(op, inner, sp) => {
                let v = self.eval(inner)?;
                let sign = if *op == UnOp::Neg { -1 } else { 1 };
                match v {
                    Input::Int(i) => Input::Int(i * sign),
                    Input::Float(f) => Input::Float(f * sign as f64),
                    Input::Bool(b) => Input::Int(b as i64 * sign),
                    other => {
                        return Err(FrapError::new(
                            PyExc::TypeError,
                            RNA_TYPE,
                            format!(
                                "bad operand type for unary {}: '{}'",
                                if *op == UnOp::Neg { '-' } else { '+' },
                                other.py_type_name()
                            ),
                        )
                        .with_span(*sp));
                    }
                }
            }
            Expr::Path(p) => {
                let main = &self.host.state.main;
                match resolve(main, p).map_err(|e| e.with_span(p.span))? {
                    Target::Id(id) => Input::Ptr(RnaPtr::id(id, srna_of(id.code))),
                    Target::Prop { id, prop } => {
                        let rna = main.id(id).expect("invariant: resolved ID exists");
                        match rna
                            .get(prop)
                            .map_err(|e| e.into_frap("", |_| RNA_NO_ATTRIBUTE))?
                        {
                            Field::Value(Value::FloatArray { data, .. }) => {
                                Input::Tuple(data.iter().map(|f| Input::Float(*f as f64)).collect())
                            }
                            Field::Value(Value::Float(f)) => Input::Float(f as f64),
                            Field::Value(Value::Int(i)) => Input::Int(i),
                            Field::Value(Value::Bool(b)) => Input::Bool(b),
                            Field::Value(Value::Str(s)) | Field::Value(Value::Enum(s)) => {
                                Input::Str(s.to_string())
                            }
                            _ => return Err(not_yet("using this value in an expression")),
                        }
                    }
                    _ => return Err(not_yet("using this path in an expression")),
                }
            }
            _ => return Err(not_yet("this expression")),
        })
    }

    // ------------------------------------------------------------------ op

    fn exec_invoke(
        &mut self,
        stmt: &Stmt,
        idname: &SmolStr,
        positional: &[Expr],
        args: &[frap_lang::Arg],
    ) -> Result<Response, (&'static str, Option<SmolStr>, FrapError)> {
        let op = Some(idname.clone());
        let pos = positional
            .iter()
            .map(|e| self.eval(e))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| ("op", op.clone(), e))?;
        let mut kw = Vec::with_capacity(args.len());
        for a in args {
            let v = self.eval(&a.value).map_err(|e| ("op", op.clone(), e))?;
            let name = a.name.clone().map(|n| n.to_string()).unwrap_or_default();
            kw.push((name, v, a.span));
        }
        self.invoke_inputs(stmt, idname, pos, kw)
    }

    fn invoke_inputs(
        &mut self,
        stmt: &Stmt,
        idname: &str,
        positional: Vec<Input>,
        kwargs: Vec<(String, Input, Span)>,
    ) -> Result<Response, (&'static str, Option<SmolStr>, FrapError)> {
        let op = Some(SmolStr::new(idname));
        let fail = |e: FrapError| ("op", op.clone(), e);
        // §10.2 step 1: lookup.
        let def = frap_op::lookup(idname).map_err(fail)?;
        // Steps 2–3: arguments, before poll.
        let spans: Vec<(String, Span)> = kwargs.iter().map(|(k, _, s)| (k.clone(), *s)).collect();
        let kw: Vec<(String, Input)> = kwargs.into_iter().map(|(k, v, _)| (k, v)).collect();
        let (args, mut reports) = frap_op::convert_args(def.spec, positional, kw).map_err(|e| {
            let arg_span = e
                .detail
                .get("argument")
                .and_then(|a| a.as_str())
                .and_then(|a| spans.iter().find(|(k, _)| k == a))
                .map(|(_, s)| *s);
            fail(match arg_span {
                Some(mut s) if s != Span::default() => {
                    // An unknown keyword points at the keyword itself (§13.5).
                    if e.code == frap_op::errors::OP_ARG_UNKNOWN {
                        s.len = e
                            .detail
                            .get("argument")
                            .and_then(|a| a.as_str())
                            .map_or(s.len, |a| a.len() as u32);
                    }
                    e.with_span(s)
                }
                _ => e,
            })
        })?;
        // Step 4 (ensure evaluated) is a no-op in M0. Step 5: poll.
        frap_op::check_poll(def, &self.host.state.ctx()).map_err(fail)?;
        // Steps 6–8: transaction + exec.
        let snapshot = self.host.state.clone();
        self.host.log = ChangeLog::default();
        self.host.reports.clear();
        let (outcome, out) = {
            let mut cx = OpCtx::new(&mut self.host, &mut self.services, def.spec);
            let r = catch_unwind(AssertUnwindSafe(|| (def.exec)(&mut cx, &args)));
            (r, cx.finish())
        };
        let log = std::mem::take(&mut self.host.log);
        reports.append(&mut self.host.reports);
        reports.extend(out.reports);
        let result = out.result;
        let status = match outcome {
            Ok(Ok(OpStatus::Finished)) => Status::Finished,
            Ok(Ok(OpStatus::Cancelled)) => Status::Cancelled,
            Ok(Err(e)) => {
                self.host.state = snapshot;
                let mut r = self.error_response(
                    "op",
                    op.clone(),
                    stmt.line,
                    e.into_frap().with_span(stmt.span),
                );
                r.reports = reports;
                return Ok(r);
            }
            Err(panic) => {
                self.host.state = snapshot;
                return Err(fail(FrapError::internal_panic(panic)));
            }
        };
        if status == Status::Cancelled {
            self.host.state = snapshot;
        }
        // Step 9: response.
        let mut r = self.base("op", stmt.line, status);
        r.op = op;
        r.reports = reports;
        if status == Status::Finished {
            let names: &dyn IdNames = &self.host.state.main;
            let mut res = Map::new();
            for (k, v) in &result {
                res.insert(k.to_string(), value_json(v, names));
            }
            r.result = Some(res);
            r.changes = Some(self.changes_json(&log));
        }
        Ok(r)
    }

    // ------------------------------------------------------------------ get / set

    fn exec_get(
        &mut self,
        stmt: &Stmt,
        target: &PathExpr,
    ) -> Result<Response, (&'static str, Option<SmolStr>, FrapError)> {
        let fail = |e: FrapError| ("get", None, e);
        let main = &self.host.state.main;
        let t = resolve(main, target).map_err(|e| fail(e.with_span(target.span)))?;
        let mut r = self.base("get", stmt.line, Status::Finished);
        match t {
            Target::DataRoot => {
                r.path = Some("bpy.data".into());
                r.value = Some(json!({"$ptr": "bpy.data", "type": "BlendData"}));
            }
            Target::Coll(c) => {
                let names = main.names(c).unwrap_or_default();
                r.path = Some(format!("bpy.data.{c}"));
                r.value = Some(json!({
                    "$coll": format!("bpy.data.{c}"), "item_type": item_type(c), "len": names.len(),
                    "items": names.iter().map(|n| n.as_str()).collect::<Vec<_>>(),
                }));
            }
            Target::Id(id) => {
                let repr = self.repr_id(id);
                r.path = Some(repr.clone());
                r.value = Some(json!({"$ref": repr}));
            }
            Target::Prop { id, prop } => {
                let rna = main.id(id).expect("invariant: resolved ID exists");
                let field = rna
                    .get(prop)
                    .map_err(|e| fail(e.into_frap("", |_| RNA_NO_ATTRIBUTE)))?;
                r.path = Some(format!("{}.{}", self.repr_id(id), prop.identifier));
                match field {
                    Field::Value(v) => {
                        r.rna = Some(rna_json(prop, &v));
                        r.value = Some(value_json(&v, main));
                    }
                    Field::Id(i) => {
                        r.value = Some(
                            i.map(|i| json!({"$ref": self.repr_id(i)}))
                                .unwrap_or(J::Null),
                        )
                    }
                    _ => return Err(fail(not_yet("this field kind"))),
                }
            }
            Target::Context(m) => {
                let ctx = self.host.state.ctx();
                let as_ref =
                    |o: frap_rna::IdRef<frap_rna::ObjectId>| json!({"$ref": self.repr_id(o.any())});
                r.path = Some(format!("bpy.context.{m}"));
                r.value = Some(match m {
                    "active_object" | "object" => {
                        ctx.active_object().map(as_ref).unwrap_or(J::Null)
                    }
                    "selected_objects" => {
                        J::Array(ctx.selected_objects().into_iter().map(as_ref).collect())
                    }
                    "mode" => J::from(ctx.mode().as_str()),
                    "scene" => json!({"$ref": self.repr_id(ctx.win.scene.any())}),
                    _ => J::Null,
                });
            }
        }
        Ok(r)
    }

    fn exec_set(
        &mut self,
        stmt: &Stmt,
        target: &PathExpr,
        value: &Expr,
    ) -> Result<Response, (&'static str, Option<SmolStr>, FrapError)> {
        let fail = |e: FrapError| ("set", None, e);
        let t =
            resolve(&self.host.state.main, target).map_err(|e| fail(e.with_span(target.span)))?;
        let (id, prop) = match t {
            Target::Prop { id, prop } => (id, prop),
            Target::Context(m) => {
                return Err(fail(
                    FrapError::new(
                        PyExc::AttributeError,
                        CONTEXT_READONLY,
                        format!("bpy_struct: Context property \"{m}\" is read-only"),
                    )
                    .with_hint(
                        "change selection with select_set() and the active object with view_layer.objects.active (M1)",
                    ),
                ));
            }
            _ => return Err(fail(not_yet("assigning to a datablock or collection"))),
        };
        let srna = srna_of(id.code);
        if prop.flags.contains(PropFlags::READONLY) {
            return Err(fail(FrapError::new(
                PyExc::AttributeError,
                RNA_READONLY,
                format!(
                    "bpy_struct: attribute \"{}\" from \"{}\" is read-only",
                    prop.identifier, srna.identifier
                ),
            )));
        }
        let input = self.eval(value).map_err(fail)?;
        let written = input.clone();
        let coerced = coerce(
            prop,
            input,
            &CoerceCx::RnaSet {
                owner: srna.identifier,
            },
        )
        .map_err(|e| {
            fail(e.into_frap("bpy_struct: item.attr = val: ", |k| match k {
                RnaErrorKind::Enum => RNA_ENUM,
                RnaErrorKind::Length => RNA_LENGTH,
                _ => RNA_TYPE,
            }))
        })?;
        let snapshot = self.host.state.clone();
        let mut log = ChangeLog::default();
        let outcome = {
            let main = &mut self.host.state.main;
            match main.id_mut(id, &mut log) {
                Some(rna) => rna.set(prop, coerced.value.clone()),
                None => Ok(SetOutcome::NeedsHook),
            }
        };
        match outcome {
            Ok(SetOutcome::Done) => {}
            Ok(SetOutcome::NeedsHook) => {
                self.host.state = snapshot;
                return Err(fail(not_yet(&format!(
                    "assigning {}.{} (needs an RNA hook, M1)",
                    srna.identifier, prop.identifier
                ))));
            }
            Err(e) => {
                self.host.state = snapshot;
                return Err(fail(
                    e.into_frap("bpy_struct: item.attr = val: ", |_| RNA_TYPE),
                ));
            }
        }
        let mut r = self.base("set", stmt.line, Status::Finished);
        r.path = Some(format!("{}.{}", self.repr_id(id), prop.identifier));
        r.value = Some(value_json(&coerced.value, &self.host.state.main));
        if coerced.clamped {
            r.reports.push(info(frap_rna::clamp_report(prop, &written)));
        }
        r.changes = Some(self.changes_json(&log));
        Ok(r)
    }
}
