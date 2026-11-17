// SPDX-License-Identifier: GPL-3.0-or-later
//! A minimal `Host` for frap-scene's own tests (the real one is `frap_session::HostImpl`) and
//! a tiny resolver that reads and writes through bindings, hooks and RNA functions the way
//! the session does (§9.3, §9.5).
#![allow(dead_code)]

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{
    BitSet, CoerceCx, Field, IdAny, IdRef, Input, Item, MeshId, ObjectId, Report, RnaError, RnaPtr,
    Root, Seg, SetOutcome, StructDef, Value, coerce,
};
use frap_scene::{
    ChangeLog, Context, EditState, Evaluator, FuncArgs, Host, HostRead, Main, Preferences,
    Snapshot, WindowState, find_func, find_hook,
};
use indexmap::IndexMap;

pub struct TestHost {
    pub main: Main,
    pub win: WindowState,
    pub edit: EditState,
    pub prefs: Preferences,
    pub log: ChangeLog,
    pub time_dirty: bool,
    pub reports: Vec<Report>,
}

impl TestHost {
    pub fn new((main, win): (Main, WindowState)) -> TestHost {
        TestHost {
            main,
            win,
            edit: EditState::default(),
            prefs: Preferences::default(),
            log: ChangeLog::default(),
            time_dirty: false,
            reports: Vec::new(),
        }
    }
    pub fn factory() -> TestHost {
        TestHost::new(frap_scene::startup::factory())
    }
    pub fn ob(&self, name: &str) -> IdRef<ObjectId> {
        self.main.objects.find(name).expect("object exists")
    }
    pub fn ob_ptr(&self, name: &str) -> RnaPtr {
        RnaPtr::id(self.ob(name).any(), &frap_spec_rna::OBJECT)
    }
    pub fn take_log(&mut self) -> ChangeLog {
        std::mem::take(&mut self.log)
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
        unimplemented!("frap-scene tests never evaluate through the host")
    }
    fn snapshot_at(&mut self, _frame: Option<i32>) -> Result<Snapshot, RnaError> {
        unimplemented!("frap-scene tests never snapshot")
    }
    fn mark_time_dirty(&mut self) {
        self.time_dirty = true;
    }
    fn apply_pending_animation(&mut self) -> Result<(), RnaError> {
        Ok(())
    }
    fn report(&mut self, r: Report) {
        self.reports.push(r);
    }
    fn set_path(
        &mut self,
        _owner: Root,
        _data_path: &str,
        _index: i32,
        _v: Value,
    ) -> Result<(), RnaError> {
        unimplemented!("frap-scene tests never animate")
    }
}

/// Reads `prop` of the struct at `ptr` (owned by `h`), through the binding or its get hook.
pub fn read(h: &TestHost, ptr: &RnaPtr, prop: &str) -> Result<Value, RnaError> {
    let def = ptr
        .srna
        .find_prop(prop)
        .unwrap_or_else(|| panic!("{}.{prop} is not an RNA property", ptr.srna.identifier));
    // Collection API structs (`view_layer.objects.active`) are served by hooks only.
    if ptr.srna.identifier == "LayerObjects" {
        let hook = find_hook(ptr.srna, prop)
            .and_then(|h| h.get)
            .expect("get hook");
        return Ok(field_value(hook(h, ptr, def)?));
    }
    with_struct(h, ptr, &mut |s| match s.get(def)? {
        Field::Value(v) => Ok(v),
        Field::Id(a) => Ok(Value::Id(a)),
        Field::NeedsHook => {
            let Some(hook) = find_hook(s.srna(), prop).and_then(|h| h.get) else {
                return Err(RnaError::new(
                    frap_rna::RnaErrorKind::NotImplemented,
                    frap_rna::PyExc::NotImplementedError,
                    format!("no get hook for {}.{prop}", s.srna().identifier),
                ));
            };
            Ok(field_value(hook(h, ptr, def)?))
        }
        Field::Ptr(p) => Ok(Value::Ptr(Box::new(p))),
        Field::Embedded(slot, r) => match ptr.root {
            Root::Id(owner) => Ok(Value::Ptr(Box::new(RnaPtr::new(
                Root::Embedded { owner, slot },
                r.srna(),
            )))),
            _ => Ok(Value::None),
        },
        other => Ok(field_value(other)),
    })
}

/// IDs of a collection-valued field (`Field::Coll` of IDs), names resolved.
pub fn field_value(f: Field<'_>) -> Value {
    match f {
        Field::Value(v) => v,
        Field::Id(a) => Value::Id(a),
        Field::Ptr(p) => Value::Ptr(Box::new(p)),
        Field::Coll(c) => Value::Tuple(
            (0..c.len())
                .map(|i| match c.item(i) {
                    Ok(Item::Id(a)) => Value::Id(a),
                    Ok(Item::Struct(_)) => Value::Str(c.key_of(i).unwrap_or_default()),
                    Err(_) => Value::None,
                })
                .collect(),
        ),
        _ => Value::None,
    }
}

/// Calls `f` on the struct a pointer addresses (IDs, embedded IDs and `Prop`/`Key`/`Index`
/// paths through bindings and get hooks).
pub fn with_struct<R>(
    h: &TestHost,
    ptr: &RnaPtr,
    f: &mut dyn FnMut(&dyn frap_rna::Rna) -> Result<R, RnaError>,
) -> Result<R, RnaError> {
    let root: &dyn frap_rna::Rna = match ptr.root {
        Root::Id(a) => h.main.id(a).expect("ID exists"),
        Root::Embedded { owner, slot } => h.main.embedded(owner, slot).expect("embedded exists"),
        Root::Context => {
            let ctx = h.ctx();
            return walk(h, &ctx, ptr, 0, f);
        }
        _ => panic!("bpy.data roots are not structs"),
    };
    walk(h, root, ptr, 0, f)
}

fn walk<R>(
    h: &TestHost,
    s: &dyn frap_rna::Rna,
    ptr: &RnaPtr,
    i: usize,
    f: &mut dyn FnMut(&dyn frap_rna::Rna) -> Result<R, RnaError>,
) -> Result<R, RnaError> {
    if i >= ptr.path.len() {
        return f(s);
    }
    let Seg::Prop(p) = &ptr.path[i] else {
        panic!("path segment {i} of {:?} is not a property", ptr.path);
    };
    let def = s.srna().find_prop(p).expect("property exists");
    // `this` of a get hook is the struct owning the property.
    let mut sub = ptr.clone();
    sub.path.truncate(i);
    sub.srna = s.srna();
    let field = match s.get(def)? {
        Field::NeedsHook => {
            let Some(hook) = find_hook(s.srna(), p).and_then(|h| h.get) else {
                return Err(RnaError::new(
                    frap_rna::RnaErrorKind::NotImplemented,
                    frap_rna::PyExc::NotImplementedError,
                    format!("no get hook for {}.{p}", s.srna().identifier),
                ));
            };
            hook(h, &sub, def)?
        }
        other => other,
    };
    match field {
        Field::Struct(r) | Field::Embedded(_, r) => walk(h, &*r, ptr, i + 1, f),
        Field::Coll(c) => {
            let key = ptr.path.get(i + 1).expect("collection item segment");
            let idx = match key {
                Seg::Key(k) => c.find(k).expect("key found"),
                Seg::Index(n) => *n as usize,
                Seg::Prop(_) => panic!("collection API structs are not walked here"),
            };
            match c.item(idx)? {
                Item::Struct(r) => walk(h, &*r, ptr, i + 2, f),
                Item::Id(_) => panic!("walk into ID items: re-root instead"),
            }
        }
        _ => Err(RnaError::new(
            frap_rna::RnaErrorKind::NotImplemented,
            frap_rna::PyExc::NotImplementedError,
            format!("cannot walk into {p}"),
        )),
    }
}

/// Writes `prop` at `ptr` with Blender's assignment path: coerce, `RnaMut::set` on the owning
/// ID (through `Main::id_mut`), or the set hook.
pub fn write(h: &mut TestHost, ptr: &RnaPtr, prop: &str, input: Input) -> Result<(), RnaError> {
    let def = ptr.srna.find_prop(prop).expect("property exists");
    let v = coerce(
        def,
        input,
        &CoerceCx::RnaSet {
            owner: ptr.srna.identifier,
        },
    )?
    .value;
    let direct = match (ptr.root, ptr.path.is_empty()) {
        (Root::Id(a), true) => {
            let (main, log) = h.main_mut();
            Some(
                main.id_mut(a, log)
                    .expect("ID exists")
                    .set(def, v.clone())?,
            )
        }
        (Root::Id(a), false) => {
            let (main, log) = h.main_mut();
            let id = main.id_mut(a, log).expect("ID exists");
            set_nested(id, ptr, 0, def, v.clone())?
        }
        _ => None,
    };
    if direct == Some(SetOutcome::Done) {
        return Ok(());
    }
    let hook = find_hook(ptr.srna, prop)
        .and_then(|h| h.set)
        .unwrap_or_else(|| panic!("no set hook for {}.{prop}", ptr.srna.identifier));
    hook(h, ptr, def, v)
}

fn set_nested(
    s: &mut dyn frap_rna::RnaMut,
    ptr: &RnaPtr,
    i: usize,
    def: &'static frap_rna::PropDef,
    v: Value,
) -> Result<Option<SetOutcome>, RnaError> {
    if i >= ptr.path.len() {
        return s.set(def, v).map(Some);
    }
    let Seg::Prop(p) = &ptr.path[i] else {
        return Ok(None);
    };
    let pd = s.srna().find_prop(p).expect("property exists");
    match s.struct_mut(pd) {
        Ok(mut g) => set_nested(&mut *g, ptr, i + 1, def, v),
        Err(_) => Ok(None), // a hook-served struct: the set hook handles it
    }
}

/// Calls an RNA function with keyword arguments (already typed values).
pub fn call(
    h: &mut TestHost,
    ptr: &RnaPtr,
    func: &str,
    kwargs: &[(&'static str, Value)],
) -> Result<Value, RnaError> {
    let def = ptr.srna.find_func(func).expect("function exists");
    let imp = find_func(ptr.srna, func).expect("function registered").imp;
    let mut values = IndexMap::new();
    let mut is_set = BitSet::new();
    for (i, p) in def.params.iter().enumerate() {
        if let Some((_, v)) = kwargs.iter().find(|(k, _)| *k == p.identifier) {
            values.insert(p.identifier, v.clone());
            is_set.insert(i);
        } else {
            values.insert(p.identifier, Value::None);
        }
    }
    let mut args = FuncArgs {
        func: def,
        values,
        is_set,
    };
    imp(h, ptr, &mut args)
}

/// Names of an ID-valued tuple/list.
pub fn names(h: &TestHost, v: &Value) -> Vec<String> {
    let Value::Tuple(items) = v else {
        panic!("not a list: {v:?}")
    };
    items
        .iter()
        .map(|x| match x {
            Value::Id(Some(a)) => h.main.header(*a).expect("ID exists").name.to_string(),
            Value::Ptr(p) => p.repr(&h.main),
            other => format!("{other:?}"),
        })
        .collect()
}

/// The ID of a `Value::Id`.
pub fn id(v: &Value) -> Option<IdAny> {
    match v {
        Value::Id(a) => *a,
        _ => None,
    }
}

/// A struct definition by identifier (for building pointers).
pub fn sdef(s: &'static StructDef) -> &'static StructDef {
    s
}
