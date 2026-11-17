// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-session` (§12): `Host` + `Services` implementations, a path resolver built
//! only on the §9.3/§9.4 traits (plus the RFC 0001 amendments), transactions, and the
//! operator invocation path through `OpCtx`.
//!
//! Layout note (RFC 0001 F-SES-1): §9.5 says `Host` is "implemented by frap-session::Session",
//! but `OpCtx::new` needs `&mut dyn Host` and `&mut dyn Services` at the same time, so the
//! `Host` must be a field of `Session` disjoint from the services (here `HostImpl`).
// RFC 0001 F-LINK-1: linkme registration is incompatible with `forbid(unsafe_code)`.
#![deny(unsafe_code)]

pub mod ops;
pub mod path;
pub mod resolve;

use std::sync::Arc;

use frap_probe_mesh::Mesh;
use frap_probe_op::{ImageCache, OpError, PathResolver, Services};
use frap_probe_render_scene::{GpuOptions, RenderError, RenderService};
use frap_probe_rna::*;
use frap_probe_scene::*;

/// Everything rolled back by a transaction (§12).
#[derive(Clone)]
pub struct State {
    pub main: Main,
    pub win: WindowState,
    pub edit: EditState,
    pub prefs: Preferences,
    pub time_dirty: bool,
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
        self.main.meshes.get_arc(mesh).expect("invariant: mesh exists")
    }
}

/// Depsgraph-lite stand-in: translation-only world matrices.
#[derive(Default)]
pub struct EvalCache {
    pub evaluations: usize,
}

impl Evaluator for EvalCache {
    fn object(&mut self, host: &dyn HostRead, ob: IdRef<ObjectId>, _mode: EvalMode) -> Result<Arc<EvaluatedObject>, EvalError> {
        self.evaluations += 1;
        let o = host.main().objects.get(ob).ok_or_else(|| EvalError("no such object".into()))?;
        let [x, y, z] = o.location;
        let mesh = o.data.and_then(IdRef::<MeshId>::from_any).map(|m| host.mesh_view(m));
        Ok(Arc::new(EvaluatedObject {
            matrix_world: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [x, y, z, 1.0]],
            mesh,
            bounds: None,
            warnings: vec![],
        }))
    }
    fn depsgraph_update(&mut self, _host: &dyn HostRead) {}
}

/// The `Host` (F-SES-1: a field of `Session`, disjoint from `services`).
pub struct HostImpl {
    pub state: State,
    pub eval: EvalCache,
    pub log: ChangeLog,
    pub reports: Vec<Report>,
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
    fn win_mut(&mut self) -> &mut WindowState {
        &mut self.state.win
    }
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        (&self.state, &mut self.eval)
    }
    fn mark_time_dirty(&mut self) {
        self.state.time_dirty = true;
    }
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        self.state.time_dirty = false;
        Ok(())
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn set_path(&mut self, _owner: Root, _data_path: &str, _index: i32, _v: Value) -> Result<(), RnaError> {
        Err(RnaError::new(PyExc::NotImplementedError, codes::NOT_IMPLEMENTED, "probe: set_path"))
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
    fn snapshot_at(&mut self, _frame: Option<i32>) -> Result<Snapshot, RnaError> {
        // COW: O(#IDs); animation would be applied to the clone here (§7.3).
        Ok(Snapshot {
            host: Box::new(self.state.clone()),
            eval: Box::new(EvalCache::default()),
        })
    }
}

pub struct NoImages;
impl ImageCache for NoImages {
    fn len(&self) -> usize {
        0
    }
}

pub type RenderFactory = fn(&GpuOptions) -> Result<Box<dyn RenderService>, RenderError>;

/// §12 `SessionServices`: the render service is created lazily on first use.
pub struct SessionServices {
    pub render: Option<Box<dyn RenderService>>,
    pub render_factory: RenderFactory,
    pub factory_calls: usize,
    pub images: NoImages,
    pub paths: PathResolver,
}

impl Services for SessionServices {
    fn render(&mut self) -> Result<&mut dyn RenderService, OpError> {
        if self.render.is_none() {
            self.factory_calls += 1;
            let svc = (self.render_factory)(&GpuOptions::from_env()).map_err(|e| OpError::Gpu(format!("GPU_UNAVAILABLE: {e:?}")))?;
            self.render = Some(svc);
        }
        // `&mut (dyn RenderService + 'static)` -> `&mut (dyn RenderService + '_)`: allowed.
        Ok(self.render.as_deref_mut().expect("invariant: just created"))
    }
    fn images(&mut self) -> &mut dyn ImageCache {
        &mut self.images
    }
    fn paths(&self) -> &PathResolver {
        &self.paths
    }
}

/// Factory used when the `gpu` feature is off (§3.6).
pub fn no_gpu(_: &GpuOptions) -> Result<Box<dyn RenderService>, RenderError> {
    Err(RenderError::NoAdapter("built without gpu".into()))
}

pub fn default_factory() -> RenderFactory {
    #[cfg(feature = "gpu")]
    {
        frap_probe_render::new_service
    }
    #[cfg(not(feature = "gpu"))]
    {
        no_gpu
    }
}

pub struct Session {
    pub host: HostImpl,
    pub services: SessionServices,
}

/// Names of the factory-like probe scene.
pub struct Factory {
    pub cube: IdRef<ObjectId>,
    pub cube_mesh: IdRef<MeshId>,
    pub material: IdRef<MaterialId>,
    pub scene: IdRef<SceneId>,
}

impl Session {
    /// A factory-like scene: Scene, Cube (mesh 'Cube' with material 'Material'), active+selected.
    pub fn factory_with(render_factory: RenderFactory) -> (Session, Factory) {
        let mut log = ChangeLog::default();
        let mut main = Main::default();
        let material = main.materials.insert("Material", Material::new("Material"), &mut log);
        let mut me = frap_probe_mesh::test_cube("Cube");
        me.materials.push(Some(material));
        let cube_mesh = main.meshes.insert("Cube", me, &mut log);
        let cube = main.objects.insert("Cube", Object::new("Cube", Some(cube_mesh.any())), &mut log);
        let mut sc = Scene::new("Scene");
        sc.view_layers[0].bases.push(Base {
            object: cube,
            selected: true,
        });
        sc.view_layers[0].active = Some(cube);
        let scene = main.scenes.insert("Scene", sc, &mut log);
        let state = State {
            main,
            win: WindowState {
                scene,
                view_layer: "ViewLayer".into(),
            },
            edit: EditState::default(),
            prefs: Preferences::default(),
            time_dirty: false,
        };
        let s = Session {
            host: HostImpl {
                state,
                eval: EvalCache::default(),
                log: ChangeLog::default(),
                reports: vec![],
            },
            services: SessionServices {
                render: None,
                render_factory,
                factory_calls: 0,
                images: NoImages,
                paths: PathResolver::default(),
            },
        };
        (
            s,
            Factory {
                cube,
                cube_mesh,
                material,
                scene,
            },
        )
    }
    pub fn factory() -> (Session, Factory) {
        Self::factory_with(default_factory())
    }

    /// One statement = one transaction (§12): restore the snapshot on error.
    pub fn transaction<R>(&mut self, f: impl FnOnce(&mut Session) -> Result<R, FrapError>) -> Result<R, FrapError> {
        let snapshot = self.host.state.clone();
        self.host.log = ChangeLog::default();
        let r = f(self);
        if r.is_err() {
            self.host.state = snapshot;
        }
        r
    }
}
