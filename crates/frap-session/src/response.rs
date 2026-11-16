// SPDX-License-Identifier: GPL-3.0-or-later
//! Response envelope and value encoding (ARCHITECTURE §13.2, §13.3; CONTRACT draft).

use frap_rna::ptr::IdNames;
use frap_rna::{ArrayShape, FrapError, PropDef, Root, Value};
use frap_rna::{Report, ReportType};
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

/// Statement outcome.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Committed.
    Finished,
    /// Rolled back by the operator.
    Cancelled,
    /// Rolled back on error.
    Error,
    /// Not run (serve/MCP `on_error: "skip_rest"`).
    Skipped,
}

impl Status {
    /// Protocol string.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Finished => "FINISHED",
            Status::Cancelled => "CANCELLED",
            Status::Error => "ERROR",
            Status::Skipped => "SKIPPED",
        }
    }
}

/// One response per statement (ARCHITECTURE §13.2; the JSON shape is a CONTRACT frozen at
/// M1 W0).
#[derive(Clone, Debug)]
pub struct Response {
    /// The serve/MCP request's `id`, echoed on every response of the request.
    pub id: Option<J>,
    /// 1-based statement counter (per session).
    pub seq: u64,
    /// Source line of the statement's first token.
    pub line: u32,
    /// `op`, `get`, `set`, `noop`, ...
    pub kind: &'static str,
    /// Operator idname (kind `op`).
    pub op: Option<SmolStr>,
    /// Outcome.
    pub status: Status,
    /// Resolved repr path (get/set).
    pub path: Option<String>,
    /// Function path (kind `call`).
    pub func: Option<String>,
    /// Value (get, call, print; set: the assigned value after coercion, §13.2).
    pub value: Option<J>,
    /// RNA type tag of `value`.
    pub rna: Option<J>,
    /// Blender-style reports.
    pub reports: Vec<Report>,
    /// Operator results.
    pub result: Option<Map<String, J>>,
    /// Change set (mutations that committed).
    pub changes: Option<J>,
    /// Error (status ERROR).
    pub error: Option<FrapError>,
    /// Context after the statement (`None` with `--verbosity minimal`, §13.1).
    pub context: Option<J>,
    /// Per object in Edit Mode: counts, selection, active element (§13.2).
    pub edit_mesh: Option<J>,
    /// frap help.
    pub hints: Vec<J>,
}

impl Response {
    /// `true` iff FINISHED.
    pub fn ok(&self) -> bool {
        self.status == Status::Finished
    }

    /// The JSON object, keys in protocol order.
    pub fn to_json(&self) -> J {
        let mut m = Map::new();
        if let Some(id) = &self.id {
            m.insert("id".into(), id.clone());
        }
        m.insert("seq".into(), J::from(self.seq));
        m.insert("line".into(), J::from(self.line));
        m.insert("kind".into(), J::from(self.kind));
        if let Some(op) = &self.op {
            m.insert("op".into(), J::from(op.as_str()));
        }
        m.insert("ok".into(), J::from(self.ok()));
        m.insert("status".into(), J::from(self.status.as_str()));
        if let Some(p) = &self.path {
            m.insert("path".into(), J::from(p.as_str()));
        }
        if let Some(f) = &self.func {
            m.insert("func".into(), J::from(f.as_str()));
        }
        if let Some(v) = &self.value {
            m.insert("value".into(), v.clone());
        }
        if let Some(r) = &self.rna {
            m.insert("rna".into(), r.clone());
        }
        m.insert(
            "reports".into(),
            J::Array(
                self.reports
                    .iter()
                    .map(|r| json!({"type": r.level.as_str(), "message": r.message}))
                    .collect(),
            ),
        );
        if let Some(r) = &self.result {
            m.insert("result".into(), J::Object(r.clone()));
        }
        if let Some(c) = &self.changes {
            m.insert("changes".into(), c.clone());
        }
        if self.status != Status::Finished {
            m.insert("state".into(), J::from("unchanged"));
        }
        if let Some(e) = &self.error {
            m.insert("error".into(), error_json(e));
        }
        if let Some(c) = &self.context {
            m.insert("context".into(), c.clone());
        }
        if let Some(em) = &self.edit_mesh {
            m.insert("edit_mesh".into(), em.clone());
        }
        m.insert("hints".into(), J::Array(self.hints.clone()));
        J::Object(m)
    }
}

fn error_json(e: &FrapError) -> J {
    let mut m = Map::new();
    m.insert("exception".into(), J::from(e.exception.as_str()));
    m.insert("code".into(), J::from(e.code.0));
    m.insert("message".into(), J::from(e.message.as_str()));
    m.insert(
        "span".into(),
        match e.span {
            Some(s) => {
                json!({"line": s.line, "col": s.col, "col_chars": s.col_chars, "len": s.len})
            }
            None => J::Null,
        },
    );
    m.insert("detail".into(), J::Object(e.detail.clone()));
    m.insert(
        "hint".into(),
        e.hint.as_deref().map(J::from).unwrap_or(J::Null),
    );
    J::Object(m)
}

/// Shortest round-trip f32 (`0.1`, not `0.10000000149011612`); integral values as integers;
/// non-finite values tagged (§13.3).
pub fn f32_json(x: f32) -> J {
    if x.is_nan() {
        return json!({"$float": "nan"});
    }
    if x.is_infinite() {
        return json!({"$float": if x > 0.0 { "inf" } else { "-inf" }});
    }
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        return J::from(x as i64);
    }
    // Rust's Display is the shortest string that round-trips the f32; as f64 it prints the same.
    let shortest: f64 = format!("{x}")
        .parse()
        .expect("invariant: float Display parses");
    J::from(shortest)
}

/// Encodes a value (§13.3).
pub fn value_json(v: &Value, names: &dyn IdNames) -> J {
    match v {
        Value::None => J::Null,
        Value::Bool(b) => J::from(*b),
        Value::Int(i) => J::from(*i),
        Value::Float(f) => f32_json(*f),
        Value::Double(d) => serde_json::Number::from_f64(*d)
            .map(J::Number)
            .unwrap_or(J::Null),
        Value::Str(s) | Value::Enum(s) => J::from(s.as_str()),
        Value::EnumSet(s) => J::Array(s.iter().map(|x| J::from(x.as_str())).collect()),
        Value::BoolArray(a) => J::Array(a.iter().map(|b| J::from(*b)).collect()),
        Value::IntArray(a) => J::Array(a.iter().map(|i| J::from(*i)).collect()),
        Value::FloatArray { data, shape, .. } => match shape {
            ArrayShape::Matrix(r, c) => J::Array(
                (0..*r as usize)
                    .map(|i| {
                        J::Array(
                            (0..*c as usize)
                                .map(|j| f32_json(data[i * *c as usize + j]))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
            _ => J::Array(data.iter().map(|f| f32_json(*f)).collect()),
        },
        Value::Id(None) => J::Null,
        Value::Id(Some(id)) => match names.id_name(*id) {
            Some((n, lib)) => json!({"$ref": frap_rna::ptr::id_repr(id.code, &n, lib.as_deref())}),
            None => J::Null,
        },
        Value::Structs(items) => J::Array(
            items
                .iter()
                .map(|m| {
                    J::Object(
                        m.iter()
                            .map(|(k, v)| (k.to_string(), value_json(v, names)))
                            .collect(),
                    )
                })
                .collect(),
        ),
        Value::Ptr(p) => {
            if matches!(p.root, Root::Id(_)) && p.path.is_empty() {
                json!({"$ref": p.repr(names)})
            } else {
                json!({"$ptr": p.repr(names), "type": p.srna.identifier})
            }
        }
        Value::Coll(p) => json!({"$coll": p.repr(names)}),
        Value::Tuple(t) => J::Array(t.iter().map(|x| value_json(x, names)).collect()),
        _ => J::Null,
    }
}

/// `rna` tag of a property value.
pub fn rna_json(prop: &PropDef, v: &Value) -> J {
    let mut m = Map::new();
    m.insert("type".into(), J::from(prop.type_str()));
    m.insert("subtype".into(), J::from(prop.subtype.as_str()));
    match prop.array {
        ArrayShape::Fixed(n) => {
            m.insert("array".into(), json!([n]));
        }
        ArrayShape::Matrix(r, c) => {
            m.insert("array".into(), json!([r, c]));
        }
        _ => {}
    }
    if let Value::FloatArray { tag, .. } = v {
        m.insert("math".into(), J::from(tag.as_str()));
    }
    J::Object(m)
}

/// INFO report helper.
pub fn info(msg: impl Into<String>) -> Report {
    Report::new(ReportType::Info, msg)
}
