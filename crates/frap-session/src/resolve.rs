// SPDX-License-Identifier: GPL-3.0-or-later
//! Path resolution (ARCHITECTURE §9.3; M0 subset: `bpy.data.<coll>[key|index].<prop>` and a
//! few read-only `bpy.context` members). Messages are Blender's (Appendix A).

use frap_lang::{PathExpr, PathRoot, PathSeg};
use frap_rna::{FrapError, IdAny, IdCode, PropDef, PyExc, StructDef};
use frap_scene::Main;
use frap_spec_rna as rna;
use serde_json::Value as J;

use crate::errors::*;

/// What a path points at.
#[derive(Clone, Debug)]
pub enum Target {
    /// `bpy.data`
    DataRoot,
    /// `bpy.data.objects`
    Coll(&'static str),
    /// `bpy.data.objects['Cube']`
    Id(IdAny),
    /// `bpy.data.objects['Cube'].location`
    Prop {
        /// Owning ID.
        id: IdAny,
        /// Property.
        prop: &'static PropDef,
    },
    /// `bpy.context.<member>`
    Context(&'static str),
}

/// RNA struct of an ID code (M0 types).
pub fn srna_of(code: IdCode) -> &'static StructDef {
    match code {
        IdCode::OB => &rna::OBJECT,
        IdCode::ME => &rna::MESH,
        IdCode::MA => &rna::MATERIAL,
        IdCode::CA => &rna::CAMERA,
        IdCode::LA => &rna::LIGHT,
        IdCode::SC => &rna::SCENE,
        IdCode::GR => &rna::COLLECTION,
        IdCode::WO => &rna::WORLD,
        IdCode::IM => &rna::IMAGE,
        IdCode::AR => &rna::ARMATURE,
        IdCode::AC => &rna::ACTION,
        IdCode::KE => &rna::KEY,
        IdCode::NT => &rna::NODE_TREE,
        IdCode::TE => &rna::TEXTURE,
        IdCode::LI => &rna::LIBRARY,
        IdCode::PT => &rna::POINT_CLOUD,
        _ => &rna::ID,
    }
}

/// Item struct of a `bpy.data` collection.
pub fn item_type(coll: &str) -> &'static str {
    Main::code_of(coll).map_or("ID", |c| srna_of(c).identifier)
}

const CONTEXT_MEMBERS: &[&str] = &[
    "active_object",
    "object",
    "selected_objects",
    "mode",
    "scene",
];

fn no_attr(owner: &str, attr: &str, candidates: &[&str]) -> FrapError {
    let sugg = frap_op::suggest(attr, candidates.iter().copied());
    FrapError::new(
        PyExc::AttributeError,
        RNA_NO_ATTRIBUTE,
        format!("'{owner}' object has no attribute '{attr}'"),
    )
    .with_detail(
        "did_you_mean",
        sugg.into_iter().map(J::from).collect::<Vec<_>>(),
    )
}

fn not_yet(what: &str) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!("frap M0: {what} is not supported yet"),
    )
}

/// Resolves a path against `bpy.data` / `bpy.context`.
pub fn resolve(main: &Main, path: &PathExpr) -> Result<Target, FrapError> {
    let mut segs = path.segs.iter();
    if path.root == PathRoot::BpyContext {
        return match (segs.next(), segs.next()) {
            (None, _) => Err(not_yet("reading bpy.context itself")),
            (Some(PathSeg::Attr(a)), None) => {
                match CONTEXT_MEMBERS.iter().find(|m| **m == a.as_str()) {
                    Some(m) => Ok(Target::Context(m)),
                    None => Err(no_attr("Context", a, CONTEXT_MEMBERS)),
                }
            }
            _ => Err(not_yet("paths below bpy.context members")),
        };
    }
    let coll = match segs.next() {
        None => return Ok(Target::DataRoot),
        Some(PathSeg::Attr(a)) => match Main::COLLECTIONS.iter().find(|c| **c == a.as_str()) {
            Some(c) => *c,
            None => return Err(no_attr("BlendData", a, Main::COLLECTIONS)),
        },
        Some(_) => {
            return Err(FrapError::new(
                PyExc::TypeError,
                RNA_NO_ATTRIBUTE,
                "'BlendData' object is not subscriptable",
            ));
        }
    };
    let id = match segs.next() {
        None => return Ok(Target::Coll(coll)),
        Some(PathSeg::Key(k)) => main.find(coll, k).ok_or_else(|| {
            let names = main.names(coll).unwrap_or_default();
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            FrapError::new(
                PyExc::KeyError,
                RNA_KEY_NOT_FOUND,
                format!("'bpy_prop_collection[key]: key \"{k}\" not found'"),
            )
            .with_detail(
                "did_you_mean",
                frap_op::suggest(k, refs.iter().copied())
                    .into_iter()
                    .map(J::from)
                    .collect::<Vec<_>>(),
            )
            .with_detail("valid", refs.into_iter().map(J::from).collect::<Vec<_>>())
        })?,
        Some(PathSeg::Index(i)) => main.at(coll, *i as isize).ok_or_else(|| {
            let n = main.names(coll).map(|v| v.len()).unwrap_or(0);
            FrapError::new(
                PyExc::IndexError,
                RNA_INDEX_RANGE,
                format!("bpy_prop_collection[index]: index {i} out of range, size {n}"),
            )
        })?,
        Some(PathSeg::Attr(a)) => {
            return Err(not_yet(&format!(
                "bpy.data.{coll}.{a} (collection methods)"
            )));
        }
        Some(_) => return Err(not_yet("slices")),
    };
    let prop = match segs.next() {
        None => return Ok(Target::Id(id)),
        Some(PathSeg::Attr(a)) => {
            let srna = srna_of(id.code);
            match srna.find_prop(a) {
                Some(p) => p,
                None => {
                    let names: Vec<&str> = srna.all_props().iter().map(|p| p.identifier).collect();
                    let mut e = no_attr(srna.identifier, a, &names);
                    e = e.with_hint(format!(
                        "frap M0 binds only {} on {}",
                        names
                            .iter()
                            .map(|n| format!("'{n}'"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        srna.identifier
                    ));
                    return Err(e);
                }
            }
        }
        Some(_) => return Err(not_yet("subscripting a datablock")),
    };
    if segs.next().is_some() {
        return Err(not_yet(
            "element access below a property (location[0], location.x)",
        ));
    }
    Ok(Target::Prop { id, prop })
}
