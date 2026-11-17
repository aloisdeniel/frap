// SPDX-License-Identifier: GPL-3.0-or-later
//! §9.3 Paths (`frap-rna/src/ptr.rs`, CONTRACT).

use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::{FuncDef, PropDef, StructDef};
use crate::id::{EmbeddedSlot, IdAny};

#[derive(Clone, Debug)]
pub struct RnaPtr {
    pub root: Root,
    pub path: SmallVec<[Seg; 6]>,
    pub srna: &'static StructDef,
}

impl PartialEq for RnaPtr {
    fn eq(&self, o: &Self) -> bool {
        self.root == o.root && self.path == o.path && core::ptr::eq(self.srna, o.srna)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Root {
    Data,
    Context,
    Id(IdAny),
    Embedded { owner: IdAny, slot: EmbeddedSlot },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    Prop(&'static str),
    Index(i64),
    Key(SmolStr),
}

/// RFC 0001 F-PTR-1: `repr(&self, names: &dyn IdNames)` names a trait the contract never
/// defines. Minimal definition: the name of an ID and, for linked IDs, its library path
/// (Blender's repr of a linked ID is `bpy.data.node_groups['Name', '<lib path>']`).
pub trait IdNames {
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)>;
}

/// Python `repr` of a str inside single quotes (verified: `bpy.data.objects['a\'b"c']`).
pub fn py_single_quoted(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('\'');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\'' => o.push_str("\\'"),
            _ => o.push(c),
        }
    }
    o.push('\'');
    o
}

/// RNA path key inside double quotes (`modifiers["Bevel"]`): escapes `"` and `\`.
pub fn rna_double_quoted(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            _ => o.push(c),
        }
    }
    o.push('"');
    o
}

impl RnaPtr {
    pub fn new(root: Root, srna: &'static StructDef) -> Self {
        RnaPtr {
            root,
            path: SmallVec::new(),
            srna,
        }
    }

    pub fn child(&self, seg: Seg, srna: &'static StructDef) -> Self {
        let mut p = self.clone();
        p.path.push(seg);
        p.srna = srna;
        p
    }

    fn id_repr(id: IdAny, names: &dyn IdNames) -> String {
        match names.id_name(id) {
            Some((name, None)) => format!("bpy.data.{}[{}]", id.code.collection(), py_single_quoted(&name)),
            Some((name, Some(lib))) => {
                format!(
                    "bpy.data.{}[{}, {}]",
                    id.code.collection(),
                    py_single_quoted(&name),
                    py_single_quoted(&lib)
                )
            }
            None => format!("<invalid {:?}>", id.code),
        }
    }

    fn root_repr(&self, names: &dyn IdNames) -> String {
        match self.root {
            Root::Data => "bpy.data".into(),
            Root::Context => "bpy.context".into(),
            Root::Id(id) => Self::id_repr(id, names),
            Root::Embedded { owner, slot } => format!("{}.{}", Self::id_repr(owner, names), slot.prop_identifier()),
        }
    }

    fn segs(&self, out: &mut String, leading_dot: bool) {
        for (i, s) in self.path.iter().enumerate() {
            match s {
                Seg::Prop(p) => {
                    if i > 0 || leading_dot {
                        out.push('.');
                    }
                    out.push_str(p);
                }
                Seg::Index(n) => {
                    out.push('[');
                    out.push_str(&n.to_string());
                    out.push(']');
                }
                Seg::Key(k) => {
                    out.push('[');
                    out.push_str(&rna_double_quoted(k));
                    out.push(']');
                }
            }
        }
    }

    /// `bpy.data.objects['Cube'].modifiers["Bevel"]`.
    pub fn repr(&self, names: &dyn IdNames) -> String {
        let mut s = self.root_repr(names);
        self.segs(&mut s, true);
        s
    }

    /// `modifiers["Bevel"].width` (F-curve data_path form), relative to the (embedded) ID.
    pub fn path_from_id(&self) -> Option<String> {
        match self.root {
            Root::Id(_) | Root::Embedded { .. } => {
                let mut s = String::new();
                self.segs(&mut s, false);
                Some(s)
            }
            Root::Data | Root::Context => None,
        }
    }

    pub fn path_from_id_prop(&self, prop: &str) -> Option<String> {
        self.path_from_id()
            .map(|p| if p.is_empty() { prop.to_string() } else { format!("{p}.{prop}") })
    }
}

pub enum Resolved {
    Struct(RnaPtr),
    Prop {
        ptr: RnaPtr,
        prop: &'static PropDef,
        index: Option<i64>,
        component: Option<char>,
    },
    Coll {
        ptr: RnaPtr,
        prop: &'static PropDef,
    },
    Func {
        ptr: RnaPtr,
        func: &'static FuncDef,
    },
}
