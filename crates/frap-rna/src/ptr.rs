// SPDX-License-Identifier: GPL-3.0-or-later
//! Property paths (ARCHITECTURE §9.3, CONTRACT frozen at M1 W0; `IdNames` defined by RFC 0001
//! F-PTR-1, embedded roots by the v1.1 review).

use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::{FuncDef, PropDef, StructDef};
use crate::id::{EmbeddedSlot, IdAny, IdCode};

/// Where a path starts.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Root {
    /// `bpy.data`
    Data,
    /// `bpy.context`
    Context,
    /// A datablock.
    Id(IdAny),
    /// An embedded ID: `material.node_tree`, `world.node_tree`, `scene.collection`.
    Embedded {
        /// The owning ID.
        owner: IdAny,
        /// Which embedded ID of the owner.
        slot: EmbeddedSlot,
    },
}

/// One segment below the root.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Seg {
    /// `.name`
    Prop(&'static str),
    /// `[3]`
    Index(i64),
    /// `["Name"]`
    Key(SmolStr),
}

/// A pointer to a struct (or collection) inside the data model. Paths never borrow across
/// datablocks: on reaching an ID the resolver re-roots at `Root::Id` (§9.3).
#[derive(Clone, Debug)]
pub struct RnaPtr {
    /// Root.
    pub root: Root,
    /// Canonical segments from the root (RFC 0001 F-TR-5: never the user's key for sockets
    /// or material slots).
    pub path: SmallVec<[Seg; 6]>,
    /// Struct type at the end of the path.
    pub srna: &'static StructDef,
}

impl PartialEq for RnaPtr {
    fn eq(&self, o: &Self) -> bool {
        self.root == o.root && self.path == o.path && std::ptr::eq(self.srna, o.srna)
    }
}

/// Name lookup for `repr` (implemented by `Main`): the ID's name and, for a linked ID that a
/// plain `['Name']` lookup would not find (a local ID of the same name shadows it), its library
/// **filepath** (`Library.filepath` as stored, e.g. `//lib.blend` or an absolute path), the
/// second element of the `['Name', '<library filepath>']` repr (D-15). Other linked IDs return
/// `None` and repr like Blender's. `.frap` references use the library *name* instead, through
/// [`crate::serde_id::IdRefNamer`].
pub trait IdNames {
    /// `(name, library)`; `None` if the ID does not exist.
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)>;
}

/// The result of resolving a path expression (§9.3).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Resolved {
    /// A struct (or ID).
    Struct(RnaPtr),
    /// A property of a struct.
    Prop {
        /// Owner.
        ptr: RnaPtr,
        /// The property.
        prop: &'static PropDef,
        /// `location[1]`
        index: Option<i64>,
        /// `.x .y .z .w .r .g .b`
        component: Option<char>,
    },
    /// A collection property.
    Coll {
        /// Owner.
        ptr: RnaPtr,
        /// The collection property.
        prop: &'static PropDef,
    },
    /// An RNA function.
    Func {
        /// Owner.
        ptr: RnaPtr,
        /// The function.
        func: &'static FuncDef,
    },
}

/// Quotes a name the way Python's `repr` of a `str` does (single quotes, unless the name
/// contains a single quote and no double quote). Blender's ID repr uses `%R`, i.e. this rule:
/// `bpy.data.objects['a\'b"c']` (verified, RFC 0001 appendix).
pub fn py_str_repr(s: &str) -> String {
    let use_double = s.contains('\'') && !s.contains('"');
    let q = if use_double { '"' } else { '\'' };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(q);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c == q => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(q);
    out
}

/// Double-quoted key below the root (`modifiers["Bevel"]`), as `RNA_path` writes it.
pub fn rna_key_repr(s: &str) -> String {
    format!("[\"{}\"]", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `bpy.data.objects['Cube']`, or `bpy.data.node_groups['Name', '<library filepath>']` when the
/// caller passes a library (§9.3 contract, the `bpy_prop_collection[name, lib]` key form).
///
/// [`IdNames`] passes a library only for a linked ID shadowed by a local ID of the same name.
/// Blender 5.2.4 prints every linked ID without its library (verified); frap keeps it in that
/// one case so that a local and a linked ID never share a repr (deviation D-15).
pub fn id_repr(code: IdCode, name: &str, library: Option<&str>) -> String {
    match library {
        None => format!("bpy.data.{}[{}]", code.collection(), py_str_repr(name)),
        Some(lib) => format!(
            "bpy.data.{}[{}, {}]",
            code.collection(),
            py_str_repr(name),
            py_str_repr(lib)
        ),
    }
}

impl RnaPtr {
    /// Pointer at a root with no segments.
    pub fn new(root: Root, srna: &'static StructDef) -> Self {
        RnaPtr {
            root,
            path: SmallVec::new(),
            srna,
        }
    }
    /// Pointer to an ID.
    pub fn id(id: IdAny, srna: &'static StructDef) -> Self {
        RnaPtr::new(Root::Id(id), srna)
    }
    /// This pointer extended by one segment.
    pub fn child(&self, seg: Seg, srna: &'static StructDef) -> Self {
        let mut p = self.clone();
        p.path.push(seg);
        p.srna = srna;
        p
    }

    fn root_repr(&self, names: &dyn IdNames) -> String {
        let idr = |id: IdAny| match names.id_name(id) {
            Some((n, lib)) => id_repr(id.code, &n, lib.as_deref()),
            None => format!("bpy.data.{}[<removed>]", id.code.collection()),
        };
        match self.root {
            Root::Data => "bpy.data".to_owned(),
            Root::Context => "bpy.context".to_owned(),
            Root::Id(id) => idr(id),
            Root::Embedded { owner, slot } => format!("{}.{}", idr(owner), slot.prop_identifier()),
        }
    }

    fn push_segs(&self, out: &mut String, leading_dot: bool) {
        for (i, seg) in self.path.iter().enumerate() {
            match seg {
                Seg::Prop(p) => {
                    if i > 0 || leading_dot {
                        out.push('.');
                    }
                    out.push_str(p);
                }
                Seg::Index(n) => out.push_str(&format!("[{n}]")),
                Seg::Key(k) => out.push_str(&rna_key_repr(k)),
            }
        }
    }

    /// Python-style repr: `bpy.data.objects['Cube'].modifiers["Bevel"]`; single quotes at the
    /// root, double quotes below; an embedded root is the owner's repr plus the slot.
    pub fn repr(&self, names: &dyn IdNames) -> String {
        let mut s = self.root_repr(names);
        self.push_segs(&mut s, true);
        s
    }

    /// `modifiers["Bevel"].width` form (F-curve `data_path`), relative to the (embedded) ID;
    /// `None` for `bpy.data` / `bpy.context` roots. Byte-identical to Blender (the glTF exporter
    /// pattern-matches on it).
    pub fn path_from_id(&self) -> Option<String> {
        match self.root {
            Root::Id(_) | Root::Embedded { .. } => {
                let mut s = String::new();
                self.push_segs(&mut s, false);
                Some(s)
            }
            Root::Data | Root::Context => None,
        }
    }
}
