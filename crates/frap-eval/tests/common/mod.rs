// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal host for evaluator tests: the factory state plus helpers to add objects and
//! meshes, and a `Host` implementation that records animation writes and reports.
#![allow(dead_code)] // each test binary uses a subset

use std::sync::Arc;

use frap_eval::EvalCache;
use frap_mesh::Mesh;
use frap_rna::{IdAny, IdRef, MeshId, ObjectId, Report, RnaError, Root, Value};
use frap_scene::id::Object;
use frap_scene::{
    ChangeLog, Context, EditState, Evaluator, Host, HostRead, Main, Preferences, Snapshot,
    WindowState,
};

pub struct TestHost {
    pub main: Main,
    pub win: WindowState,
    pub edit: EditState,
    pub prefs: Preferences,
    pub log: ChangeLog,
    pub eval: EvalCache,
    /// `set_path` calls (animation writes).
    pub set_paths: usize,
    /// Reports received.
    pub reports: Vec<Report>,
    /// When set, `mesh_view` of a mesh in `edit` returns this mesh (stands in for the edit
    /// BMesh view of §6.3).
    pub edit_view: Option<Arc<Mesh>>,
}

impl TestHost {
    pub fn factory() -> Self {
        let (main, win) = frap_scene::startup::factory();
        TestHost {
            main,
            win,
            edit: EditState::default(),
            prefs: Preferences::default(),
            log: ChangeLog::default(),
            eval: EvalCache::new(),
            set_paths: 0,
            reports: Vec::new(),
            edit_view: None,
        }
    }

    pub fn add_mesh(
        &mut self,
        name: &str,
        verts: &[[f32; 3]],
        faces: &[Vec<i32>],
    ) -> IdRef<MeshId> {
        let mut me = Mesh::new(name);
        me.from_pydata(verts, &[], faces, false);
        self.main.meshes.insert(name, me, &mut self.log)
    }

    pub fn add_object(&mut self, name: &str, data: Option<IdAny>) -> IdRef<ObjectId> {
        self.main
            .objects
            .insert(name, Object::new(name, data), &mut self.log)
    }

    pub fn obj(&self, name: &str) -> IdRef<ObjectId> {
        self.main.objects.find(name).expect("object exists")
    }

    /// Writes through `IdMap::get_mut` (generation bump).
    pub fn edit_obj(&mut self, ob: IdRef<ObjectId>, f: impl FnOnce(&mut Object)) {
        f(self
            .main
            .objects
            .get_mut(ob, &mut self.log)
            .expect("object exists"));
    }

    /// Every ID generation of `bpy.data` objects and meshes, in order.
    pub fn generations(&self) -> Vec<u64> {
        let mut g: Vec<u64> = self
            .main
            .objects
            .iter()
            .map(|(_, o)| o.header.generation)
            .collect();
        g.extend(self.main.meshes.iter().map(|(_, m)| m.header.generation));
        g.extend(self.main.scenes.iter().map(|(_, s)| s.header.generation));
        g
    }
}

impl HostRead for TestHost {
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
        if self.edit.meshes.contains_key(&mesh.key())
            && let Some(v) = &self.edit_view
        {
            return v.clone();
        }
        self.main.meshes.get_arc(mesh).expect("mesh exists")
    }
}

impl Host for TestHost {
    fn main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
        (&mut self.main, &mut self.log)
    }
    fn edit_mut(&mut self) -> &mut EditState {
        &mut self.edit
    }
    fn split_edit(&mut self) -> (Context<'_>, &mut EditState) {
        (
            Context {
                main: &self.main,
                win: &self.win,
                prefs: &self.prefs,
            },
            &mut self.edit,
        )
    }
    fn win_mut(&mut self) -> &mut WindowState {
        &mut self.win
    }
    fn eval(&mut self) -> (&dyn HostRead, &mut dyn Evaluator) {
        panic!("not used by these tests")
    }
    fn snapshot_at(&mut self, _frame: Option<i32>) -> Result<Snapshot, RnaError> {
        panic!("not used by these tests")
    }
    fn mark_time_dirty(&mut self) {}
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        Ok(())
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn set_path(
        &mut self,
        _owner: Root,
        _path: &str,
        _index: i32,
        _v: Value,
    ) -> Result<(), RnaError> {
        self.set_paths += 1;
        Ok(())
    }
}
