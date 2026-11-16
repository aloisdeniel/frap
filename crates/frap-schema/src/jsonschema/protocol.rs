// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe=protocol`: JSON Schemas of the stream protocol (§13.1–§13.4), generated with
//! `schemars` from mirror types (the static part of `--describe`, §14). `frap-session` (same
//! layer) cannot be named here; `tests/protocol.rs` runs a real session and validates every
//! response against these schemas, so the mirror cannot drift from `Response::to_json`.

#![allow(dead_code)] // the types exist only for their JSON Schema

use schemars::JsonSchema;
use serde_json::{Map, Value as J};

/// A serve/MCP request id: string or number, echoed verbatim.
#[derive(JsonSchema)]
#[serde(untagged)]
enum RequestId {
    Str(String),
    Num(f64),
}

/// What happens to the rest of a request after a failing statement.
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
enum OnError {
    /// Answer every remaining statement with status SKIPPED (default).
    SkipRest,
    /// Run the remaining statements.
    Continue,
}

/// `frap serve --jsonl`: one request per input line. `src` is parsed as one complete unit, so frap never waits for more input.
#[derive(JsonSchema)]
#[schemars(title = "frap serve request", deny_unknown_fields)]
struct Request {
    /// Echoed on every response of the request and on its done record.
    id: RequestId,
    /// One or more statements (bpy-line or short form), separated by newlines or `;`.
    src: String,
    /// Batch error policy (default `skip_rest`).
    #[serde(default)]
    on_error: Option<OnError>,
}

/// Statement kind.
#[derive(JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Kind {
    /// `bpy.ops.ns.op(...)`
    Op,
    /// `bpy.ops.ns.op.poll()`, `.idname()`, `.get_rna_type()`
    OpMeta,
    /// A path read.
    Get,
    /// An assignment.
    Set,
    /// An RNA function call.
    Call,
    /// `print(...)`
    Print,
    /// An empty statement or comment.
    Noop,
    /// Input that never became a statement (syntax error, invalid UTF-8).
    Invalid,
}

/// Statement outcome.
#[derive(JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Status {
    /// Committed.
    Finished,
    /// Rolled back by the operator (Blender's WARNING reports say why).
    Cancelled,
    /// Rolled back on error (`error` says why).
    Error,
    /// Not run: an earlier statement of the same serve/MCP request failed (`on_error: skip_rest`).
    Skipped,
}

/// Report level (Blender's `ReportType`).
#[derive(JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ReportLevel {
    Info,
    Warning,
    Error,
    Operator,
    Property,
}

/// A Blender-style report, wording copied from Blender.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct Report {
    #[serde(rename = "type")]
    level: ReportLevel,
    message: String,
}

/// `{from, to}` repr paths of a renamed datablock.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct Renamed {
    from: String,
    to: String,
}

/// Datablocks touched by a committed mutation, as repr paths (`bpy.data.objects['Cube']`).
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct Changes {
    added: Vec<String>,
    removed: Vec<String>,
    modified: Vec<String>,
    renamed: Vec<Renamed>,
}

/// Always `"unchanged"`: a failed, cancelled or skipped statement never changes the state.
#[derive(JsonSchema)]
enum Unchanged {
    #[serde(rename = "unchanged")]
    Unchanged,
}

/// Source span of an error. `col` and `len` count UTF-8 bytes, `col_chars` code points.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct Span {
    line: u32,
    col: u32,
    col_chars: u32,
    len: u32,
}

/// Python exception class the error corresponds to.
#[allow(clippy::enum_variant_names)] // Python's class names
#[derive(JsonSchema)]
enum Exception {
    SyntaxError,
    NameError,
    TypeError,
    ValueError,
    KeyError,
    IndexError,
    AttributeError,
    RuntimeError,
    NotImplementedError,
}

/// The error of an `ERROR` response (§13.4). `message` is Blender's wording where Blender has one; `detail` carries `did_you_mean`, `valid`, `operator`, `argument`, `requires`, `unmet`.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct Error {
    exception: Exception,
    /// Stable error code (`frap --describe=errors`).
    code: String,
    message: String,
    span: Option<Span>,
    detail: Map<String, J>,
    /// The exact fix, when frap knows it (`bpy.ops.object.mode_set(mode='EDIT')`).
    hint: Option<String>,
}

/// RNA type tag of a value.
#[derive(JsonSchema)]
struct Rna {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    subtype: Option<String>,
    /// `[n]` or `[rows, cols]`.
    #[serde(default)]
    array: Option<Vec<u32>>,
    /// `Vector`, `Euler`, `Quaternion`, `AxisAngle`, `Matrix`, `Color`, `Array`.
    #[serde(default)]
    math: Option<String>,
    /// Euler order.
    #[serde(default)]
    order: Option<String>,
}

/// Context after the statement (absent with `--verbosity minimal`).
#[derive(JsonSchema)]
struct Context {
    /// `OBJECT`, `EDIT_MESH`, …
    mode: String,
    /// Active object name.
    active_object: Option<String>,
    /// Selected object names, in base order.
    selected_objects: Vec<Option<String>>,
    frame_current: i64,
    /// Edit mode only.
    #[serde(default)]
    edit_object: Option<String>,
    /// Edit mode only: `[vertex, edge, face]` flags.
    #[serde(default)]
    select_mode: Option<J>,
}

/// frap help attached to a response (`NO_OP_DEFAULT`, `ANGLE_LIKELY_DEGREES`, …).
#[derive(JsonSchema)]
struct Hint {
    code: String,
    message: String,
}

/// One response per statement (§13.2). Keys appear in this order; optional keys are omitted when they do not apply.
#[derive(JsonSchema)]
#[schemars(title = "frap response", deny_unknown_fields)]
struct Response {
    /// serve/MCP: the request's id.
    #[serde(default)]
    id: Option<RequestId>,
    /// 1-based statement counter (per session).
    seq: u64,
    /// Source line of the statement's first token.
    line: u32,
    kind: Kind,
    /// Operator idname (kind `op`/`op_meta`).
    #[serde(default)]
    op: Option<String>,
    /// True iff status is FINISHED.
    ok: bool,
    status: Status,
    /// Resolved repr path (get/set).
    #[serde(default)]
    path: Option<String>,
    /// Function path (call).
    #[serde(default)]
    func: Option<String>,
    /// get/call/print value; set: the assigned value after coercion (§13.3 encoding).
    #[serde(default)]
    value: Option<J>,
    #[serde(default)]
    rna: Option<Rna>,
    reports: Vec<Report>,
    /// Operator results (`object`, `objects`, `removed`, `filepath`, …).
    #[serde(default)]
    result: Option<Map<String, J>>,
    #[serde(default)]
    changes: Option<Changes>,
    /// CANCELLED, ERROR, SKIPPED.
    #[serde(default)]
    state: Option<Unchanged>,
    #[serde(default)]
    error: Option<Error>,
    #[serde(default)]
    context: Option<Context>,
    /// Per object in Edit Mode: counts, selection, active element.
    #[serde(default)]
    edit_mesh: Option<J>,
    hints: Vec<Hint>,
}

/// `{"event": "ready", …}`: the first line of raw mode (disable with `--no-ready`).
#[derive(JsonSchema)]
#[schemars(title = "frap ready event")]
struct Ready {
    event: ReadyTag,
    frap: String,
    protocol: u32,
    blender_api: String,
    /// `factory`, `empty` or a file path.
    startup: String,
}
#[derive(JsonSchema)]
enum ReadyTag {
    #[serde(rename = "ready")]
    Ready,
}

#[derive(JsonSchema)]
struct EndStats {
    statements: u64,
    errors: u64,
    cancelled: u64,
}

/// `{"event": "end", …}` at EOF (disable with `--no-end-event`).
#[derive(JsonSchema)]
#[schemars(title = "frap end event")]
struct End {
    event: EndTag,
    ok: bool,
    stats: EndStats,
}
#[derive(JsonSchema)]
enum EndTag {
    #[serde(rename = "end")]
    End,
}

#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct DoneCounts {
    finished: u32,
    error: u32,
    cancelled: u32,
    skipped: u32,
}

/// The last line of every serve/MCP request.
#[derive(JsonSchema)]
#[schemars(title = "frap done record", deny_unknown_fields)]
struct Done {
    id: RequestId,
    event: DoneTag,
    /// True iff every statement finished.
    ok: bool,
    counts: DoneCounts,
}
#[derive(JsonSchema)]
enum DoneTag {
    #[serde(rename = "done")]
    Done,
}

/// Value of the `.frap` manifest's `format`.
#[derive(JsonSchema)]
enum FrapFormat {
    #[serde(rename = "frap")]
    Frap,
}

/// One `buffers/<hash>.bin` entry of a `.frap`.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct FrapBufferEntry {
    /// sha256 of the content, first 32 lowercase hex characters.
    #[schemars(pattern("^[0-9a-f]{32}$"))]
    hash: String,
    /// Uncompressed size in bytes.
    size: u64,
}

/// One `images/<hash>.<ext>` entry of a `.frap`.
#[derive(JsonSchema)]
#[schemars(deny_unknown_fields)]
struct FrapImageEntry {
    /// sha256 of the encoded bytes, first 32 lowercase hex characters.
    #[schemars(pattern("^[0-9a-f]{32}$"))]
    hash: String,
    /// Lowercase file extension without the dot (`png`, `jpg`, `exr`, …).
    #[schemars(pattern("^[0-9a-z]{1,8}$"))]
    ext: String,
    /// Size in bytes.
    size: u64,
}

/// `manifest.json` of a `.frap` project (§15; mirror of `frap_project::manifest::Manifest`).
/// Unknown fields fail the load unless `--lenient`; a newer `schema_version` is refused.
#[derive(JsonSchema)]
#[schemars(title = "frap project manifest.json", deny_unknown_fields)]
struct FrapManifest {
    format: FrapFormat,
    /// Integer project schema version (1 in this build); older files are migrated on load.
    #[schemars(range(min = 1))]
    schema_version: u32,
    /// The Blender Python API whose RNA identifiers `scene.json` uses (`"5.2"`).
    blender_api: String,
    /// `"frap <version>"`.
    generator: String,
    /// Every `buffers/<hash>.bin` entry, in archive order.
    buffers: Vec<FrapBufferEntry>,
    /// Every `images/<hash>.<ext>` entry, in archive order.
    images: Vec<FrapImageEntry>,
}

fn root<T: JsonSchema>() -> J {
    schemars::generate::SchemaSettings::draft2020_12()
        .with(|s| s.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<T>()
        .to_value()
}

/// `--describe=protocol`.
pub fn protocol() -> J {
    serde_json::json!({
        "protocol": 1,
        "request": root::<Request>(),
        "response": root::<Response>(),
        "error": root::<Error>(),
        "events": {
            "ready": root::<Ready>(),
            "end": root::<End>(),
            "done": root::<Done>(),
        },
        "frap_project_manifest": root::<FrapManifest>(),
    })
}
