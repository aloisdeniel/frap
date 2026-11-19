// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe` manifest: operators, structs, context members, modifier and node types,
//! `defaults_that_surprise` (§14). WP 1.18.
//!
//! Everything is built from the generated `frap-spec-*` tables plus the operator registry
//! (`frap_op::OPERATORS`), so it cannot drift: an operator Blender has but frap does not
//! implement yet is listed with `frap.status = "not_implemented"`, its dumped properties and its
//! dumped poll matrix, so an agent can tell "not in Blender" (absent) from "not yet in frap".
//!
//! Entry points: [`describe`] (every `--describe` target), [`manifest`], [`describe_op`] /
//! [`describe_entry`] (one operator), [`describe_struct`], [`describe_struct_prop`].

mod curated;
mod ops;
mod planned;
mod props;
mod target;

use frap_op::{OpKind, OperatorDef, Support};
use frap_rna::{FuncDef, FuncFlags, OpOptions, OperatorSpec, PollCell, PropFlags, StructDef};
use serde_json::{Map, Value as J, json};

pub use curated::{
    DEFAULTS_THAT_SURPRISE, POLL_CONTEXTS, PRIMITIVES_APPLYING_SCALE, PRIMITIVES_IGNORING_SCALE,
    SurpriseDefault, SurpriseKind, unset_semantics,
};
pub use ops::{OpEntry, all_ops, find_op, glob_match, namespaces};
pub use planned::{PlannedRow, operator_milestone, row as planned_row};
pub use props::{
    PropOwner, array_json, default_json, describe_prop, float_json, poll_req_name, poll_req_text,
    ranges, unit_label,
};
pub use target::{DESCRIBE_TARGET, DescribeError, DescribeOptions, Target, describe};

/// The Blender API frap mirrors.
pub const BLENDER_API: &str = "5.2";

/// The `--describe op:<idname>` fragment of one registered operator.
pub fn describe_op(def: &OperatorDef) -> J {
    describe_spec(def.spec, Some(def))
}

/// The manifest entry of one operator of the universe.
pub fn describe_entry(e: &OpEntry) -> J {
    describe_spec(e.spec, e.def)
}

fn cells(c: &[PollCell; 5]) -> String {
    c.iter()
        .map(|c| match c {
            PollCell::Y => 'Y',
            PollCell::N => 'N',
            PollCell::Raised => 'X',
        })
        .collect()
}

fn matrix_text(spec: &OperatorSpec) -> String {
    let Some(m) = spec.poll_matrix else {
        return "frap operator: no Blender poll".into();
    };
    let pass: Vec<String> = POLL_CONTEXTS
        .iter()
        .enumerate()
        .filter_map(|(i, (k, text))| {
            let plain = m.plain[i] == PollCell::Y;
            let view = m.view3d[i] == PollCell::Y;
            match (plain, view) {
                (true, _) => Some(format!("{k} ({text})")),
                (false, true) => Some(format!("{k} ({text}, with a 3D View)")),
                _ => None,
            }
        })
        .collect();
    if pass.is_empty() {
        "Blender 5.2's poll fails in every dumped context (A–E): it needs data or an editor these contexts lack".into()
    } else {
        format!("Blender 5.2's poll passes in: {}", pass.join("; "))
    }
}

fn poll_json(spec: &OperatorSpec, def: Option<&OperatorDef>) -> J {
    let matrix = spec.poll_matrix.map_or(
        J::Null,
        |m| json!({"plain": cells(&m.plain), "view3d": cells(&m.view3d)}),
    );
    match def {
        Some(d) => {
            let requires: Vec<String> = d.poll.requires.iter().map(poll_req_name).collect();
            let text = if d.poll.requires.is_empty() {
                "always available".to_owned()
            } else {
                format!(
                    "requires {}",
                    d.poll
                        .requires
                        .iter()
                        .map(poll_req_text)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            json!({"requires": requires, "text": text, "matrix": matrix})
        }
        None => json!({"requires": J::Null, "text": matrix_text(spec), "matrix": matrix}),
    }
}

fn frap_json(spec: &OperatorSpec, def: Option<&OperatorDef>) -> J {
    let planned = operator_milestone(spec.idname);
    match def {
        Some(d) => {
            let (support, note, headless) = match d.support {
                Support::Full => ("full", None, "native"),
                Support::Partial(n) => ("partial", Some(n), "native"),
                Support::HeadlessExtended(n) => ("full", Some(n), "extended"),
                _ => ("partial", None, "native"),
            };
            json!({
                "status": "implemented",
                "tier": format!("{:?}", d.tier),
                "support": support,
                "support_note": note,
                "headless": headless,
                "planned": planned,
                "extension": spec.idname.starts_with("frap."),
                "notes": d.notes,
            })
        }
        None => json!({
            "status": "not_implemented",
            "tier": J::Null,
            "support": J::Null,
            "support_note": J::Null,
            "headless": J::Null,
            "planned": planned,
            "extension": spec.idname.starts_with("frap."),
            "notes": [if planned == "OUT" {
                format!("{} exists in Blender 5.2 but is out of frap's scope (OP_NOT_IMPLEMENTED)", spec.idname)
            } else {
                format!("{} exists in Blender 5.2; frap implements it in {planned} (OP_NOT_IMPLEMENTED until then)", spec.idname)
            }],
        }),
    }
}

/// The manifest entry of an operator spec, with its implementation when registered.
pub fn describe_spec(spec: &'static OperatorSpec, def: Option<&OperatorDef>) -> J {
    let is_macro = match def {
        Some(d) => d.kind == OpKind::Macro,
        None => spec.options.contains(OpOptions::MACRO) || !spec.macro_steps.is_empty(),
    };
    let properties: Vec<J> = spec
        .props
        .iter()
        .map(|p| describe_prop(p, PropOwner::Op { spec, def }))
        .collect();
    json!({
        "idname": spec.idname,
        "bl_idname": spec.bl_idname,
        "label": spec.label,
        "description": spec.description,
        "bl_options": spec.options.names(),
        "kind": if is_macro { "MACRO" } else { "NORMAL" },
        "macro_steps": spec.macro_steps,
        "poll": poll_json(spec, def),
        "properties": properties,
        "frap": frap_json(spec, def),
    })
}

fn func_json(f: &FuncDef) -> J {
    json!({
        "identifier": f.identifier,
        "description": f.description,
        "parameters": f.params.iter().map(|p| describe_prop(p, PropOwner::Param)).collect::<Vec<_>>(),
        "returns": f.returns.iter().map(|p| describe_prop(p, PropOwner::Param)).collect::<Vec<_>>(),
        "keyword_only_from": f.keyword_only_from,
        "flags": f.flags.iter_names().map(|(n, _)| n).collect::<Vec<_>>(),
        "python_only": f.flags.contains(FuncFlags::PYTHON_ONLY),
    })
}

/// One `structs[]` entry: own properties and functions (inherited members live in `base`),
/// plus the identifiers of Python-only members (`spec/overlay/python_only.toml`).
pub fn describe_struct(s: &'static StructDef) -> J {
    let python_only: Vec<&str> = s
        .props
        .iter()
        .filter(|p| p.flags.contains(PropFlags::PYTHON_ONLY))
        .map(|p| p.identifier)
        .chain(
            s.funcs
                .iter()
                .filter(|f| f.flags.contains(FuncFlags::PYTHON_ONLY))
                .map(|f| f.identifier),
        )
        .collect();
    json!({
        "identifier": s.identifier,
        "name": s.name,
        "description": s.description,
        "base": s.base.map(|b| b.identifier),
        "is_id": s.is_id,
        "name_property": s.name_property,
        "properties": s.props.iter().map(|p| describe_prop(p, PropOwner::Struct)).collect::<Vec<_>>(),
        "functions": s.funcs.iter().map(func_json).collect::<Vec<_>>(),
        "python_only_members": python_only,
    })
}

/// `<Struct>.<prop>`: the property entry (found through the base chain) with its owner.
pub fn describe_struct_prop(s: &'static StructDef, prop: &str) -> Option<J> {
    if let Some(p) = s.find_prop(prop) {
        let mut v = describe_prop(p, PropOwner::Struct);
        let owner = owner_of(s, |d| d.props.iter().any(|q| std::ptr::eq(q, p)));
        v.as_object_mut()?
            .insert("struct".into(), J::from(owner.identifier));
        return Some(v);
    }
    let f = s.find_func(prop).or_else(|| {
        frap_spec_rna::BPY_STRUCT_FUNCS
            .iter()
            .find(|f| f.identifier == prop)
    })?;
    let mut v = func_json(f);
    let owner = owner_of(s, |d| d.funcs.iter().any(|g| std::ptr::eq(g, f)));
    let owner_name = if owner.funcs.iter().any(|g| std::ptr::eq(g, f)) {
        owner.identifier
    } else {
        "bpy_struct"
    };
    v.as_object_mut()?
        .insert("struct".into(), J::from(owner_name));
    v.as_object_mut()?
        .insert("kind".into(), J::from("function"));
    Some(v)
}

fn owner_of(s: &'static StructDef, has: impl Fn(&StructDef) -> bool) -> &'static StructDef {
    let mut cur = Some(s);
    while let Some(d) = cur {
        if has(d) {
            return d;
        }
        cur = d.base;
    }
    s
}

/// `context_members[]`: every `bpy.context` member (RNA properties of `Context` and the
/// C-callback members, F-DEF-5).
pub fn context_members() -> J {
    J::Array(
        frap_spec_rna::CONTEXT
            .props
            .iter()
            .map(|p| {
                let (kind, ty) = match &p.kind {
                    frap_rna::PropKind::Pointer { fixed_type, .. } => ("pointer", *fixed_type),
                    frap_rna::PropKind::Collection { item_type, .. } => ("collection", *item_type),
                    _ => ("value", p.type_str()),
                };
                json!({
                    "identifier": p.identifier,
                    "kind": kind,
                    "type": ty,
                    "callback_member": p.flags.contains(PropFlags::CONTEXT_MEMBER),
                    "description": p.description,
                })
            })
            .collect(),
    )
}

/// `modifier_types[]`: every `Modifier.type`, its struct and evaluator support. Types without an
/// evaluator are data only: the stack passes the mesh through with a WARNING (§7.2).
pub fn modifier_types() -> J {
    use frap_scene::generated::modifier_settings::ModifierType;
    // The types registered in `frap_modifiers::MODIFIER_TYPES` (WP 4.7, M34W1-10).
    let evaluated = frap_modifiers::registry::evaluated_types();
    J::Array(
        ModifierType::ALL
            .iter()
            .map(|t| {
                let row = planned_row(t.identifier()).filter(|r| r.section == 'B');
                json!({
                    "identifier": t.identifier(),
                    "name": t.ui_name(),
                    "struct": row.and_then(|r| r.second),
                    "evaluator": evaluated.binary_search(t).is_ok(),
                    "planned": row.map_or("OUT", |r| r.milestone),
                })
            })
            .collect(),
    )
}

fn socket_type(t: frap_nodes::SocketType) -> String {
    let d = format!("{t:?}");
    match d.as_str() {
        "Rgba" => "RGBA".into(),
        "Value" => "VALUE".into(),
        _ => d.to_ascii_uppercase(),
    }
}

/// `node_types[]`: every shader node type frap can create, with its sockets.
pub fn node_types() -> J {
    let sock = |s: &frap_nodes::generated::SocketDef| json!({"identifier": s.identifier, "name": s.name, "type": socket_type(s.ty), "bl_idname": s.bl_idname});
    J::Array(
        frap_nodes::generated::NODE_TYPES
            .iter()
            .map(|n| {
                let row = planned_row(n.bl_idname).filter(|r| r.section == 'C');
                json!({
                    "bl_idname": n.bl_idname,
                    "name": n.name,
                    "struct": n.srna.identifier,
                    "inputs": n.inputs.iter().map(sock).collect::<Vec<_>>(),
                    "outputs": n.outputs.iter().map(sock).collect::<Vec<_>>(),
                    "planned": row.map_or("OUT", |r| r.milestone),
                })
            })
            .collect(),
    )
}

/// `defaults_that_surprise[]`.
pub fn defaults_that_surprise() -> J {
    J::Array(
        DEFAULTS_THAT_SURPRISE
            .iter()
            .map(|s| {
                json!({
                    "target": s.target,
                    "kind": match s.kind {
                        SurpriseKind::OperatorProperty => "operator_property",
                        SurpriseKind::Data => "data",
                    },
                    "default": serde_json::from_str::<J>(s.default_json).unwrap_or(J::Null),
                    "why": s.why,
                })
            })
            .collect(),
    )
}

/// `--describe=errors`: every registered error code (the `ERROR_CODES` slice of every crate
/// linked into the binary), sorted by code.
pub fn errors() -> J {
    let mut v: Vec<&frap_rna::ErrorCodeDoc> = frap_rna::ERROR_CODES.iter().collect();
    v.sort_by_key(|d| d.code.0);
    v.dedup_by_key(|d| d.code.0);
    json!({
        "errors": v.iter().map(|d| json!({
            "code": d.code.0,
            "exception": d.exception.as_str(),
            "summary": d.summary,
        })).collect::<Vec<_>>(),
    })
}

/// The operators selected by `opts`, sorted by idname.
pub fn selected_ops(opts: &DescribeOptions) -> impl Iterator<Item = &'static OpEntry> + '_ {
    all_ops().iter().filter(move |e| {
        (!opts.implemented_only || e.implemented())
            && opts
                .filter
                .as_deref()
                .is_none_or(|f| glob_match(f, e.spec.idname))
    })
}

/// The whole manifest (`frap --describe`). With a `filter`, only the selected operators are
/// listed (the struct, context, modifier and node sections are left out).
pub fn manifest(opts: &DescribeOptions) -> J {
    let ops: Vec<&OpEntry> = selected_ops(opts).collect();
    let implemented = ops.iter().filter(|e| e.implemented()).count();
    let mut m = Map::new();
    m.insert(
        "frap".into(),
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "blender_api": BLENDER_API,
            "protocol": 1,
        }),
    );
    m.insert(
        "counts".into(),
        json!({
            "operators": ops.len(),
            "implemented": implemented,
            "not_implemented": ops.len() - implemented,
        }),
    );
    m.insert(
        "poll_contexts".into(),
        J::Object(
            POLL_CONTEXTS
                .iter()
                .map(|(k, v)| ((*k).to_owned(), J::from(*v)))
                .collect(),
        ),
    );
    m.insert(
        "operators".into(),
        J::Array(ops.iter().map(|e| describe_entry(e)).collect()),
    );
    if opts.filter.is_none() {
        m.insert(
            "structs".into(),
            J::Array(
                frap_spec_rna::ALL_STRUCTS
                    .iter()
                    .map(|s| describe_struct(s))
                    .collect(),
            ),
        );
        m.insert(
            "bpy_struct_functions".into(),
            J::Array(
                frap_spec_rna::BPY_STRUCT_FUNCS
                    .iter()
                    .map(func_json)
                    .collect(),
            ),
        );
        m.insert("context_members".into(), context_members());
        m.insert("modifier_types".into(), modifier_types());
        m.insert("node_types".into(), node_types());
        m.insert("defaults_that_surprise".into(), defaults_that_surprise());
    }
    J::Object(m)
}
