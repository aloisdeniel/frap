// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator specs (ARCHITECTURE §4.2, §10.4):
//! * `crates/frap-spec-ops-<group>/src/generated/<ns>.rs`: `OperatorSpec` statics and one typed
//!   props struct per operator whose OPERATORS.md milestone is ≤ `spec/current_milestone`;
//! * `crates/frap-spec-catalog/src/generated/catalog.json.deflate`: every other operator as
//!   deflated JSON, decoded lazily by `frap-spec-catalog`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Write as _;

use serde_json::{Value as J, json};

use super::Files;
use super::model::{Cell, Def, EnumDef, GROUPS, Kind, Op, Prop, Res, Shape, Spec};
use super::rust::{EnumPool, camel, flags_lit, header, ident, lit_str, prop_lit};

pub fn group_dir(group: &str) -> String {
    format!("crates/frap-spec-ops-{group}/src/generated")
}
pub const CATALOG_DIR: &str = "crates/frap-spec-catalog/src/generated";

fn cell_lit(c: Cell) -> &'static str {
    match c {
        Cell::Y => "PollCell::Y",
        Cell::N => "PollCell::N",
        Cell::Raised => "PollCell::Raised",
    }
}

fn cell_str(c: Cell) -> &'static str {
    match c {
        Cell::Y => "Y",
        Cell::N => "N",
        Cell::Raised => "X",
    }
}

struct EnumGen {
    type_name: String,
    /// (variant, identifier)
    variants: Vec<(String, String)>,
    default_variant: String,
}

fn variant_name(id: &str) -> String {
    let v = camel(id);
    if v.is_empty() || !v.starts_with(|c: char| c.is_ascii_alphabetic()) {
        format!("V{v}")
    } else {
        v
    }
}

/// Typed field: (Rust type, nested macro args?).
fn field_type(p: &Prop, op_mod: &str, enums: &mut Vec<EnumGen>) -> (String, bool) {
    let t = match (&p.kind, p.array) {
        (Kind::Bool(_), Shape::Scalar) => "bool".into(),
        (Kind::Bool(_), Shape::Fixed(n)) => format!("[bool; {n}]"),
        (Kind::Int { .. }, Shape::Scalar) => "i32".into(),
        (Kind::Int { .. }, Shape::Fixed(n)) => format!("[i32; {n}]"),
        (Kind::Float { .. }, Shape::Scalar) => "f32".into(),
        (Kind::Float { .. }, Shape::Fixed(n)) => format!("[f32; {n}]"),
        (Kind::Float { .. }, Shape::Matrix(r, c)) => format!("[[f32; {c}]; {r}]"),
        (Kind::Str { .. }, _) => "SmolStr".into(),
        (Kind::Enum { .. }, _) if p.has_flag("ENUM_FLAG") => "BTreeSet<SmolStr>".into(),
        (
            Kind::Enum {
                items,
                dynamic: None,
                default,
                ..
            },
            _,
        ) if !items.is_empty() => {
            let type_name = camel(&p.identifier);
            let mut seen = BTreeSet::new();
            let mut variants = Vec::new();
            for it in items {
                let mut v = variant_name(&it.id);
                if !seen.insert(v.clone()) {
                    let mut k = 2;
                    while !seen.insert(format!("{v}{k}")) {
                        k += 1;
                    }
                    v = format!("{v}{k}");
                }
                variants.push((v, it.id.clone()));
            }
            let d = match default {
                EnumDef::Item(d) => d.clone(),
                EnumDef::Flags(_) => String::new(),
            };
            let default_variant = variants
                .iter()
                .find(|(_, id)| *id == d)
                .unwrap_or(&variants[0])
                .0
                .clone();
            enums.push(EnumGen {
                type_name: type_name.clone(),
                variants,
                default_variant,
            });
            format!("{op_mod}::{type_name}")
        }
        (Kind::Enum { .. }, _) => "SmolStr".into(),
        (Kind::Pointer { fixed_type, .. }, _) if fixed_type.contains("_OT_") => {
            return ("OpArgs".into(), true);
        }
        _ => "Value".into(),
    };
    (t, false)
}

fn describe_default(p: &Prop) -> String {
    match &p.kind {
        Kind::Bool(Def::Scalar(b)) => format!("default {b}"),
        Kind::Int {
            default: Def::Scalar(v),
            ..
        } => format!("default {v}"),
        Kind::Float {
            default: Def::Scalar(v),
            ..
        } => format!("default {v}"),
        Kind::Float {
            default: Def::Array(a),
            ..
        } => format!("default {a:?}"),
        Kind::Str { default, .. } => format!("default {default:?}"),
        Kind::Enum {
            items,
            default: EnumDef::Item(d),
            dynamic,
            ..
        } => {
            let ids: Vec<&str> = items.iter().map(|i| i.id.as_str()).take(8).collect();
            let more = if items.len() > 8 { " | …" } else { "" };
            format!(
                "default {d:?}{}: {}{more}",
                if dynamic.is_some() { ", dynamic" } else { "" },
                ids.join(" | ")
            )
        }
        Kind::Enum {
            default: EnumDef::Flags(f),
            ..
        } => format!("default {f:?}"),
        _ => p.type_str().to_owned(),
    }
}

fn spec_static(op: &Op) -> String {
    format!("OPSPEC_{}", op.bl_idname.to_ascii_uppercase())
}

fn op_name(op: &Op) -> &str {
    op.idname.split_once('.').map(|x| x.1).unwrap_or(&op.idname)
}

/// One namespace file of a typed ops crate.
/// Typed struct name: `camel(op)` (`PrimitiveCubeAdd`, §10.4), or `camel(ns_op)` when another
/// dumped operator of the same ops group has the same name (`export_scene.gltf` /
/// `import_scene.gltf` → `ExportSceneGltf` / `ImportSceneGltf`). Computed over every dumped
/// operator of the group, so promoting an operator never renames another one's struct.
pub fn struct_name(spec: &Spec, op: &Op) -> String {
    let name = op_name(op);
    let clash = spec
        .ops
        .iter()
        .filter(|o| o.group == op.group && op_name(o) == name)
        .count()
        > 1;
    if clash {
        camel(&format!("{}_{name}", op.ns))
    } else {
        camel(name)
    }
}

fn ns_file(spec: &Spec, ns: &str, ops: &[&Op]) -> Res<String> {
    let mut pool = EnumPool::default();
    let mut body = String::new();
    let mut struct_names = BTreeSet::new();
    for op in ops {
        let name = op_name(op);
        let upname = name.to_ascii_uppercase();
        let _ = writeln!(
            body,
            "// ---------------------------------------------------------------- {}",
            op.idname
        );
        let _ = writeln!(body, "/// Props of `{}`, RNA order.", op.idname);
        let _ = writeln!(
            body,
            "pub static {upname}_PROPS: [PropDef; {}] = [",
            op.props.len()
        );
        for p in &op.props {
            let _ = writeln!(body, "    {},", prop_lit(p, &mut pool, "enums")?);
        }
        body.push_str("];\n");
        let poll = match &op.poll {
            Some([plain, v3d]) => format!(
                "Some(PollMatrix {{ plain: [{}], view3d: [{}] }})",
                plain
                    .iter()
                    .map(|c| cell_lit(*c))
                    .collect::<Vec<_>>()
                    .join(", "),
                v3d.iter()
                    .map(|c| cell_lit(*c))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            None => "None".into(),
        };
        let _ = writeln!(
            body,
            "/// `{}`\npub static {}: OperatorSpec = OperatorSpec {{\n    idname: {},\n    bl_idname: {},\n    label: {},\n    description: {},\n    options: {},\n    props: &{upname}_PROPS,\n    macro_steps: &[{}],\n    poll_matrix: {poll},\n}};",
            op.bl_idname,
            spec_static(op),
            lit_str(&op.idname),
            lit_str(&op.bl_idname),
            lit_str(&op.label),
            lit_str(&op.description),
            flags_lit("OpOptions", &op.options),
            op.macro_steps
                .iter()
                .map(|s| lit_str(s))
                .collect::<Vec<_>>()
                .join(", "),
        );

        let op_mod = ident(name);
        let mut enums = Vec::new();
        let mut fields = Vec::new();
        for (i, p) in op.props.iter().enumerate() {
            let (t, nested) = field_type(p, &op_mod, &mut enums);
            fields.push((ident(&p.identifier), t, nested, i, p));
        }
        if !enums.is_empty() {
            let _ = writeln!(body, "/// Enums of `{}`.\npub mod {op_mod} {{", op.idname);
            for e in &enums {
                let _ = writeln!(
                    body,
                    "    /// `{}`\n    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n    pub enum {} {{",
                    e.type_name, e.type_name
                );
                for (v, id) in &e.variants {
                    let _ = writeln!(body, "        /// {id:?}\n        {v},");
                }
                body.push_str("    }\n");
                let _ = writeln!(
                    body,
                    "    impl {} {{\n        /// Every variant, in RNA item order.\n        pub const ALL: [{}; {}] = [{}];\n        /// Blender identifier.\n        pub const fn identifier(self) -> &'static str {{\n            match self {{",
                    e.type_name,
                    e.type_name,
                    e.variants.len(),
                    e.variants
                        .iter()
                        .map(|(v, _)| format!("{}::{v}", e.type_name))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                for (v, id) in &e.variants {
                    let _ = writeln!(
                        body,
                        "                {}::{v} => {},",
                        e.type_name,
                        lit_str(id)
                    );
                }
                let _ = writeln!(
                    body,
                    "            }}\n        }}\n        /// Parses a Blender identifier.\n        pub fn from_identifier(s: &str) -> Option<Self> {{\n            match s {{"
                );
                for (v, id) in &e.variants {
                    let _ = writeln!(
                        body,
                        "                {} => Some({}::{v}),",
                        lit_str(id),
                        e.type_name
                    );
                }
                let _ = writeln!(
                    body,
                    "                _ => None,\n            }}\n        }}\n    }}\n    impl Default for {} {{\n        fn default() -> Self {{\n            {}::{}\n        }}\n    }}\n    impl frap_rna::FromValue for {} {{\n        /// Infallible after `coerce`; an unknown identifier (a spec mismatch) yields the default.\n        fn from_value(v: &frap_rna::Value) -> Self {{\n            v.as_str().and_then(Self::from_identifier).unwrap_or_default()\n        }}\n    }}",
                    e.type_name, e.type_name, e.default_variant, e.type_name
                );
            }
            body.push_str("}\n");
        }
        let sname = struct_name(spec, op);
        if !struct_names.insert(sname.clone()) {
            return Err(format!(
                "{}: typed struct name collision `{sname}`",
                op.idname
            ));
        }
        let _ = writeln!(
            body,
            "/// Typed props of `{}` ({}).\n#[derive(Debug, Clone)]\npub struct {sname} {{",
            op.idname, op.label
        );
        for (f, t, nested, _, p) in &fields {
            let mut doc = p.description.replace('\n', " ");
            if !doc.is_empty() {
                doc.push_str("; ");
            }
            doc.push_str(&describe_default(p));
            if !p.flags.is_empty() {
                let _ = write!(doc, " [{}]", p.flags.join(", "));
            }
            if *nested {
                let _ = writeln!(
                    body,
                    "    /// Macro step `{}`: its arguments.\n    pub {f}: {t},",
                    p.identifier
                );
            } else {
                let _ = writeln!(body, "    /// {doc}\n    pub {f}: Prop<{t}>,");
            }
        }
        body.push_str("}\n");
        let _ = writeln!(
            body,
            "impl OpProps for {sname} {{\n    const IDNAME: &'static str = {};\n    const BL_IDNAME: &'static str = {};\n    fn spec() -> &'static OperatorSpec {{\n        &{}\n    }}\n    fn from_args(a: &OpArgs) -> Self {{\n        {sname} {{",
            lit_str(&op.idname),
            lit_str(&op.bl_idname),
            spec_static(op)
        );
        for (f, _, nested, i, p) in &fields {
            if *nested {
                let _ = writeln!(
                    body,
                    "            {f}: a.nested.get({}).cloned().unwrap_or_default(),",
                    lit_str(&p.identifier)
                );
            } else {
                let _ = writeln!(
                    body,
                    "            {f}: a.prop({i}, {}),",
                    lit_str(&p.identifier)
                );
            }
        }
        body.push_str("        }\n    }\n}\n\n");
    }
    let mut s = header(&format!(
        "`bpy.ops.{ns}.*` operators with OPERATORS.md milestone ≤ spec/current_milestone: `OperatorSpec`\n//! statics and typed props structs (ARCHITECTURE §4.2, §10.4)."
    ));
    s.push_str("#![allow(non_snake_case, non_upper_case_globals, unused_imports, unused_variables, clippy::all)]\n");
    s.push_str("use std::collections::BTreeSet;\n\nuse frap_rna::{ArrayShape, Defaults, Deprecation, EnumDefault, FloatStorage, OpArgs, OpOptions, OpProps, OperatorSpec, PollCell, PollMatrix, Prop, PropDef, PropFlags, PropKind, Subtype, Unit, Value};\nuse smol_str::SmolStr;\n\n");
    s.push_str("/// Enum item tables of this namespace (deduplicated).\npub mod enums {\n    use frap_rna::EnumItem;\n");
    s.push_str(&pool.statics());
    s.push_str("}\n\n");
    s.push_str(&body);
    Ok(s)
}

pub fn generate_typed(spec: &Spec, files: &mut Files) -> Res<()> {
    for group in GROUPS {
        let dir = spec.root.join(group_dir(group));
        let mut by_ns: BTreeMap<&str, Vec<&Op>> = BTreeMap::new();
        for op in spec
            .ops
            .iter()
            .filter(|o| o.group == group && spec.is_typed(o))
        {
            by_ns.entry(op.ns.as_str()).or_default().push(op);
        }
        let mut m = header(&format!(
            "`frap-spec-ops-{group}` generated modules, one per Blender namespace (§4.2)."
        ));
        m.push_str("#![allow(clippy::all)]\n\nuse frap_rna::OperatorSpec;\n\n");
        let mut all = Vec::new();
        for (ns, ops) in &by_ns {
            let _ = writeln!(m, "pub mod {};", ident(ns));
            files.insert(
                dir.join(format!("{ns}.rs")),
                ns_file(spec, ns, ops)?.into_bytes(),
            );
            for op in ops {
                all.push(format!("&{}::{}", ident(ns), spec_static(op)));
            }
        }
        let _ = writeln!(
            m,
            "\n/// Every typed operator spec of this group, sorted by idname.\npub static ALL_OPS: [&OperatorSpec; {}] = [{}];",
            all.len(),
            if all.is_empty() {
                String::new()
            } else {
                format!("\n    {},\n", all.join(",\n    "))
            }
        );
        files.insert(dir.join("mod.rs"), m.into_bytes());
    }
    Ok(())
}

fn jf(v: f64) -> J {
    if v.is_finite() {
        J::from(v)
    } else if v.is_nan() {
        json!({"$float": "nan"})
    } else if v > 0.0 {
        json!({"$float": "inf"})
    } else {
        json!({"$float": "-inf"})
    }
}

/// Catalog JSON of one property (decoded by `frap-spec-catalog/src/decode.rs`).
pub fn prop_json(p: &Prop) -> J {
    let mut o = serde_json::Map::new();
    o.insert("id".into(), J::from(p.identifier.clone()));
    o.insert("name".into(), J::from(p.name.clone()));
    o.insert("desc".into(), J::from(p.description.clone()));
    o.insert("type".into(), J::from(p.type_str()));
    o.insert("subtype".into(), J::from(p.subtype.clone()));
    o.insert("unit".into(), J::from(p.unit.clone()));
    o.insert(
        "array".into(),
        match p.array {
            Shape::Scalar => J::Null,
            Shape::Fixed(n) => json!([n]),
            Shape::Matrix(r, c) => json!([r, c]),
            Shape::Dynamic => J::from("dynamic"),
        },
    );
    o.insert("flags".into(), json!(p.flags));
    match &p.kind {
        Kind::Bool(d) => {
            o.insert(
                "default".into(),
                match d {
                    Def::Scalar(b) => J::from(*b),
                    Def::Array(a) => json!(a),
                },
            );
        }
        Kind::Int {
            default,
            hard,
            soft,
        } => {
            o.insert(
                "default".into(),
                match default {
                    Def::Scalar(v) => J::from(*v),
                    Def::Array(a) => json!(a),
                },
            );
            o.insert("hard".into(), json!([hard.0, hard.1]));
            o.insert("soft".into(), json!([soft.0, soft.1]));
        }
        Kind::Float {
            default,
            hard,
            soft,
            precision,
            f64_storage,
        } => {
            o.insert(
                "default".into(),
                match default {
                    Def::Scalar(v) => jf(*v),
                    Def::Array(a) => J::Array(a.iter().map(|v| jf(*v)).collect()),
                },
            );
            o.insert("hard".into(), J::Array(vec![jf(hard.0), jf(hard.1)]));
            o.insert("soft".into(), J::Array(vec![jf(soft.0), jf(soft.1)]));
            o.insert("precision".into(), J::from(*precision));
            o.insert(
                "storage".into(),
                J::from(if *f64_storage { "F64" } else { "F32" }),
            );
        }
        Kind::Str { default, max_bytes } => {
            o.insert("default".into(), J::from(default.clone()));
            o.insert("max_bytes".into(), J::from(*max_bytes));
        }
        Kind::Enum {
            items,
            default,
            dynamic,
            ..
        } => {
            o.insert(
                "items".into(),
                J::Array(
                    items
                        .iter()
                        .map(|i| json!([i.id, i.name, i.desc, i.value]))
                        .collect(),
                ),
            );
            o.insert(
                "default".into(),
                match default {
                    EnumDef::Item(d) => J::from(d.clone()),
                    EnumDef::Flags(f) => json!(f),
                },
            );
            o.insert("dynamic".into(), json!(dynamic));
        }
        Kind::Pointer { fixed_type, is_id } => {
            o.insert("fixed_type".into(), J::from(fixed_type.clone()));
            o.insert("is_id".into(), J::from(*is_id));
        }
        Kind::Coll { item_type, api } => {
            o.insert("item_type".into(), J::from(item_type.clone()));
            o.insert("api".into(), json!(api));
        }
    }
    if let Some(d) = &p.deprecated {
        o.insert(
            "deprecated".into(),
            json!({"note": d.note, "version": d.version, "removal_version": d.removal_version}),
        );
    }
    J::Object(o)
}

fn op_json(op: &Op) -> J {
    json!({
        "idname": op.idname,
        "bl_idname": op.bl_idname,
        "label": op.label,
        "description": op.description,
        "options": op.options,
        "props": op.props.iter().map(prop_json).collect::<Vec<_>>(),
        "macro_steps": op.macro_steps,
        "poll": op.poll.map(|[a, b]| json!({
            "plain": a.iter().map(|c| cell_str(*c)).collect::<Vec<_>>(),
            "view3d": b.iter().map(|c| cell_str(*c)).collect::<Vec<_>>(),
        })),
    })
}

/// The catalog JSON text (uncompressed).
pub fn catalog_json(spec: &Spec) -> String {
    let ops: Vec<J> = spec
        .ops
        .iter()
        .filter(|o| !spec.is_typed(o))
        .map(op_json)
        .collect();
    let mut s = serde_json::to_string(&J::Array(ops)).expect("invariant: JSON values serialize");
    s.push('\n');
    s
}

pub fn deflate(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(data).expect("invariant: writing to a Vec");
    e.finish().expect("invariant: writing to a Vec")
}

pub fn inflate(data: &[u8]) -> Res<Vec<u8>> {
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::DeflateDecoder::new(data)
        .read_to_end(&mut out)
        .map_err(|e| format!("inflate: {e}"))?;
    Ok(out)
}

pub fn generate_catalog(spec: &Spec, files: &mut Files) -> Res<()> {
    let dir = spec.root.join(CATALOG_DIR);
    let json = catalog_json(spec);
    files.insert(dir.join("catalog.json.deflate"), deflate(json.as_bytes()));
    let catalog: Vec<&Op> = spec.ops.iter().filter(|o| !spec.is_typed(o)).collect();
    let typed: Vec<&Op> = spec.ops.iter().filter(|o| spec.is_typed(o)).collect();
    let mut m = header(
        "The operator catalog blob (every dumped operator without a typed props struct) and its index.",
    );
    m.push_str("#![allow(clippy::all)]\n\n");
    m.push_str("/// Deflate-compressed JSON array of the catalog operators (schema: `decode.rs`).\npub static CATALOG_DEFLATE: &[u8] = include_bytes!(\"catalog.json.deflate\");\n");
    let _ = writeln!(
        m,
        "/// Number of operators in the blob.\npub const CATALOG_LEN: usize = {};\n/// Their idnames, sorted (membership tests without decoding).\npub static CATALOG_IDNAMES: [&str; {}] = [\n{}];\n/// Operators with a typed props struct in `frap-spec-ops-<group>` (milestone ≤ {}), sorted;\n/// their specs are NOT in the blob.\npub static TYPED_IDNAMES: [&str; {}] = [\n{}];",
        catalog.len(),
        catalog.len(),
        catalog
            .iter()
            .map(|o| format!("    {},\n", lit_str(&o.idname)))
            .collect::<String>(),
        spec.milestone,
        typed.len(),
        typed
            .iter()
            .map(|o| format!("    {},\n", lit_str(&o.idname)))
            .collect::<String>(),
    );
    m.push_str("\n/// The catalog spec of `idname` (`\"mesh.bevel\"`), if it is a catalog operator (decodes the\n/// blob on first use; the address is stable for the process).\npub fn find(idname: &str) -> Option<&'static frap_rna::OperatorSpec> {\n    let c = crate::catalog();\n    c.binary_search_by(|s| s.idname.cmp(idname)).ok().map(|i| c[i])\n}\n\n/// True when `idname` is a catalog operator (no decoding).\npub fn contains(idname: &str) -> bool {\n    CATALOG_IDNAMES.binary_search(&idname).is_ok()\n}\n\n/// True when `idname` has a typed props struct in `frap-spec-ops-<group>`.\npub fn is_typed(idname: &str) -> bool {\n    TYPED_IDNAMES.binary_search(&idname).is_ok()\n}\n");
    files.insert(dir.join("mod.rs"), m.into_bytes());
    Ok(())
}
