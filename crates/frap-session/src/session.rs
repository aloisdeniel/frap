// SPDX-License-Identifier: GPL-3.0-or-later
//! `Session`: executor (IR → calls), transactions and response building (ARCHITECTURE §11.6,
//! §12, §13). One statement is one transaction; errors, CANCELLED and caught panics restore
//! the snapshot, and pure reads run without one.

mod eval;
mod exec;
mod funcs;

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;

use frap_eval::EvalCache;
use frap_imbuf::DecodedImage;
use frap_lang::Stmt;
use frap_mesh::Mesh;
use frap_op::{ImageCache, OpError, PathResolver, Services};
use frap_render_scene::{GpuOptions, RenderError, RenderService};
use frap_rna::{
    FrapError, IdCode, IdKey, IdRef, ImageId, Input, MeshId, PyExc, Report, RnaError, RnaErrorKind,
    RnaPtr, Root, Span, Value,
};
use frap_scene::{
    ChangeLog, Context, EditState, Evaluator, Host, HostRead, Main, Preferences, Snapshot,
    WindowState,
};
use indexmap::IndexMap;
use serde_json::{Value as J, json};
use smol_str::SmolStr;

use crate::resolve::{Caps, id_repr};
use crate::response::{Response, Status};

pub(crate) use exec::{stmt_kind, stmt_op};

/// Everything a rollback restores (§12).
#[derive(Clone)]
pub(crate) struct State {
    main: Main,
    win: WindowState,
    edit: EditState,
    prefs: Preferences,
    time_dirty: bool,
    /// Datablocks an RNA edit tagged for an animation update since the last evaluation
    /// boundary (`ID_RECALC_ANIMATION`; `frap_eval::animate::tags_animation_on_write` /
    /// `tags_animation_on_call`, WP 6.4): the boundary re-applies their animation at the
    /// current frame even when time is not dirty.
    anim_tagged: Vec<Root>,
    /// `SessionOptions::cwd`, so that render snapshots (clones of the state) resolve relative
    /// image paths against the session's directory too (WP 5.5, `HostRead::cwd`).
    cwd: Option<PathBuf>,
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
        // §6.3: the cached edit-mode view while the mesh is in Edit Mode (WP 2.2).
        crate::edit::mesh_view(&self.main, &self.edit, mesh)
    }
    fn cwd(&self) -> Option<&std::path::Path> {
        self.cwd.as_deref()
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
    /// Created on first use by `render_factory` (and kept, also after a failure is retried).
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
    /// `SessionOptions::cwd`, for the relative paths of RNA functions (`HostRead::cwd`).
    cwd: PathBuf,
}

impl HostImpl {
    /// Tags the datablock `ptr` is rooted at for an animation update when the statement's
    /// RNA write of `prop` (`Some`) or call of the RNA function `func` (`None`: see
    /// `call`) is one Blender's update callback tags with `ID_RECALC_ANIMATION` (WP 6.4).
    pub(crate) fn tag_animation_write(&mut self, ptr: &RnaPtr, prop: &str) {
        if frap_eval::animate::tags_animation_on_write(ptr.srna, prop) {
            self.tag_animation(ptr.root);
        }
        // `rna_LayerCollection_exclude_update` tags every object of the view layer when the
        // layer collection ends up included.
        if prop == "exclude" && ptr.srna.is_a("LayerCollection") {
            let obs = frap_scene::id::layers::exclude_update_objects(&self.state.main, ptr);
            let tagged = &mut self.state.anim_tagged;
            let mut seen: std::collections::HashSet<Root> = tagged.iter().copied().collect();
            for ob in obs {
                if seen.insert(Root::Id(ob.any())) {
                    tagged.push(Root::Id(ob.any()));
                }
            }
        }
    }

    /// The same for a call of the RNA function `func` on `ptr`.
    pub(crate) fn tag_animation_call(&mut self, ptr: &RnaPtr, func: &str) {
        if frap_eval::animate::tags_animation_on_call(ptr.srna, func) {
            self.tag_animation(ptr.root);
        }
    }

    fn tag_animation(&mut self, root: Root) {
        if matches!(root, Root::Id(_) | Root::Embedded { .. })
            && !self.state.anim_tagged.contains(&root)
        {
            self.state.anim_tagged.push(root);
        }
    }
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
    fn cwd(&self) -> Option<&std::path::Path> {
        Some(&self.cwd)
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
        self.log.context_changed = true;
        &mut self.state.win
    }
    fn prefs_mut(&mut self) -> Option<&mut Preferences> {
        self.log.context_changed = true;
        Some(&mut self.state.prefs)
    }
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        (&self.state, &mut self.eval)
    }
    fn snapshot_at(&mut self, frame: Option<i32>) -> Result<Snapshot, RnaError> {
        // §7.3 / D-07 (WP 6.4): animation is applied on the clone at `frame` (the scene's
        // frame when `None`), never on the session's originals.
        let mut clone = HostImpl {
            state: self.state.clone(),
            eval: EvalCache::new(),
            log: ChangeLog::default(),
            reports: Vec::new(),
            cwd: self.cwd.clone(),
        };
        let scene = clone.state.win.scene;
        if let Some(f) = frame {
            let mut untracked = ChangeLog::default();
            if let Some(s) = clone.state.main.scenes.get_mut(scene, &mut untracked) {
                s.frame_current = f;
            }
        }
        let at = clone
            .state
            .main
            .scenes
            .get(scene)
            .map_or(0, |s| s.frame_current);
        frap_eval::animate::apply_animation_at(&mut clone, at as f32)?;
        clone.state.time_dirty = false;
        clone.state.anim_tagged.clear();
        Ok(Snapshot {
            host: Box::new(clone.state),
            eval: Box::new(EvalCache::new()),
        })
    }
    fn mark_time_dirty(&mut self) {
        self.state.time_dirty = true;
    }
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        // §7.3 (WP 6.4): the boundary writes the animation of the scene's frame (of every
        // animated datablock when time is dirty, else of those an RNA edit tagged).
        let dirty = self.state.time_dirty;
        let tagged = std::mem::take(&mut self.state.anim_tagged);
        frap_eval::animate::apply_pending_animation_tagged(self, dirty, &tagged)?;
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
        // WP 6.4: `BKE_animsys_write_to_rna_path` on the original. The path is resolved from
        // the (embedded) ID; the written value follows the property type (`channel_value`);
        // an unchanged value is not written (no generation bump).
        let not_found = || {
            RnaError::new(
                RnaErrorKind::NoAttribute,
                PyExc::AttributeError,
                format!("frap: animation path '{data_path}' does not resolve"),
            )
        };
        let segs = frap_eval::animate::parse_path(data_path).ok_or_else(not_found)?;
        let (ptr, prop, current) =
            frap_eval::animate::read_path(&*self, owner, &segs).ok_or_else(not_found)?;
        let value = match v {
            Value::Float(f) => f,
            Value::Double(d) => d as f32,
            Value::Int(i) => i as f32,
            Value::Bool(b) => f32::from(u8::from(b)),
            _ => return Err(not_found()),
        };
        let Some(new) = frap_eval::animate::channel_value(prop, &current, index, value) else {
            return Ok(());
        };
        crate::resolve::set_prop(self, &ptr, prop, new)
            .map_err(|e| RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, e.message))
    }
}

/// A frap session (one scene state, many statements).
pub struct Session {
    host: HostImpl,
    services: SessionServices,
    opts: SessionOptions,
    seq: u64,
    /// Nesting depth of [`Session::transact`] (an operator called with nested calls in its
    /// arguments runs its own transaction inside the statement's).
    txn_depth: u32,
}

/// A statement failure before it becomes a response: kind, operator, error, reports.
pub(crate) struct Fail {
    kind: &'static str,
    op: Option<SmolStr>,
    err: FrapError,
    reports: Vec<Report>,
}

impl Fail {
    pub(crate) fn new(kind: &'static str, err: FrapError) -> Fail {
        Fail {
            kind,
            op: None,
            err,
            reports: vec![],
        }
    }
}

/// Result of a transaction body: a value and whether the statement asks for a rollback
/// (CANCELLED).
pub(crate) struct Txn<R> {
    /// The body's result (`Err` = rolled back).
    pub(crate) result: Result<R, FrapError>,
    /// CANCELLED (rolled back).
    pub(crate) cancelled: bool,
    /// The statement's change log (empty after a rollback is still returned for reports).
    pub(crate) log: ChangeLog,
    /// Reports raised through `Host::report`.
    pub(crate) reports: Vec<Report>,
}

impl Session {
    /// Starts a session (factory startup by default, §5.7).
    pub fn new(opts: SessionOptions) -> Result<Session, FrapError> {
        frap_ops_all::link();
        let (main, win) = match &opts.startup {
            Startup::Factory => frap_scene::startup::factory(),
            // `startup::empty` is WP 1.10's; until it lands it panics, reported as INTERNAL.
            Startup::Empty => {
                catch_unwind(frap_scene::startup::empty).map_err(FrapError::internal_panic)?
            }
            // `--file`: the factory state, then `wm.open_mainfile` (WP 1.23) below.
            Startup::File(_) => frap_scene::startup::factory(),
        };
        let mut session = Session {
            host: HostImpl {
                state: State {
                    main,
                    win,
                    edit: EditState::default(),
                    prefs: Preferences::default(),
                    time_dirty: false,
                    anim_tagged: Vec::new(),
                    cwd: Some(opts.cwd.clone()),
                },
                eval: EvalCache::new(),
                log: ChangeLog::default(),
                reports: Vec::new(),
                cwd: opts.cwd.clone(),
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
            txn_depth: 0,
        };
        if let Startup::File(path) = &session.opts.startup {
            // §13.1/§14: `--file scene.frap` opens the file exactly as
            // `bpy.ops.wm.open_mainfile(filepath=…)` would (`bpy.data.filepath`, clean state).
            let filepath = eval::Py::Str(path.to_string_lossy().into_owned());
            session
                .invoke_values(
                    0,
                    "wm.open_mainfile",
                    vec![],
                    vec![("filepath".to_owned(), filepath, Span::default())],
                )
                .map_err(|f| f.err)?;
        }
        Ok(session)
    }

    /// The options this session was started with.
    pub fn options(&self) -> &SessionOptions {
        &self.opts
    }

    /// Read-only access to `bpy.data` (tests, schema export).
    pub fn main(&self) -> &Main {
        &self.host.state.main
    }

    /// Read-only access to the whole session state (`bpy.data`, window, edit state).
    pub fn host(&self) -> &dyn HostRead {
        &self.host
    }

    /// A textual dump of everything a rollback restores (`Main`, window state, edit state,
    /// preferences, pending time change). Two equal dumps mean equal states; tests use it to
    /// check that reads and failed statements leave the state bit-identical (§12).
    pub fn state_dump(&self) -> String {
        let s = &self.host.state;
        format!(
            "{:?}\n{:?}\n{:?}\n{:?}\n{} {:?}",
            s.main, s.win, s.edit, s.prefs.addons, s.time_dirty, s.anim_tagged
        )
    }

    /// True once the lazily created render service exists (§12: created on first use).
    pub fn render_service_started(&self) -> bool {
        self.services.render.is_some()
    }

    /// Runs a complete source unit; one response per statement.
    pub fn exec_source(&mut self, src: &str) -> Vec<Response> {
        self.exec_source_at(src, 1)
    }

    /// Like [`Session::exec_source`], with line numbers starting at `first_line`.
    pub fn exec_source_at(&mut self, src: &str, first_line: u32) -> Vec<Response> {
        let mut out = Vec::new();
        self.exec_source_each(src, first_line, |r| {
            out.push(r);
            std::ops::ControlFlow::Continue(())
        });
        out
    }

    /// Like [`Session::exec_source_at`], handing each response to `on` as soon as its
    /// statement has run (a front end writes and flushes it before the next statement
    /// starts); `ControlFlow::Break` stops before the next statement (`--fail-fast`).
    pub fn exec_source_each(
        &mut self,
        src: &str,
        first_line: u32,
        mut on: impl FnMut(Response) -> std::ops::ControlFlow<()>,
    ) {
        for r in frap_lang::parse_complete_at(src, first_line) {
            let resp = match r {
                Ok(stmt) => self.exec(&stmt),
                Err(e) => {
                    let line = e.span.map(|s| s.line).unwrap_or(first_line);
                    self.reject(line, e)
                }
            };
            if on(resp).is_break() {
                return;
            }
        }
    }

    /// A response for input that never became a statement (e.g. invalid UTF-8 on a line).
    pub fn reject(&mut self, line: u32, e: FrapError) -> Response {
        self.seq += 1;
        self.error_response(Fail::new("invalid", e), line)
    }

    /// Executes one parsed statement.
    pub fn exec(&mut self, stmt: &Stmt) -> Response {
        self.seq += 1;
        let _budget = frap_math::budget::scope(self.budget());
        let r = catch_unwind(AssertUnwindSafe(|| self.dispatch(stmt)))
            .unwrap_or_else(|p| Err(Fail::new(exec::stmt_kind(&stmt.cmd), self.unwind_error(p))));
        r.unwrap_or_else(|mut f| {
            f.err = f.err.with_span(stmt.span);
            self.error_response(f, stmt.line)
        })
    }

    /// Invokes an operator by idname with keyword inputs (front-end-free entry point, §11.6).
    pub fn invoke(&mut self, idname: &str, kwargs: IndexMap<String, Input>) -> Response {
        self.seq += 1;
        let _budget = frap_math::budget::scope(self.budget());
        let kw = kwargs
            .into_iter()
            .map(|(k, v)| (k, eval::Py::from_input(v), Span::default()))
            .collect();
        self.invoke_values(1, idname, vec![], kw)
            .unwrap_or_else(|f| self.error_response(f, 1))
    }

    /// Reads a property path (§11.6; the path uses the shared path grammar).
    pub fn get(&mut self, path: &str) -> Response {
        self.seq += 1;
        let _budget = frap_math::budget::scope(self.budget());
        match exec::parse_path(path) {
            Ok(p) => self
                .get_path(1, &p)
                .unwrap_or_else(|f| self.error_response(f, 1)),
            Err(e) => self.error_response(Fail::new("get", e), 1),
        }
    }

    /// Assigns a property path (§11.6).
    pub fn set(&mut self, path: &str, value: Input) -> Response {
        self.seq += 1;
        let _budget = frap_math::budget::scope(self.budget());
        match exec::parse_path(path) {
            Ok(p) => self
                .set_path(
                    1,
                    &p,
                    frap_lang::AssignOp::Assign,
                    exec::ValueSrc::Value(eval::Py::from_input(value)),
                )
                .unwrap_or_else(|f| self.error_response(f, 1)),
            Err(e) => self.error_response(Fail::new("set", e), 1),
        }
    }

    /// Calls an RNA function path (§11.6).
    pub fn call(
        &mut self,
        path: &str,
        args: Vec<Input>,
        kwargs: IndexMap<String, Input>,
    ) -> Response {
        self.seq += 1;
        let _budget = frap_math::budget::scope(self.budget());
        let mut call_args: Vec<funcs::CallArg> = args
            .into_iter()
            .map(|v| funcs::CallArg {
                name: None,
                value: eval::Py::from_input(v),
                span: Span::default(),
            })
            .collect();
        call_args.extend(kwargs.into_iter().map(|(k, v)| funcs::CallArg {
            name: Some(k.into()),
            value: eval::Py::from_input(v),
            span: Span::default(),
        }));
        match exec::parse_path(path) {
            Ok(p) => self
                .call_path(1, &p, exec::ArgsSrc::Values(call_args))
                .unwrap_or_else(|f| self.error_response(f, 1)),
            Err(e) => self.error_response(Fail::new("call", e), 1),
        }
    }

    // ------------------------------------------------------------------ helpers

    pub(crate) fn caps(&self) -> Caps {
        Caps {
            max_items: self.opts.max_items,
            max_array: self.opts.max_array,
        }
    }

    pub(crate) fn base(&self, kind: &'static str, line: u32, status: Status) -> Response {
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
            edit_mesh: if self.opts.verbosity == Verbosity::Minimal {
                None
            } else {
                crate::edit::edit_mesh_block(&self.host, self.opts.verbosity == Verbosity::Full)
            },
            hints: vec![],
        }
    }

    pub(crate) fn error_response(&self, f: Fail, line: u32) -> Response {
        let mut r = self.base(f.kind, line, Status::Error);
        r.op = f.op;
        r.reports = f.reports;
        let mut err = f.err;
        // Operator pre-checks (`OpError::Limit`) carry only a message: add the limit and the
        // hint the post-statement check gives.
        if err.code == frap_op::errors::LIMIT_EXCEEDED && err.hint.is_none() {
            err = err
                .with_detail("max_elements", self.opts.limits.max_elements)
                .with_hint("raise --max-elements, or build less geometry per statement");
        }
        r.error = Some(err);
        r
    }

    /// A `SKIPPED` response (serve/MCP `on_error: "skip_rest"`); the statement is not run.
    pub(crate) fn skipped(
        &mut self,
        kind: &'static str,
        op: Option<SmolStr>,
        line: u32,
    ) -> Response {
        self.seq += 1;
        let mut r = self.base(kind, line, Status::Skipped);
        r.op = op;
        r
    }

    fn context_json(&self) -> J {
        let main = &self.host.state.main;
        let ctx = self.host.state.ctx();
        let name = |r| main.objects.name_of(r).map(J::from).unwrap_or(J::Null);
        let mode = ctx.mode();
        let mut j = json!({
            "mode": mode.as_str(),
            "active_object": ctx.active_object().map(name).unwrap_or(J::Null),
            "selected_objects": ctx.selected_objects().into_iter().map(name).collect::<Vec<_>>(),
            "frame_current": ctx.scene().frame_current,
        });
        // `bpy.context.edit_object` is set in the edit modes only (not in Pose Mode); the
        // mesh select mode only matters in mesh Edit Mode.
        let edit = matches!(
            mode,
            frap_scene::ContextMode::EditMesh | frap_scene::ContextMode::EditArmature
        );
        if edit && let J::Object(m) = &mut j {
            m.insert(
                "edit_object".into(),
                ctx.active_object().map(name).unwrap_or(J::Null),
            );
        }
        if self.opts.verbosity == Verbosity::Full
            && let J::Object(m) = &mut j
            && let Some(ob) = ctx.active_object()
        {
            m.extend(self.evaluated_json(ob));
        }
        if mode == frap_scene::ContextMode::EditMesh
            && let J::Object(m) = &mut j
        {
            let sm = ctx.tool_settings().mesh_select_mode;
            m.insert(
                "select_mode".into(),
                J::Array(
                    ["VERT", "EDGE", "FACE"]
                        .iter()
                        .zip(sm)
                        .filter(|(_, on)| *on)
                        .map(|(n, _)| J::from(*n))
                        .collect(),
                ),
            );
        }
        j
    }

    /// `--verbosity full` (§13.2): the active object's evaluated element counts
    /// (`evaluated: {vertices, edges, faces}`, viewport) and, when it has modifiers,
    /// `modifier_status`: `[{name, status, message?}]` with status `EVALUATED`, `SKIPPED`
    /// (hidden in the viewport), `PASSED_THROUGH` (no frap evaluator, WARNING), `DISABLED`
    /// (Blender's `is_disabled`, e.g. a Boolean without operand) or `ERROR` (the modifier
    /// error). Nothing when the object cannot be evaluated.
    fn evaluated_json(
        &self,
        ob: frap_rna::IdRef<frap_rna::ObjectId>,
    ) -> serde_json::Map<String, J> {
        use frap_scene::ModifierStatus as S;
        let mut m = serde_json::Map::new();
        let modifier_names = |s: &Self| -> Vec<SmolStr> {
            s.host
                .state
                .main
                .objects
                .get(ob)
                .map(|o| o.modifiers.iter().map(|md| md.name.clone()).collect())
                .unwrap_or_default()
        };
        // The context is built after the statement, still under its budget: an evaluation
        // that runs past `--stmt-timeout` (or any other unwind: a limit an evaluator checks
        // outside the stack) must not escape `Session::exec` (the statement itself already
        // succeeded). `evaluated` is then omitted and every modifier reports the error.
        let eo = match catch_unwind(AssertUnwindSafe(|| {
            crate::resolve::evaluated_for_read(&self.host, ob)
        })) {
            Ok(Ok(eo)) => eo,
            Ok(Err(_)) => return m,
            Err(payload) => {
                let err = self.unwind_error(payload);
                let list: Vec<J> = modifier_names(self)
                    .iter()
                    .map(|n| {
                        let msg = err.message.strip_prefix("Error: ").unwrap_or(&err.message);
                        json!({"name": n.as_str(), "status": "ERROR", "message": msg})
                    })
                    .collect();
                if !list.is_empty() {
                    m.insert("modifier_status".into(), J::Array(list));
                }
                return m;
            }
        };
        if let Some(mesh) = &eo.mesh {
            m.insert(
                "evaluated".into(),
                json!({
                    "vertices": mesh.verts_num(),
                    "edges": mesh.edges_num(),
                    "faces": mesh.faces_num(),
                }),
            );
        }
        let has_modifiers = self
            .host
            .state
            .main
            .objects
            .get(ob)
            .is_some_and(|o| !o.modifiers.is_empty());
        if has_modifiers {
            let list: Vec<J> = eo
                .modifier_status
                .iter()
                .map(|(name, st)| {
                    let (status, message) = match st {
                        S::Evaluated => ("EVALUATED", None),
                        S::Skipped => ("SKIPPED", None),
                        S::PassedThrough => ("PASSED_THROUGH", None),
                        S::Disabled(w) => ("DISABLED", Some(w.as_str())),
                        S::Error(w) => ("ERROR", Some(w.as_str())),
                        _ => ("UNKNOWN", None),
                    };
                    let mut e = json!({"name": name.as_str(), "status": status});
                    if let (Some(msg), J::Object(o)) = (message, &mut e) {
                        o.insert("message".into(), J::from(msg));
                    }
                    e
                })
                .collect();
            m.insert("modifier_status".into(), J::Array(list));
        }
        m
    }

    pub(crate) fn changes_json(&self, log: &ChangeLog) -> J {
        let main = &self.host.state.main;
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
        let added_j: Vec<J> = added.iter().map(|a| J::from(id_repr(main, *a))).collect();
        let modified: Vec<J> = log
            .modified
            .iter()
            .filter(|m| !log.added.contains(m) && main.header(**m).is_some())
            .map(|m| J::from(id_repr(main, *m)))
            .collect();
        let removed: Vec<J> = log
            .removed
            .iter()
            .map(|(a, n)| J::from(frap_rna::ptr::id_repr(a.code, n, None)))
            .collect();
        let renamed: Vec<J> = log
            .renamed
            .iter()
            .map(|(a, old)| {
                json!({"from": frap_rna::ptr::id_repr(a.code, old, None), "to": id_repr(main, *a)})
            })
            .collect();
        json!({"added": added_j, "removed": removed, "modified": modified, "renamed": renamed})
    }

    /// Runs `f` as one transaction (§12): snapshot, run (panics caught), and restore the
    /// snapshot on error, on CANCELLED (`Ok((_, true))`) or when a limit is exceeded.
    ///
    /// Re-entrant: a nested transaction keeps the change log and reports gathered so far by
    /// the enclosing one (nested calls in an operator's arguments), so its limits, `changes`
    /// and reports cover the whole statement; the enclosing transaction still restores its
    /// own snapshot when the statement fails.
    pub(crate) fn transact<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<(R, bool), FrapError>,
    ) -> Txn<R> {
        let _budget = frap_math::budget::scope(self.budget());
        let snapshot = self.host.state.clone();
        if self.txn_depth == 0 {
            self.host.log = ChangeLog::default();
            self.host.reports.clear();
        }
        self.txn_depth += 1;
        let r = catch_unwind(AssertUnwindSafe(|| f(self)))
            .unwrap_or_else(|p| Err(self.unwind_error(p)))
            .and_then(|(v, cancelled)| {
                if !cancelled {
                    self.check_limits()?;
                    // A real deadline: work that finished after it still fails (and rolls
                    // back), whether or not a loop reached a cancellation point in time.
                    if frap_math::budget::expired() {
                        return Err(self.timeout_error());
                    }
                }
                Ok((v, cancelled))
            });
        self.txn_depth -= 1;
        let log = std::mem::take(&mut self.host.log);
        let reports = std::mem::take(&mut self.host.reports);
        let (result, cancelled) = match r {
            Ok((v, false)) => (Ok(v), false),
            Ok((v, true)) => {
                self.host.state = snapshot;
                (Ok(v), true)
            }
            Err(e) => {
                self.host.state = snapshot;
                (Err(e), false)
            }
        };
        Txn {
            result,
            cancelled,
            log,
            reports,
        }
    }

    /// The statement budget of `--stmt-timeout` / `--max-elements` (§12, `frap_math::budget`).
    fn budget(&self) -> frap_math::budget::Budget {
        frap_math::budget::Budget {
            timeout: self.opts.limits.stmt_timeout,
            max_elements: self.opts.limits.max_elements,
        }
    }

    /// The error of a caught unwind: `TIMEOUT` for an expired `--stmt-timeout` (a cooperative
    /// cancellation point unwound), `INTERNAL` for any other panic.
    fn unwind_error(&self, payload: Box<dyn std::any::Any + Send>) -> FrapError {
        if frap_math::budget::interrupted(&*payload).is_some() {
            self.timeout_error()
        } else if let Some(e) = frap_math::budget::elements_exceeded(&*payload) {
            // A growing BMesh hit `--max-elements` (`budget::check_corners`), or a buffer was
            // too large (`budget::check_buffer`: image pixels).
            let err = FrapError::new(
                PyExc::RuntimeError,
                frap_op::errors::LIMIT_EXCEEDED,
                format!("Error: {e}"),
            )
            .with_detail("max_elements", self.opts.limits.max_elements);
            if e.buffer.is_some() {
                err.with_detail("bytes", e.corners)
                    .with_hint("raise --max-elements, or use a smaller image")
            } else {
                err.with_detail("corners", e.corners)
                    .with_hint("raise --max-elements, or build less geometry per statement")
            }
        } else {
            FrapError::internal_panic(payload)
        }
    }

    /// `RuntimeError` code `TIMEOUT` (§12, §13.4).
    fn timeout_error(&self) -> FrapError {
        let ms = self
            .opts
            .limits
            .stmt_timeout
            .map_or(0, |d| d.as_millis().min(u64::MAX as u128) as u64);
        FrapError::new(
            PyExc::RuntimeError,
            crate::errors::TIMEOUT,
            "Error: frap: limit exceeded (stmt_timeout)",
        )
        .with_detail("stmt_timeout_ms", ms)
        .with_hint(
            "the statement was rolled back; raise --stmt-timeout, or split the work into smaller statements",
        )
    }

    /// `--max-elements`: corners of the meshes the statement created or modified. A mesh in
    /// Edit Mode counts the loops of its edit BMesh (its `Main` geometry is stale until the
    /// mesh leaves Edit Mode), so the statement that grows it fails, not the later `mode_set`.
    fn check_limits(&self) -> Result<(), FrapError> {
        let main = &self.host.state.main;
        let edit = &self.host.state.edit;
        let log = &self.host.log;
        // A mesh both added and modified by the statement counts once.
        let meshes: std::collections::BTreeSet<IdRef<MeshId>> = log
            .added
            .iter()
            .chain(log.modified.iter())
            .filter_map(|id| id.typed::<MeshId>())
            .collect();
        let corners: u64 = meshes
            .into_iter()
            .filter_map(|m| match edit.meshes.get(&m.key()) {
                Some(em) => Some(em.bm.loops_num() as u64),
                None => main.meshes.get(m).map(|me| me.corners_num() as u64),
            })
            .sum();
        if corners > self.opts.limits.max_elements {
            return Err(FrapError::new(
                PyExc::RuntimeError,
                frap_op::errors::LIMIT_EXCEEDED,
                "Error: frap: limit exceeded (max_elements)",
            )
            .with_detail("max_elements", self.opts.limits.max_elements)
            .with_detail("corners", corners)
            .with_hint("raise --max-elements, or build less geometry per statement"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::ObjectMode;

    fn with_mode(mode: ObjectMode) -> Session {
        let mut s = Session::new(SessionOptions::default()).expect("factory session");
        let ob = s.host.state.ctx().active_object().expect("active Cube");
        let (main, log) = (&mut s.host.state.main, &mut s.host.log);
        main.objects.get_mut(ob, log).expect("Cube").mode = mode;
        s
    }

    #[test]
    fn edit_object_and_select_mode_only_in_edit_modes() {
        let s = with_mode(ObjectMode::Edit);
        let c = s.context_json();
        assert_eq!(c["mode"], "EDIT_MESH");
        assert_eq!(c["edit_object"], "Cube");
        assert_eq!(c["select_mode"], json!(["VERT"]));
        let mut s = s;
        assert_eq!(
            s.get("bpy.context.edit_object").value,
            Some(json!({"$ref": "bpy.data.objects['Cube']"}))
        );
        assert_eq!(
            s.get("bpy.context.objects_in_mode").value,
            Some(json!([{"$ref": "bpy.data.objects['Cube']"}]))
        );

        let mut s = with_mode(ObjectMode::Pose);
        let c = s.context_json();
        assert_eq!(c["mode"], "POSE");
        assert!(c.get("edit_object").is_none(), "{c}");
        assert!(c.get("select_mode").is_none(), "{c}");
        assert_eq!(s.get("bpy.context.edit_object").value, Some(J::Null));

        let s = with_mode(ObjectMode::Object);
        let c = s.context_json();
        assert!(c.get("edit_object").is_none(), "{c}");
    }
}
