// SPDX-License-Identifier: GPL-3.0-or-later
//! Path resolution (§9.3 "Resolution") written only against the `Rna`/`RnaMut`/`RnaColl`
//! traits. Reads walk read-only and re-root at IDs / embedded IDs / canonical pointers; writes
//! resolve first, then walk again mutably from the owning ID (§9.6 step 3).

use frap_probe_rna::*;
use frap_probe_scene::{Host, HostRead, find_hook};
use frap_probe_spec as spec;

use crate::path::{PRoot, PSeg, parse};

#[derive(Debug, PartialEq)]
pub enum GotValue {
    Value(Value),
    Id(Option<IdAny>),
    Struct,
    Coll { len: usize },
}

/// Result of a read: Blender's repr, the canonical struct pointer, and the value.
#[derive(Debug)]
pub struct Got {
    pub repr: String,
    /// The struct that holds `prop` (or the struct itself when `prop` is None).
    pub ptr: RnaPtr,
    pub prop: Option<&'static PropDef>,
    pub value: GotValue,
}

impl Got {
    pub fn path_from_id(&self) -> Option<String> {
        match self.prop {
            Some(p) => self.ptr.path_from_id_prop(p.identifier),
            None => self.ptr.path_from_id(),
        }
    }
}

fn err(exc: PyExc, code: ErrorCode, msg: String) -> RnaError {
    RnaError::new(exc, code, msg)
}

fn key_not_found(k: &str) -> RnaError {
    err(
        PyExc::KeyError,
        codes::RNA_KEY_NOT_FOUND,
        format!("bpy_prop_collection[key]: key \"{k}\" not found"),
    )
}

fn names(host: &dyn HostRead) -> &dyn IdNames {
    host.main()
}

pub fn get(host: &dyn HostRead, src: &str) -> Result<Got, RnaError> {
    let (root, segs) = parse(src).map_err(|e| err(PyExc::SyntaxError, ErrorCode("SYNTAX"), e))?;
    get_segs(host, root, &segs)
}

pub fn get_segs(host: &dyn HostRead, root: PRoot, segs: &[PSeg]) -> Result<Got, RnaError> {
    match root {
        PRoot::Context => {
            let ctx = host.ctx();
            walk(host, RnaPtr::new(Root::Context, &spec::CONTEXT), &ctx, segs)
        }
        PRoot::Data => {
            let Some(PSeg::Attr(coll)) = segs.first() else {
                return Err(err(PyExc::SyntaxError, ErrorCode("SYNTAX"), "bpy.data needs a collection".into()));
            };
            let (ids, item_type) = host.main().collection(coll).ok_or_else(|| {
                err(
                    PyExc::AttributeError,
                    codes::RNA_NO_ATTRIBUTE,
                    format!("'BlendData' object has no attribute '{coll}'"),
                )
            })?;
            let Some(sel) = segs.get(1) else {
                return Ok(Got {
                    repr: format!("bpy.data.{coll}"),
                    ptr: RnaPtr::new(Root::Data, item_type),
                    prop: None,
                    value: GotValue::Coll { len: ids.len() },
                });
            };
            let a = match sel {
                PSeg::Key(k) => *ids
                    .iter()
                    .find(|a| names(host).id_name(**a).is_some_and(|(n, _)| n == k.as_str()))
                    .ok_or_else(|| key_not_found(k))?,
                PSeg::Index(i) => *index(ids.len(), *i).map(|i| &ids[i])?,
                PSeg::Attr(_) => {
                    return Err(err(
                        PyExc::NotImplementedError,
                        codes::NOT_IMPLEMENTED,
                        "probe: collection methods".into(),
                    ));
                }
            };
            step_id(host, a, &segs[2..])
        }
    }
}

fn index(len: usize, i: i64) -> Result<usize, RnaError> {
    let j = if i < 0 { len as i64 + i } else { i };
    if j < 0 || j >= len as i64 {
        return Err(err(
            PyExc::IndexError,
            codes::RNA_INDEX_RANGE,
            format!("bpy_prop_collection[index]: index {i} out of range, size {len}"),
        ));
    }
    Ok(j as usize)
}

fn step_id(host: &dyn HostRead, a: IdAny, rest: &[PSeg]) -> Result<Got, RnaError> {
    let r = host
        .main()
        .id(a)
        .ok_or_else(|| err(PyExc::RuntimeError, codes::INTERNAL, "dangling ID".into()))?;
    walk(host, RnaPtr::new(Root::Id(a), r.srna()), r, rest)
}

fn walk(host: &dyn HostRead, ptr: RnaPtr, cur: &dyn Rna, segs: &[PSeg]) -> Result<Got, RnaError> {
    let Some(first) = segs.first() else {
        return Ok(Got {
            repr: ptr.repr(names(host)),
            ptr,
            prop: None,
            value: GotValue::Struct,
        });
    };
    let PSeg::Attr(name) = first else {
        return Err(err(
            PyExc::TypeError,
            codes::RNA_TYPE,
            format!("'{}' object is not subscriptable", cur.srna().identifier),
        ));
    };
    let srna = cur.srna();
    let prop = srna.find_prop(name).ok_or_else(|| no_attr(srna, name))?;
    let field = match cur.get(prop)? {
        Field::NeedsHook => {
            let hook = find_hook(srna, name).and_then(|h| h.get).ok_or_else(|| unbound(srna, name))?;
            hook(host, &ptr, prop)?
        }
        f => f,
    };
    let rest = &segs[1..];
    match field {
        Field::Value(v) => {
            if !rest.is_empty() {
                return Err(err(
                    PyExc::NotImplementedError,
                    codes::NOT_IMPLEMENTED,
                    "probe: array element access".into(),
                ));
            }
            Ok(Got {
                repr: format!("{}.{}", ptr.repr(names(host)), name),
                ptr,
                prop: Some(prop),
                value: GotValue::Value(v),
            })
        }
        Field::Id(opt) => match (opt, rest.is_empty()) {
            (_, true) => Ok(Got {
                repr: format!("{}.{}", ptr.repr(names(host)), name),
                ptr,
                prop: Some(prop),
                value: GotValue::Id(opt),
            }),
            (None, false) => Err(err(
                PyExc::AttributeError,
                codes::RNA_NO_ATTRIBUTE,
                format!("'NoneType' object has no attribute '{:?}'", rest[0]),
            )),
            (Some(a), false) => step_id(host, a, rest),
        },
        Field::Embedded(slot, sref) => {
            let owner = match ptr.root {
                Root::Id(a) if ptr.path.is_empty() => a,
                _ => return Err(err(PyExc::RuntimeError, codes::INTERNAL, "embedded ID below the owner root".into())),
            };
            walk(host, RnaPtr::new(Root::Embedded { owner, slot }, sref.srna()), &*sref, rest)
        }
        Field::Ptr(p) => {
            // Re-root at a canonical pointer: resolve its own path from its root, then `rest`.
            let Root::Id(a) = p.root else {
                return Err(err(PyExc::RuntimeError, codes::INTERNAL, "Ptr must be ID-rooted".into()));
            };
            let mut segs: Vec<PSeg> = p
                .path
                .iter()
                .map(|s| match s {
                    Seg::Prop(p) => PSeg::Attr((*p).into()),
                    Seg::Key(k) => PSeg::Key(k.to_string()),
                    Seg::Index(i) => PSeg::Index(*i),
                })
                .collect();
            segs.extend_from_slice(rest);
            step_id(host, a, &segs)
        }
        Field::Struct(sref) => {
            let nptr = ptr.child(Seg::Prop(prop.identifier), sref.srna());
            walk(host, nptr, &*sref, rest)
        }
        Field::Coll(c) => {
            let Some(sel) = rest.first() else {
                return Ok(Got {
                    repr: format!("{}.{}", ptr.repr(names(host)), name),
                    ptr,
                    prop: Some(prop),
                    value: GotValue::Coll { len: c.len() },
                });
            };
            let i = match sel {
                PSeg::Index(i) => index(c.len(), *i)?,
                PSeg::Key(k) => coll_find(host, &*c, srna, name, k)?,
                PSeg::Attr(a) => {
                    return Err(err(
                        PyExc::NotImplementedError,
                        codes::NOT_IMPLEMENTED,
                        format!("probe: collection attribute {a}"),
                    ));
                }
            };
            match c.item_any(i)? {
                Item::Struct(sref) => {
                    let nptr = ptr.child(Seg::Prop(prop.identifier), sref.srna()).child(c.item_seg(i), sref.srna());
                    walk(host, nptr, &*sref, &rest[1..])
                }
                Item::Id(Some(a)) => step_id(host, a, &rest[1..]),
                Item::Id(None) if rest.len() == 1 => Ok(Got {
                    repr: "None".into(),
                    ptr,
                    prop: Some(prop),
                    value: GotValue::Id(None),
                }),
                Item::Id(None) => Err(err(
                    PyExc::AttributeError,
                    codes::RNA_NO_ATTRIBUTE,
                    "'NoneType' object has no attribute".into(),
                )),
            }
        }
        Field::NeedsHook => Err(unbound(srna, name)),
    }
}

/// `coll['key']`: `name_property` decides whether string lookups exist (verified wording for
/// nameless collections); ID-item collections are searched by ID name (F-TR-4).
fn coll_find(host: &dyn HostRead, c: &dyn RnaColl, owner: &'static StructDef, prop: &str, k: &str) -> Result<usize, RnaError> {
    if c.item_type().name_property.is_none() {
        return Err(err(
            PyExc::TypeError,
            codes::RNA_TYPE,
            format!(
                "bpy_prop_collection[key]: {}.{} does not support string lookups",
                owner.identifier, prop
            ),
        ));
    }
    if let Some(i) = c.find(k) {
        return Ok(i);
    }
    for i in 0..c.len() {
        if let Ok(Item::Id(Some(a))) = c.item_any(i)
            && names(host).id_name(a).is_some_and(|(n, _)| n == k)
        {
            return Ok(i);
        }
    }
    Err(key_not_found(k))
}

// ---------------------------------------------------------------------------------- set

/// `<path> = <input>`: resolve the owner struct read-only, coerce, then hook or mutable walk.
pub fn set(host: &mut dyn Host, src: &str, input: Input) -> Result<(), RnaError> {
    let (root, segs) = parse(src).map_err(|e| err(PyExc::SyntaxError, ErrorCode("SYNTAX"), e))?;
    let Some((PSeg::Attr(name), owner_segs)) = segs.split_last() else {
        return Err(err(
            PyExc::NotImplementedError,
            codes::NOT_IMPLEMENTED,
            "probe: item assignment".into(),
        ));
    };
    let owner = get_segs(host, root, owner_segs)?;
    let (ptr, srna) = match owner.value {
        GotValue::Struct => {
            let s = owner.ptr.srna;
            (owner.ptr, s)
        }
        GotValue::Id(Some(a)) => {
            let s = host
                .main()
                .id(a)
                .map(|r| r.srna())
                .ok_or_else(|| err(PyExc::RuntimeError, codes::INTERNAL, "dangling".into()))?;
            (RnaPtr::new(Root::Id(a), s), s)
        }
        _ => return Err(err(PyExc::TypeError, codes::RNA_TYPE, "probe: target is not a struct".into())),
    };
    let prop = srna.find_prop(name).ok_or_else(|| no_attr(srna, name))?;
    let v = coerce(prop, input, &CoerceCx::RnaSet { owner: srna.identifier })?.value;
    set_resolved(host, &ptr, prop, v)
}

pub fn set_resolved(host: &mut dyn Host, ptr: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    if let Some(hook) = find_hook(ptr.srna, prop.identifier).and_then(|h| h.set) {
        return hook(host, ptr, prop, v);
    }
    let outcome = {
        let (main, log) = host.main_mut();
        match ptr.root {
            Root::Id(a) => {
                let r = main
                    .id_mut(a, log)
                    .ok_or_else(|| err(PyExc::RuntimeError, codes::INTERNAL, "dangling".into()))?;
                walk_set(r, &ptr.path, prop, v)?
            }
            Root::Embedded { owner, slot } => {
                // Mutation of an embedded ID goes through the owner's get_mut (owner gen bump, §5.1).
                let r = main
                    .id_mut(owner, log)
                    .ok_or_else(|| err(PyExc::RuntimeError, codes::INTERNAL, "dangling".into()))?;
                let pd = r
                    .srna()
                    .find_prop(slot.prop_identifier())
                    .ok_or_else(|| unbound(r.srna(), slot.prop_identifier()))?;
                let mut g = r.struct_mut(pd)?;
                walk_set(&mut *g, &ptr.path, prop, v)?
            }
            Root::Data | Root::Context => {
                return Err(err(
                    PyExc::AttributeError,
                    codes::RNA_READONLY,
                    format!(
                        "bpy_struct: attribute \"{}\" from \"{}\" is read-only",
                        prop.identifier, ptr.srna.identifier
                    ),
                ));
            }
        }
    };
    match outcome {
        SetOutcome::Done => Ok(()),
        SetOutcome::NeedsHook => Err(unbound(ptr.srna, prop.identifier)),
    }
}

fn walk_set(cur: &mut dyn RnaMut, path: &[Seg], prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
    let Some(first) = path.first() else { return cur.set(prop, v) };
    let Seg::Prop(p) = first else {
        return Err(err(
            PyExc::RuntimeError,
            codes::INTERNAL,
            "canonical path starts with a prop".into(),
        ));
    };
    let srna = cur.srna();
    let pd = srna.find_prop(p).ok_or_else(|| no_attr(srna, p))?;
    match path.get(1) {
        Some(Seg::Index(i)) => {
            let mut c = cur.coll_mut(pd)?;
            let idx = index(c.len(), *i)?;
            let mut item = c.item_mut(idx)?;
            walk_set(&mut *item, &path[2..], prop, v)
        }
        Some(Seg::Key(k)) => {
            let mut c = cur.coll_mut(pd)?;
            let idx = c.find(k).ok_or_else(|| key_not_found(k))?;
            let mut item = c.item_mut(idx)?;
            walk_set(&mut *item, &path[2..], prop, v)
        }
        _ => {
            let mut g = cur.struct_mut(pd)?;
            walk_set(&mut *g, &path[1..], prop, v)
        }
    }
}
