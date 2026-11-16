// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the session integration tests, plus test-only RNA hooks that stand in
//! for bindings other M1 W2 packages own (WP 1.10 scene, WP 1.27 mesh elements). They are
//! registered on properties the WP 1.10/1.27 bindings serve directly, so once those land the
//! bindings answer first for reads and these hooks only keep the same semantics.
#![allow(dead_code)]

use frap_mesh::Mesh;
use frap_rna::traits::SmolStr;
use frap_rna::{
    CollRef, Field, Item, MathTag, PropDef, PyExc, Rna, RnaColl, RnaError, RnaErrorKind, RnaPtr,
    Root, Seg, StructDef, StructRef, Value,
};
use frap_scene::{FuncArgs, Host, HostRead, RnaFuncReg, RnaHookReg};
use frap_session::{Session, SessionOptions};
use serde_json::Value as J;

pub fn session() -> Session {
    session_with(|_| {})
}

pub fn session_with(f: impl FnOnce(&mut SessionOptions)) -> Session {
    let mut opts = SessionOptions::default();
    opts.cwd = std::env::temp_dir();
    f(&mut opts);
    Session::new(opts).expect("factory session")
}

/// Runs a source unit; the JSON of each response.
pub fn run(s: &mut Session, src: &str) -> Vec<J> {
    s.exec_source(src).iter().map(|r| r.to_json()).collect()
}

/// Runs one statement; its JSON.
pub fn one(s: &mut Session, src: &str) -> J {
    let mut v = run(s, src);
    assert_eq!(v.len(), 1, "{src}: {v:?}");
    v.remove(0)
}

/// Runs one statement that must succeed; its `value`.
pub fn val(s: &mut Session, src: &str) -> J {
    let r = one(s, src);
    assert_eq!(r["status"], "FINISHED", "{src}: {r}");
    r["value"].clone()
}

/// Runs one statement that must fail; `(exception, code, message)`.
pub fn err(s: &mut Session, src: &str) -> (String, String, String) {
    let r = one(s, src);
    assert_eq!(r["status"], "ERROR", "{src}: {r}");
    assert_eq!(r["state"], "unchanged", "{src}: {r}");
    let e = &r["error"];
    (
        e["exception"].as_str().unwrap().to_owned(),
        e["code"].as_str().unwrap().to_owned(),
        e["message"].as_str().unwrap().to_owned(),
    )
}

// ------------------------------------------------------------------ test hooks

/// `MeshVertex` proxy over the positions column.
struct Vert<'a> {
    mesh: &'a Mesh,
    i: usize,
}

impl Rna for Vert<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::MESH_VERTEX
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "co" => Ok(Field::Value(Value::FloatArray {
                data: self.mesh.positions()[self.i].iter().copied().collect(),
                shape: prop.array,
                tag: MathTag::Vector,
            })),
            "index" => Ok(Field::Value(Value::Int(self.i as i64))),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

struct Verts<'a> {
    mesh: &'a Mesh,
}

impl RnaColl for Verts<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::MESH_VERTEX
    }
    fn len(&self) -> usize {
        self.mesh.positions().len()
    }
    fn key_of(&self, _: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        if i >= self.len() {
            return Err(frap_rna::traits::index_out_of_range(i, self.len()));
        }
        Ok(Item::Struct(StructRef::Owned(Box::new(Vert {
            mesh: self.mesh,
            i,
        }))))
    }
}

fn hook_err(msg: &str) -> RnaError {
    RnaError::new(RnaErrorKind::Other, PyExc::RuntimeError, msg)
}

fn verts_get<'h>(
    host: &'h dyn HostRead,
    this: &RnaPtr,
    _: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let Root::Id(id) = this.root else {
        return Err(hook_err("vertices of a non-ID"));
    };
    let mesh = id
        .typed()
        .and_then(|m| host.main().meshes.get(m))
        .ok_or_else(|| hook_err("no mesh"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(Verts { mesh }))))
}

fn co_set(
    host: &mut dyn Host,
    this: &RnaPtr,
    _: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    let (Root::Id(id), Some(Seg::Index(i))) = (this.root, this.path.get(1)) else {
        return Err(hook_err("co of a non-vertex"));
    };
    let (main, log) = host.main_mut();
    let mesh = id
        .typed()
        .and_then(|m| main.meshes.get_mut(m, log))
        .ok_or_else(|| hook_err("no mesh"))?;
    mesh.positions_mut()[*i as usize] = v.as_vec3().ok_or_else(|| hook_err("not a vec3"))?;
    Ok(())
}

fn cursor_get<'h>(
    host: &'h dyn HostRead,
    this: &RnaPtr,
    _: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let Root::Id(id) = this.root else {
        return Err(hook_err("cursor of a non-ID"));
    };
    let scene = id
        .typed()
        .and_then(|s| host.main().scenes.get(s))
        .ok_or_else(|| hook_err("no scene"))?;
    Ok(Field::Struct(StructRef::Borrowed(&scene.cursor)))
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_HOOKS)]
#[linkme(crate = frap_rna::linkme)]
static TEST_MESH_VERTICES: RnaHookReg = RnaHookReg {
    srna: "Mesh",
    prop: "vertices",
    get: Some(verts_get),
    set: None,
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_HOOKS)]
#[linkme(crate = frap_rna::linkme)]
static TEST_VERTEX_CO: RnaHookReg = RnaHookReg {
    srna: "MeshVertex",
    prop: "co",
    get: None,
    set: Some(co_set),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_HOOKS)]
#[linkme(crate = frap_rna::linkme)]
static TEST_SCENE_CURSOR: RnaHookReg = RnaHookReg {
    srna: "Scene",
    prop: "cursor",
    get: Some(cursor_get),
    set: None,
};

/// `Object.is_deform_modified(scene, settings)`: a pure test implementation (no M1 package
/// implements it) used to exercise argument binding and dispatch.
fn is_deform_modified(
    _: &mut dyn Host,
    _: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    Ok(Value::Bool(
        args.get("settings").and_then(Value::as_str) == Some("RENDER"),
    ))
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_scene::RNA_FUNCS)]
#[linkme(crate = frap_rna::linkme)]
static TEST_IS_DEFORM_MODIFIED: RnaFuncReg = RnaFuncReg {
    srna: "Object",
    func: "is_deform_modified",
    imp: is_deform_modified,
};
