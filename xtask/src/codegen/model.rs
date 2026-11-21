// SPDX-License-Identifier: GPL-3.0-or-later
//! Loading the Blender dumps (`spec/blender-5.2/`), frap's own operator specs (`spec/frap/`),
//! the overlays (`spec/overlay/*.toml`) and the OPERATORS.md milestone column into one
//! normalised model. Every generator reads only this model, so the compiled tables
//! (`frap-spec-*`) and the catalog blob decode the same facts.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value as J;

/// `Result` with a message.
pub type Res<T> = Result<T, String>;

/// Scalar or per-element default.
#[derive(Clone, Debug, PartialEq)]
pub enum Def<T> {
    /// Scalar default.
    Scalar(T),
    /// Array default (flattened, row-major as Blender reports it).
    Array(Vec<T>),
}

/// One enum item.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub value: i64,
}

/// Enum default.
#[derive(Clone, Debug, PartialEq)]
pub enum EnumDef {
    Item(String),
    Flags(Vec<String>),
}

/// Array shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Scalar,
    Fixed(u16),
    Matrix(u16, u16),
    Dynamic,
}

/// Type-specific metadata (mirrors `frap_rna::PropKind`).
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Bool(Def<bool>),
    Int {
        default: Def<i64>,
        hard: (i64, i64),
        soft: (i64, i64),
    },
    Float {
        default: Def<f64>,
        hard: (f64, f64),
        soft: (f64, f64),
        precision: u8,
        f64_storage: bool,
    },
    Str {
        default: String,
        max_bytes: u32,
    },
    Enum {
        items: Vec<Item>,
        default: EnumDef,
        /// Overlay key when the items come from a callback in Blender.
        dynamic: Option<String>,
        /// Items depend on user data / live context (`runtime = true` in the overlay).
        runtime: bool,
    },
    Pointer {
        fixed_type: String,
        is_id: bool,
    },
    Coll {
        item_type: String,
        api: Option<String>,
    },
}

/// Deprecation metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Deprecation {
    pub note: String,
    pub version: [u16; 3],
    pub removal_version: [u16; 3],
}

/// A normalised RNA / operator property.
#[derive(Clone, Debug, PartialEq)]
pub struct Prop {
    pub identifier: String,
    pub name: String,
    pub description: String,
    pub kind: Kind,
    pub subtype: String,
    pub unit: String,
    pub array: Shape,
    /// `frap_rna::PropFlags` names, in declaration order.
    pub flags: Vec<String>,
    pub deprecated: Option<Deprecation>,
}

impl Prop {
    pub fn has_flag(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
    pub fn type_str(&self) -> &'static str {
        match self.kind {
            Kind::Bool(_) => "BOOLEAN",
            Kind::Int { .. } => "INT",
            Kind::Float { .. } => "FLOAT",
            Kind::Str { .. } => "STRING",
            Kind::Enum { .. } => "ENUM",
            Kind::Pointer { .. } => "POINTER",
            Kind::Coll { .. } => "COLLECTION",
        }
    }
}

/// An RNA function.
#[derive(Clone, Debug)]
pub struct Func {
    pub identifier: String,
    pub description: String,
    pub params: Vec<Prop>,
    pub returns: Vec<Prop>,
    pub keyword_only_from: Option<u8>,
    /// `FuncFlags` names.
    pub flags: Vec<String>,
}

/// An RNA struct.
#[derive(Clone, Debug)]
pub struct Struct {
    pub identifier: String,
    pub name: String,
    pub description: String,
    pub base: Option<String>,
    pub name_property: Option<String>,
    pub is_id: bool,
    pub props: Vec<Prop>,
    pub funcs: Vec<Func>,
    /// `fresh.differs_from_default` of the dump (RFC 0001 F-DEF-7).
    pub fresh: Option<serde_json::Map<String, J>>,
}

/// Poll cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    Y,
    N,
    Raised,
}

/// An operator.
#[derive(Clone, Debug)]
pub struct Op {
    pub idname: String,
    pub bl_idname: String,
    pub label: String,
    pub description: String,
    pub options: Vec<String>,
    pub props: Vec<Prop>,
    pub macro_steps: Vec<String>,
    /// `[plain, view3d]` × contexts A–E; `None` for hand-written frap specs.
    pub poll: Option<[[Cell; 5]; 2]>,
    /// Blender namespace ("mesh").
    pub ns: String,
    /// Ops group (`frap-spec-ops-<group>`).
    pub group: &'static str,
    /// OPERATORS.md milestone (`None` = OUT or not listed).
    pub milestone: Option<u32>,
}

/// A frap-only operator option (`spec/overlay/frap_options.toml`).
#[derive(Clone, Debug)]
pub struct FrapOption {
    pub op: String,
    pub identifier: String,
    pub feature: String,
    pub milestone: u32,
    pub prop: Prop,
}

/// One `compat_4x.toml` entry.
#[derive(Clone, Debug)]
pub struct CompatMember {
    pub struct_: String,
    pub identifier: String,
    pub kind: String,
}

/// The whole model.
pub struct Spec {
    pub root: PathBuf,
    pub milestone: u32,
    /// Sorted by identifier.
    pub structs: BTreeMap<String, Struct>,
    /// `bpy_struct.*` Python-only functions (keyframe_insert, …).
    pub bpy_struct_funcs: Vec<Func>,
    /// Every operator (dumped + frap), sorted by idname.
    pub ops: Vec<Op>,
    pub frap_options: Vec<FrapOption>,
    pub frap_features: BTreeSet<String>,
    /// op idname -> (feature -> Blender prop).
    pub frap_covered: BTreeMap<String, BTreeMap<String, String>>,
    pub compat: Vec<CompatMember>,
    /// (struct, identifier) of every Python-only property (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub python_only_props: Vec<(String, String)>,
    /// Shader node dumps, sorted by bl_idname.
    pub nodes: Vec<J>,
    /// Modifier dumps by type, in `Modifier.type` item order.
    pub modifiers: Vec<J>,
    /// `Modifier.type` items (all of them, grease pencil included).
    pub modifier_types: Vec<Item>,
    /// Raw `rna/_meta.json` (tests).
    #[cfg_attr(not(test), allow(dead_code))]
    pub rna_meta: J,
    pub dynamic_overlay_keys: BTreeSet<String>,
    pub used_dynamic_overlay_keys: BTreeSet<String>,
}

/// Blender namespace -> ops group (ARCHITECTURE §3.2: `frap-spec-ops-<group>`).
pub fn group_of(ns: &str) -> Res<&'static str> {
    Ok(match ns {
        "object" | "view3d" | "collection" | "scene" => "object",
        "transform" => "transform",
        "mesh" | "geometry" => "mesh",
        "uv" => "uv",
        "anim" | "screen" | "armature" | "pose" | "nla" | "marker" => "anim",
        "material" | "image" | "world" => "data",
        "export_scene" | "import_scene" | "wm" | "ed" => "io",
        "render" => "render",
        "frap" => "frap",
        other => return Err(format!("namespace `{other}` has no ops group (§3.2)")),
    })
}

/// Every ops group, in crate order.
pub const GROUPS: [&str; 9] = [
    "object",
    "transform",
    "mesh",
    "uv",
    "anim",
    "data",
    "io",
    "render",
    "frap",
];

/// `frap_rna::PropFlags` names in declaration order (the dump names they come from are equal).
pub const PROP_FLAGS: [&str; 15] = [
    "READONLY",
    "HIDDEN",
    "SKIP_SAVE",
    "ENUM_FLAG",
    "REQUIRED",
    "OUTPUT",
    "ANIMATABLE",
    "PYTHON_ONLY",
    "DYNAMIC_ENUM",
    "NEVER_NULL",
    "ARGUMENT_OPTIONAL",
    "DEPRECATED",
    "PATH_SUPPORTS_BLEND_RELATIVE",
    "CONTEXT_MEMBER",
    "SKIP_PRESET",
];

pub fn read_json(path: &Path) -> Res<J> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))
}

fn read_toml(path: &Path) -> Res<toml::Table> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    s.parse::<toml::Table>()
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn s(j: &J) -> String {
    j.as_str().unwrap_or("").to_owned()
}

/// A dumped float: a JSON number or the tagged non-finite form `{"$float": "inf"}`.
pub fn jf64(j: &J) -> Res<f64> {
    match j {
        J::Number(n) => n.as_f64().ok_or_else(|| format!("bad float {j}")),
        J::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        J::Object(o) => match o.get("$float").and_then(J::as_str) {
            Some("inf") => Ok(f64::INFINITY),
            Some("-inf") => Ok(f64::NEG_INFINITY),
            Some("nan") => Ok(f64::NAN),
            _ => Err(format!("bad tagged float {j}")),
        },
        _ => Err(format!("not a float: {j}")),
    }
}

fn ji64(j: &J) -> Res<i64> {
    match j {
        J::Number(n) => n
            .as_i64()
            .or_else(|| n.as_f64().map(|f| f as i64))
            .ok_or_else(|| format!("bad int {j}")),
        J::Bool(b) => Ok(*b as i64),
        _ => Err(format!("not an int: {j}")),
    }
}

fn jbool(j: &J) -> Res<bool> {
    match j {
        J::Bool(b) => Ok(*b),
        J::Number(n) => Ok(n.as_i64().unwrap_or(0) != 0),
        _ => Err(format!("not a bool: {j}")),
    }
}

fn flatten(j: &J, out: &mut Vec<J>) {
    match j {
        J::Array(a) => a.iter().for_each(|x| flatten(x, out)),
        x => out.push(x.clone()),
    }
}

fn def<T>(j: &J, f: fn(&J) -> Res<T>) -> Res<Def<T>> {
    if j.is_array() {
        let mut flat = Vec::new();
        flatten(j, &mut flat);
        Ok(Def::Array(flat.iter().map(f).collect::<Res<_>>()?))
    } else {
        Ok(Def::Scalar(f(j)?))
    }
}

fn shape(p: &J) -> Res<Shape> {
    let Some(a) = p.get("array").and_then(J::as_array) else {
        return Ok(Shape::Scalar);
    };
    let dims: Vec<&J> = a.iter().collect();
    Ok(match dims.as_slice() {
        [] => Shape::Scalar,
        [J::String(d)] if d == "dynamic" => Shape::Dynamic,
        [n] => Shape::Fixed(n.as_u64().ok_or("bad array dim")? as u16),
        [r, c] => Shape::Matrix(
            r.as_u64().ok_or("bad array dim")? as u16,
            c.as_u64().ok_or("bad array dim")? as u16,
        ),
        _ => return Err(format!("unsupported array shape {a:?}")),
    })
}

fn items_of(j: &J) -> Vec<Item> {
    j.as_array()
        .map(|a| {
            a.iter()
                .map(|i| Item {
                    id: s(&i["identifier"]),
                    name: s(&i["name"]),
                    desc: s(&i["description"]),
                    value: i["value"].as_i64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn canonical_flags(names: &BTreeSet<String>) -> Vec<String> {
    PROP_FLAGS
        .iter()
        .filter(|f| names.contains(**f))
        .map(|f| f.to_string())
        .collect()
}

/// Dynamic-enum overlay entry.
#[derive(Clone, Debug, Default)]
struct DynEntry {
    items: Option<Vec<Item>>,
    items_from: Option<String>,
    runtime: bool,
    default: Option<String>,
}

/// Overlays and indexes needed while normalising props.
pub struct Ctx {
    dynamic: BTreeMap<String, DynEntry>,
    f64_props: BTreeSet<String>,
    /// "rna:Struct.prop" / "op:BL.prop" -> raw dumped items.
    raw_items: BTreeMap<String, Vec<Item>>,
    pub used_dynamic: BTreeSet<String>,
}

impl Ctx {
    /// Normalises one dumped property. `key` is the overlay key ("Struct.prop" or "BL.prop").
    pub fn prop(&mut self, p: &J, key: &str) -> Res<Prop> {
        let identifier = s(&p["identifier"]);
        let ty = p["type"]
            .as_str()
            .ok_or_else(|| format!("{key}: no type"))?;
        let mut flags: BTreeSet<String> = p["flags"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        flags.retain(|f| PROP_FLAGS.contains(&f.as_str()));
        let array = shape(p)?;
        let ctx = |e: String| format!("{key}: {e}");
        let kind = match ty {
            "BOOLEAN" => Kind::Bool(def(&p["default"], jbool).map_err(ctx)?),
            "INT" => Kind::Int {
                default: def(&p["default"], ji64).map_err(ctx)?,
                hard: (ji64(&p["hard_min"])?, ji64(&p["hard_max"])?),
                soft: (ji64(&p["soft_min"])?, ji64(&p["soft_max"])?),
            },
            "FLOAT" => Kind::Float {
                default: def(&p["default"], jf64).map_err(ctx)?,
                hard: (jf64(&p["hard_min"])?, jf64(&p["hard_max"])?),
                soft: (jf64(&p["soft_min"])?, jf64(&p["soft_max"])?),
                // Blender reports -1 ("automatic") for a few modifier props: stored as 0.
                precision: p["precision"].as_i64().map_or(3, |v| v.clamp(0, 255) as u8),
                f64_storage: self.f64_props.contains(key),
            },
            "STRING" => Kind::Str {
                default: s(&p["default"]),
                max_bytes: p["length_max"].as_u64().unwrap_or(0) as u32,
            },
            "ENUM" => {
                let is_flag = flags.contains("ENUM_FLAG");
                let dumped = items_of(&p["items"]);
                let entry = self.dynamic.get(key).cloned();
                let dynamic = entry.is_some()
                    || p.get("items_static").is_some()
                    || p.get("dynamic_hint").and_then(J::as_bool) == Some(true)
                    || (!is_flag && dumped.is_empty());
                let mut items = dumped;
                let mut default = if is_flag {
                    EnumDef::Flags(
                        p["default"]
                            .as_array()
                            .map(|a| a.iter().map(s).collect())
                            .unwrap_or_default(),
                    )
                } else {
                    EnumDef::Item(s(&p["default"]))
                };
                let mut runtime = false;
                if let Some(e) = &entry {
                    self.used_dynamic.insert(key.to_owned());
                    if let Some(i) = &e.items {
                        items = i.clone();
                    } else if let Some(src) = &e.items_from {
                        // The source's explicit overlay items win over its dumped ones.
                        let src_key = src.split_once(':').map_or(src.as_str(), |x| x.1);
                        let explicit = self.dynamic.get(src_key).and_then(|d| d.items.clone());
                        items = match explicit {
                            Some(i) => i,
                            None => self.raw_items.get(src).cloned().ok_or_else(|| {
                                format!("{key}: items_from `{src}` is not dumped")
                            })?,
                        };
                        if items.is_empty() {
                            return Err(format!("{key}: items_from `{src}` has no items"));
                        }
                    }
                    if let Some(d) = &e.default {
                        default = EnumDef::Item(d.clone());
                    }
                    runtime = e.runtime;
                }
                if dynamic && items.is_empty() && !runtime {
                    return Err(format!(
                        "{key}: dynamic enum with no items: add it to spec/overlay/dynamic_enums.toml"
                    ));
                }
                if dynamic {
                    flags.insert("DYNAMIC_ENUM".into());
                }
                Kind::Enum {
                    items,
                    default,
                    dynamic: dynamic.then(|| key.to_owned()),
                    runtime,
                }
            }
            "POINTER" => Kind::Pointer {
                fixed_type: p["fixed_type"].as_str().unwrap_or("AnyType").to_owned(),
                is_id: p["is_id"].as_bool().unwrap_or(false),
            },
            "COLLECTION" => Kind::Coll {
                item_type: p["fixed_type"].as_str().unwrap_or("AnyType").to_owned(),
                api: p["api"].as_str().map(str::to_owned),
            },
            other => return Err(format!("{key}: unknown property type {other}")),
        };
        let deprecated = match p.get("deprecated") {
            Some(d) => {
                let v = |k: &str| -> [u16; 3] {
                    let a: Vec<u16> = d[k]
                        .as_array()
                        .map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0) as u16).collect())
                        .unwrap_or_default();
                    [
                        a.first().copied().unwrap_or(0),
                        a.get(1).copied().unwrap_or(0),
                        a.get(2).copied().unwrap_or(0),
                    ]
                };
                Some(Deprecation {
                    note: s(&d["note"]),
                    version: v("version"),
                    removal_version: v("removal_version"),
                })
            }
            None => None,
        };
        Ok(Prop {
            identifier,
            name: s(&p["name"]),
            description: s(&p["description"]),
            kind,
            // Pointer props with a non-NONE C subtype (`ColorMapping.color_ramp`) report "" in bpy.
            subtype: match p["subtype"].as_str() {
                None | Some("") => "NONE".to_owned(),
                Some(x) => x.to_owned(),
            },
            unit: p["unit"].as_str().unwrap_or("NONE").to_owned(),
            array,
            flags: canonical_flags(&flags),
            deprecated,
        })
    }
}

fn toml_items(v: &toml::Value) -> Vec<Item> {
    v.as_array()
        .map(|a| {
            a.iter()
                .enumerate()
                .map(|(i, it)| {
                    let row = it.as_array().cloned().unwrap_or_default();
                    let st =
                        |k: usize| row.get(k).and_then(|x| x.as_str()).unwrap_or("").to_owned();
                    Item {
                        id: st(0),
                        name: st(1),
                        desc: st(2),
                        value: row.get(3).and_then(|x| x.as_integer()).unwrap_or(i as i64),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Parses OPERATORS.md: `| [ ] | \`ns.op\` | M1 | …` → idname -> milestone (`None` = OUT).
pub fn parse_operators_md(text: &str) -> Res<BTreeMap<String, Option<u32>>> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        if cells.len() < 5 || !cells[0].is_empty() {
            continue;
        }
        let Some(name) = cells[2].strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
            continue;
        };
        let mut parts = name.split('.');
        let (Some(ns), Some(op), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let ok = |x: &str| {
            !x.is_empty()
                && x.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        if !ok(ns) || !ok(op) {
            continue;
        }
        let m = match cells[3] {
            "OUT" => None,
            m if m.starts_with('M') => Some(
                m[1..]
                    .parse::<u32>()
                    .map_err(|_| format!("OPERATORS.md: bad milestone `{m}` for {name}"))?,
            ),
            _ => continue,
        };
        if out.insert(name.to_owned(), m).is_some() {
            return Err(format!("OPERATORS.md lists {name} twice"));
        }
    }
    Ok(out)
}

fn cell(j: &J) -> Cell {
    match j.as_str() {
        Some("Y") => Cell::Y,
        Some("N") | None => Cell::N,
        Some(_) => Cell::Raised,
    }
}

fn toml_prop(t: &toml::Table, key: &str, extra_flags: &[&str]) -> Res<Prop> {
    // Builds a dump-shaped JSON prop from a TOML table and normalises it with an empty context.
    let ty = t
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{key}: no type"))?;
    let ident = t
        .get("identifier")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("{key}: no identifier"))?;
    let tj = |v: &toml::Value| serde_json::to_value(v).unwrap_or(J::Null);
    let mut flags: Vec<String> = extra_flags.iter().map(|f| f.to_string()).collect();
    if t.get("required").and_then(|v| v.as_bool()) == Some(true) {
        flags.push("REQUIRED".into());
    }
    if t.get("flag").and_then(|v| v.as_bool()) == Some(true) {
        flags.push("ENUM_FLAG".into());
    }
    let mut p = serde_json::json!({
        "identifier": ident,
        "name": t.get("name").and_then(|v| v.as_str()).unwrap_or(ident),
        "description": t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "type": ty, "subtype": "NONE", "unit": "NONE", "flags": flags,
    });
    let o = p.as_object_mut().expect("invariant: object literal");
    // `array = [3]` and `subtype = "XYZ"`: Python-only vector members (`Bone.vector`, M5+M6
    // stabilization).
    if let Some(a) = t.get("array") {
        o.insert("array".into(), tj(a));
    }
    if let Some(st) = t.get("subtype") {
        o.insert("subtype".into(), tj(st));
    }
    let dflt = t.get("default").map(tj);
    match ty {
        "BOOLEAN" => {
            o.insert("default".into(), dflt.unwrap_or(J::Bool(false)));
        }
        "INT" => {
            o.insert("default".into(), dflt.unwrap_or(J::from(0)));
            for (k, v) in [
                ("hard_min", i32::MIN as i64),
                ("hard_max", i32::MAX as i64),
                ("soft_min", i32::MIN as i64),
                ("soft_max", i32::MAX as i64),
            ] {
                o.insert(k.into(), J::from(v));
            }
        }
        "FLOAT" => {
            o.insert("default".into(), dflt.unwrap_or(J::from(0.0)));
            for (k, v) in [
                ("hard_min", -f32::MAX as f64),
                ("hard_max", f32::MAX as f64),
                ("soft_min", -f32::MAX as f64),
                ("soft_max", f32::MAX as f64),
            ] {
                o.insert(k.into(), J::from(v));
            }
            o.insert("precision".into(), J::from(3));
        }
        "STRING" => {
            o.insert("default".into(), dflt.unwrap_or(J::from("")));
            o.insert("length_max".into(), J::from(0));
        }
        "ENUM" => {
            let items: Vec<J> = match t.get("items").and_then(|v| v.as_array()) {
                Some(a) => a
                    .iter()
                    .enumerate()
                    .map(|(i, it)| match it {
                        toml::Value::String(id) => {
                            serde_json::json!({"identifier": id, "name": id, "description": "", "value": i})
                        }
                        toml::Value::Array(row) => {
                            let st = |k: usize| row.get(k).and_then(|x| x.as_str()).unwrap_or("");
                            serde_json::json!({"identifier": st(0), "name": st(1), "description": st(2), "value": i})
                        }
                        _ => J::Null,
                    })
                    .collect(),
                None => vec![],
            };
            o.insert("items".into(), J::Array(items));
            o.insert("default".into(), dflt.unwrap_or(J::from("")));
        }
        "POINTER" | "COLLECTION" => {
            o.insert(
                "fixed_type".into(),
                J::from(
                    t.get("fixed_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("AnyType"),
                ),
            );
            o.insert("is_id".into(), J::Bool(false));
        }
        other => return Err(format!("{key}: unsupported overlay type {other}")),
    }
    let mut cx = Ctx {
        dynamic: BTreeMap::new(),
        f64_props: BTreeSet::new(),
        raw_items: BTreeMap::new(),
        used_dynamic: BTreeSet::new(),
    };
    cx.prop(&p, key)
}

/// Loads everything.
pub fn load(root: &Path) -> Res<Spec> {
    let spec_dir = root.join("spec/blender-5.2");
    let ov = root.join("spec/overlay");
    let milestone_text = std::fs::read_to_string(root.join("spec/current_milestone"))
        .map_err(|e| format!("spec/current_milestone: {e}"))?;
    let milestone: u32 = milestone_text
        .trim()
        .trim_start_matches('M')
        .parse()
        .map_err(|_| {
            format!(
                "spec/current_milestone: bad value `{}`",
                milestone_text.trim()
            )
        })?;

    // ---- overlays
    let dyn_t = read_toml(&ov.join("dynamic_enums.toml"))?;
    let mut dynamic = BTreeMap::new();
    for (k, v) in &dyn_t {
        let t = v
            .as_table()
            .ok_or_else(|| format!("dynamic_enums.toml: `{k}` is not a table"))?;
        dynamic.insert(
            k.clone(),
            DynEntry {
                items: t.get("items").map(toml_items),
                items_from: t
                    .get("items_from")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned),
                runtime: t.get("runtime").and_then(|v| v.as_bool()) == Some(true),
                default: t.get("default").and_then(|v| v.as_str()).map(str::to_owned),
            },
        );
    }
    let fs_t = read_toml(&ov.join("float_storage.toml"))?;
    let f64_props: BTreeSet<String> = fs_t
        .get("f64")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    // ---- raw dumps
    let rna_meta = read_json(&spec_dir.join("rna/_meta.json"))?;
    let struct_names: Vec<String> = rna_meta["structs"]
        .as_array()
        .ok_or("rna/_meta.json: no structs")?
        .iter()
        .map(s)
        .collect();
    let mut raw_structs = BTreeMap::new();
    for n in &struct_names {
        raw_structs.insert(
            n.clone(),
            read_json(&spec_dir.join(format!("rna/{n}.json")))?,
        );
    }
    let ops_meta = read_json(&spec_dir.join("ops/_meta.json"))?;
    let mut raw_ops: Vec<(String, J)> = Vec::new();
    for ns in ops_meta["namespaces"].as_array().ok_or("ops/_meta.json")? {
        let ns = s(ns);
        let d = read_json(&spec_dir.join(format!("ops/{ns}.json")))?;
        for op in d["operators"].as_array().ok_or("ops: no operators")? {
            raw_ops.push((ns.clone(), op.clone()));
        }
    }
    let frap_dir = root.join("spec/frap/ops");
    let mut frap_files: Vec<PathBuf> = std::fs::read_dir(&frap_dir)
        .map_err(|e| format!("{}: {e}", frap_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    frap_files.sort();
    for f in frap_files {
        let d = read_json(&f)?;
        let ns = s(&d["namespace"]);
        for op in d["operators"].as_array().ok_or("frap ops: no operators")? {
            raw_ops.push((ns.clone(), op.clone()));
        }
    }

    // ---- raw item index for `items_from`
    let mut raw_items = BTreeMap::new();
    for (n, d) in &raw_structs {
        for p in d["props"].as_array().into_iter().flatten() {
            if p["type"] == "ENUM" {
                raw_items.insert(
                    format!("rna:{n}.{}", s(&p["identifier"])),
                    items_of(&p["items"]),
                );
            }
        }
    }
    for (_, op) in &raw_ops {
        for p in op["props"].as_array().into_iter().flatten() {
            if p["type"] == "ENUM" {
                raw_items.insert(
                    format!("op:{}.{}", s(&op["bl_idname"]), s(&p["identifier"])),
                    items_of(&p["items"]),
                );
            }
        }
    }
    let mut cx = Ctx {
        dynamic,
        f64_props,
        raw_items,
        used_dynamic: BTreeSet::new(),
    };
    let dynamic_overlay_keys: BTreeSet<String> = cx.dynamic.keys().cloned().collect();

    // ---- python-only overlay
    let py = read_toml(&ov.join("python_only.toml"))?;
    let mut py_props: BTreeMap<String, Vec<Prop>> = BTreeMap::new();
    let mut python_only_props = Vec::new();
    for t in py
        .get("prop")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let t = t
            .as_table()
            .ok_or("python_only.toml: [[prop]] is not a table")?;
        let st = t.get("struct").and_then(|v| v.as_str()).unwrap_or("");
        let id = t.get("identifier").and_then(|v| v.as_str()).unwrap_or("");
        let p = toml_prop(t, &format!("{st}.{id}"), &["PYTHON_ONLY", "READONLY"])?;
        python_only_props.push((st.to_owned(), id.to_owned()));
        py_props.entry(st.to_owned()).or_default().push(p);
    }
    let mut py_funcs: BTreeMap<String, Vec<Func>> = BTreeMap::new();
    for t in py
        .get("func")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let t = t
            .as_table()
            .ok_or("python_only.toml: [[func]] is not a table")?;
        let st = t.get("struct").and_then(|v| v.as_str()).unwrap_or("");
        let id = t.get("identifier").and_then(|v| v.as_str()).unwrap_or("");
        let key = format!("{st}.{id}");
        let list = |k: &str, out: bool| -> Res<Vec<Prop>> {
            let mut v = Vec::new();
            for p in t.get(k).and_then(|v| v.as_array()).into_iter().flatten() {
                let pt = p.as_table().ok_or_else(|| format!("{key}: bad {k}"))?;
                let mut extra = vec![];
                if out {
                    extra.push("OUTPUT");
                } else if pt.get("required").and_then(|v| v.as_bool()) != Some(true) {
                    extra.push("ARGUMENT_OPTIONAL");
                }
                v.push(toml_prop(pt, &key, &extra)?);
            }
            Ok(v)
        };
        py_funcs.entry(st.to_owned()).or_default().push(Func {
            identifier: id.to_owned(),
            description: t
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            params: list("params", false)?,
            returns: list("returns", true)?,
            keyword_only_from: t
                .get("keyword_only_from")
                .and_then(|v| v.as_integer())
                .map(|x| x as u8),
            flags: vec!["PYTHON_ONLY".into()],
        });
    }

    // ---- context members overlay
    let cm = read_toml(&ov.join("context_members.toml"))?;
    let cm_over = cm
        .get("members")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    // ---- structs
    let mut structs = BTreeMap::new();
    for (n, d) in &raw_structs {
        let mut props = Vec::new();
        for p in d["props"].as_array().into_iter().flatten() {
            props.push(cx.prop(p, &format!("{n}.{}", s(&p["identifier"])))?);
        }
        if let Some(members) = d.get("context_members").and_then(J::as_array) {
            let rna: BTreeSet<String> = props.iter().map(|p| p.identifier.clone()).collect();
            let mut seen = BTreeSet::new();
            let mut synth = |id: &str, kind: &str, ty: &str| -> Res<()> {
                if rna.contains(id) || !seen.insert(id.to_owned()) {
                    return Ok(());
                }
                let is_id = raw_structs
                    .get(ty)
                    .is_some_and(|d| d["is_id"].as_bool() == Some(true))
                    || ID_TYPES.contains(&ty);
                let kind = match kind {
                    "collection" => Kind::Coll {
                        item_type: ty.to_owned(),
                        api: None,
                    },
                    "pointer" => Kind::Pointer {
                        fixed_type: ty.to_owned(),
                        is_id,
                    },
                    "string" => Kind::Str {
                        default: String::new(),
                        max_bytes: 0,
                    },
                    other => return Err(format!("Context.{id}: unknown member kind `{other}`")),
                };
                props.push(Prop {
                    identifier: id.to_owned(),
                    name: id.to_owned(),
                    description: "Context member (C context callback, not an RNA property)"
                        .to_owned(),
                    kind,
                    subtype: "NONE".into(),
                    unit: "NONE".into(),
                    array: Shape::Scalar,
                    flags: vec!["READONLY".into(), "CONTEXT_MEMBER".into()],
                    deprecated: None,
                });
                Ok(())
            };
            for m in members {
                let id = s(&m["identifier"]);
                let (kind, ty) = match cm_over.get(&id).and_then(|v| v.as_table()) {
                    Some(o) => (
                        o.get("kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("pointer")
                            .to_owned(),
                        o.get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("AnyType")
                            .to_owned(),
                    ),
                    None => {
                        let kind = s(&m["kind"]);
                        if kind == "unknown" {
                            return Err(format!(
                                "Context.{id}: type never seen; add it to spec/overlay/context_members.toml"
                            ));
                        }
                        let ty = m["types"]
                            .as_array()
                            .and_then(|a| a.first())
                            .map(s)
                            .unwrap_or_else(|| "AnyType".into());
                        (kind, ty)
                    }
                };
                synth(&id, &kind, &ty)?;
            }
            for (id, o) in &cm_over {
                let o = o.as_table().ok_or("context_members.toml: bad entry")?;
                synth(
                    id,
                    o.get("kind").and_then(|v| v.as_str()).unwrap_or("pointer"),
                    o.get("type").and_then(|v| v.as_str()).unwrap_or("AnyType"),
                )?;
            }
        }
        if let Some(extra) = py_props.remove(n) {
            props.extend(extra);
        }
        let mut seen = BTreeSet::new();
        for p in &props {
            if !seen.insert(p.identifier.clone()) {
                return Err(format!("{n}: duplicate property `{}`", p.identifier));
            }
        }
        let mut funcs = Vec::new();
        for f in d["funcs"].as_array().into_iter().flatten() {
            let fid = s(&f["identifier"]);
            let key = format!("{n}.{fid}()");
            let mut flags = Vec::new();
            if f["use_self"] == J::Bool(false) {
                flags.push("NO_SELF".to_owned());
            }
            if f["use_self_type"] == J::Bool(true) {
                flags.push("USE_SELF_TYPE".to_owned());
            }
            funcs.push(Func {
                identifier: fid,
                description: s(&f["description"]),
                params: f["params"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| cx.prop(p, &key))
                    .collect::<Res<_>>()?,
                returns: f["returns"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| cx.prop(p, &key))
                    .collect::<Res<_>>()?,
                keyword_only_from: None,
                flags,
            });
        }
        if let Some(extra) = py_funcs.remove(n) {
            funcs.extend(extra);
        }
        structs.insert(
            n.clone(),
            Struct {
                identifier: n.clone(),
                name: s(&d["name"]),
                description: s(&d["description"]),
                base: d["base"].as_str().map(str::to_owned),
                name_property: d["name_property"].as_str().map(str::to_owned),
                is_id: d["is_id"].as_bool().unwrap_or(false),
                props,
                funcs,
                fresh: d
                    .get("fresh")
                    .and_then(|f| f.get("differs_from_default"))
                    .and_then(J::as_object)
                    .cloned(),
            },
        );
    }
    let bpy_struct_funcs = py_funcs.remove("bpy_struct").unwrap_or_default();
    if let Some((k, _)) = py_props.into_iter().next() {
        return Err(format!("python_only.toml: struct `{k}` is not dumped"));
    }
    if let Some((k, _)) = py_funcs.into_iter().next() {
        return Err(format!("python_only.toml: struct `{k}` is not dumped"));
    }
    for st in structs.values() {
        if let Some(b) = &st.base
            && !structs.contains_key(b)
        {
            return Err(format!("{}: base `{b}` is not dumped", st.identifier));
        }
    }

    // ---- milestones
    let md = std::fs::read_to_string(root.join("docs/OPERATORS.md"))
        .map_err(|e| format!("docs/OPERATORS.md: {e}"))?;
    let milestones = parse_operators_md(&md)?;

    // ---- frap options
    let fo = read_toml(&ov.join("frap_options.toml"))?;
    let frap_features: BTreeSet<String> = fo
        .get("features")
        .and_then(|v| v.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();
    let mut frap_covered = BTreeMap::new();
    for (op, t) in fo
        .get("covered")
        .and_then(|v| v.as_table())
        .into_iter()
        .flatten()
    {
        let m: BTreeMap<String, String> = t
            .as_table()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
            .collect();
        frap_covered.insert(op.clone(), m);
    }
    let mut frap_options = Vec::new();
    for t in fo
        .get("option")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let t = t.as_table().ok_or("frap_options.toml: bad [[option]]")?;
        let op = t
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let id = t
            .get("identifier")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        frap_options.push(FrapOption {
            prop: toml_prop(t, &format!("{op}.{id}"), &[])?,
            op,
            identifier: id,
            feature: t
                .get("feature")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
            milestone: t.get("milestone").and_then(|v| v.as_integer()).unwrap_or(0) as u32,
        });
    }

    // ---- operators
    let mut ops = Vec::new();
    let mut seen_ops = BTreeSet::new();
    for (ns, op) in &raw_ops {
        let idname = s(&op["idname"]);
        let bl = s(&op["bl_idname"]);
        if !seen_ops.insert(idname.clone()) {
            return Err(format!("operator {idname} dumped twice"));
        }
        let mut props = Vec::new();
        for p in op["props"].as_array().into_iter().flatten() {
            props.push(cx.prop(p, &format!("{bl}.{}", s(&p["identifier"])))?);
        }
        for o in &frap_options {
            if o.op == idname && o.milestone <= milestone {
                props.push(o.prop.clone());
            }
        }
        let poll = match op.get("poll") {
            Some(J::Object(m)) => {
                let row = |sfx: &str| -> [Cell; 5] {
                    ["A", "B", "C", "D", "E"]
                        .map(|c| cell(&m.get(&format!("{c}{sfx}")).cloned().unwrap_or(J::Null)))
                };
                Some([row(""), row("V")])
            }
            _ => None,
        };
        ops.push(Op {
            group: group_of(ns)?,
            ns: ns.clone(),
            milestone: milestones.get(&idname).copied().flatten(),
            idname,
            bl_idname: bl,
            label: s(&op["label"]),
            description: s(&op["description"]),
            options: op["options"]
                .as_array()
                .map(|a| a.iter().map(s).collect())
                .unwrap_or_default(),
            props,
            macro_steps: op["macro_steps"]
                .as_array()
                .map(|a| a.iter().map(s).collect())
                .unwrap_or_default(),
            poll,
        });
    }
    ops.sort_by(|a, b| a.idname.cmp(&b.idname));
    for (name, m) in &milestones {
        // frap.* operators of later milestones get their spec from the WP implementing them.
        let later_frap = name.starts_with("frap.") && m.is_none_or(|m| m > milestone);
        if !seen_ops.contains(name) && !later_frap {
            return Err(format!(
                "OPERATORS.md lists {name}, which is neither dumped nor in spec/frap/ops/"
            ));
        }
    }

    // ---- compat
    let ct = read_toml(&ov.join("compat_4x.toml"))?;
    let mut compat = Vec::new();
    for t in ct
        .get("member")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let t = t.as_table().ok_or("compat_4x.toml: bad [[member]]")?;
        let g = |k: &str| t.get(k).and_then(|v| v.as_str()).unwrap_or("").to_owned();
        compat.push(CompatMember {
            struct_: g("struct"),
            identifier: g("identifier"),
            kind: g("kind"),
        });
    }

    // ---- nodes and modifiers
    let nmeta = read_json(&spec_dir.join("nodes/_meta.json"))?;
    let mut nodes = Vec::new();
    for n in nmeta["node_types"].as_array().ok_or("nodes/_meta.json")? {
        nodes.push(read_json(&spec_dir.join(format!("nodes/{}.json", s(n))))?);
    }
    nodes.sort_by(|a, b| s(&a["bl_idname"]).cmp(&s(&b["bl_idname"])));
    let mmeta = read_json(&spec_dir.join("modifiers/_meta.json"))?;
    let mut modifiers = Vec::new();
    for t in mmeta["types"].as_array().ok_or("modifiers/_meta.json")? {
        modifiers.push(read_json(
            &spec_dir.join(format!("modifiers/{}.json", s(t))),
        )?);
    }
    let modifier_types = match structs
        .get("Modifier")
        .and_then(|m| m.props.iter().find(|p| p.identifier == "type"))
    {
        Some(Prop {
            kind: Kind::Enum { items, .. },
            ..
        }) => items.clone(),
        _ => return Err("Modifier.type is not a dumped enum".into()),
    };

    Ok(Spec {
        root: root.to_path_buf(),
        milestone,
        structs,
        bpy_struct_funcs,
        ops,
        frap_options,
        frap_features,
        frap_covered,
        compat,
        python_only_props,
        nodes,
        modifiers,
        modifier_types,
        rna_meta,
        dynamic_overlay_keys,
        used_dynamic_overlay_keys: cx.used_dynamic,
    })
}

/// ID types frap knows by name (context members whose struct was not dumped).
const ID_TYPES: &[&str] = &[
    "ID",
    "Object",
    "Mesh",
    "Material",
    "Camera",
    "Light",
    "World",
    "Image",
    "Armature",
    "Action",
    "Scene",
    "Collection",
    "Key",
    "NodeTree",
    "ShaderNodeTree",
    "Texture",
    "Library",
    "PointCloud",
    "GreasePencil",
    "Annotation",
];

impl Spec {
    /// Operators that get a typed props struct (OPERATORS.md `M` ≤ `spec/current_milestone`).
    pub fn is_typed(&self, op: &Op) -> bool {
        op.milestone.is_some_and(|m| m <= self.milestone)
    }
    /// Walks the base chain (self first).
    pub fn chain(&self, ident: &str) -> Vec<&Struct> {
        let mut out = Vec::new();
        let mut cur = self.structs.get(ident);
        while let Some(st) = cur {
            out.push(st);
            cur = st.base.as_deref().and_then(|b| self.structs.get(b));
        }
        out
    }
}
