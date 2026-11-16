// SPDX-License-Identifier: GPL-3.0-or-later
//! Errors and reports (ARCHITECTURE §13.4, CONTRACT frozen at M1 W0; `Report` moved here from
//! frap-op by RFC 0001 F-LAYER-1).

use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value as Json};

/// Python exception class reported to the client.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize)]
#[non_exhaustive]
pub enum PyExc {
    /// SyntaxError
    SyntaxError,
    /// NameError
    NameError,
    /// TypeError
    TypeError,
    /// ValueError
    ValueError,
    /// KeyError
    KeyError,
    /// IndexError
    IndexError,
    /// AttributeError
    AttributeError,
    /// RuntimeError
    RuntimeError,
    /// NotImplementedError
    NotImplementedError,
}

impl PyExc {
    /// Class name.
    pub fn as_str(self) -> &'static str {
        match self {
            PyExc::SyntaxError => "SyntaxError",
            PyExc::NameError => "NameError",
            PyExc::TypeError => "TypeError",
            PyExc::ValueError => "ValueError",
            PyExc::KeyError => "KeyError",
            PyExc::IndexError => "IndexError",
            PyExc::AttributeError => "AttributeError",
            PyExc::RuntimeError => "RuntimeError",
            PyExc::NotImplementedError => "NotImplementedError",
        }
    }
}

/// Stable machine-readable error code. Constants live in the crate that raises them.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ErrorCode(pub &'static str);

impl ErrorCode {
    /// Panic caught by the session.
    pub const INTERNAL: ErrorCode = ErrorCode("INTERNAL");
}

/// One documented error code, collected by `frap --describe=errors` (§13.4). Each crate
/// registers its codes from its own `errors.rs` (§21.2).
#[derive(Debug)]
pub struct ErrorCodeDoc {
    /// The code.
    pub code: ErrorCode,
    /// Exception class it is raised with.
    pub exception: PyExc,
    /// One-line description.
    pub summary: &'static str,
}

/// Every crate's `ErrorCodeDoc`s.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static ERROR_CODES: [ErrorCodeDoc];

/// Error codes raised by the RNA layer (`coerce`, resolver, hooks; §13.4 table).
pub mod codes {
    use super::ErrorCode;
    /// Unknown attribute.
    pub const RNA_NO_ATTRIBUTE: ErrorCode = ErrorCode("RNA_NO_ATTRIBUTE");
    /// Read-only property.
    pub const RNA_READONLY: ErrorCode = ErrorCode("RNA_READONLY");
    /// Write to a `bpy.context` member.
    pub const CONTEXT_READONLY: ErrorCode = ErrorCode("CONTEXT_READONLY");
    /// 4.x member removed in 5.x (`spec/overlay/compat_4x.toml`).
    pub const RNA_REMOVED_IN_5X: ErrorCode = ErrorCode("RNA_REMOVED_IN_5X");
    /// `coll['Nope']`
    pub const RNA_KEY_NOT_FOUND: ErrorCode = ErrorCode("RNA_KEY_NOT_FOUND");
    /// `coll[9]`
    pub const RNA_INDEX_RANGE: ErrorCode = ErrorCode("RNA_INDEX_RANGE");
    /// Wrong Python type.
    pub const RNA_TYPE: ErrorCode = ErrorCode("RNA_TYPE");
    /// Unknown enum item.
    pub const RNA_ENUM: ErrorCode = ErrorCode("RNA_ENUM");
    /// Bad RNA function arguments.
    pub const RNA_FUNC_ARGS: ErrorCode = ErrorCode("RNA_FUNC_ARGS");
    /// Wrong sequence length.
    pub const RNA_LENGTH: ErrorCode = ErrorCode("RNA_LENGTH");
    /// Mesh data write while the mesh is in Edit Mode (§6.3).
    pub const RNA_EDITMODE_WRITE: ErrorCode = ErrorCode("RNA_EDITMODE_WRITE");
    /// Part of the RNA surface frap does not implement yet.
    pub const RNA_NOT_IMPLEMENTED: ErrorCode = ErrorCode("RNA_NOT_IMPLEMENTED");
    /// Any other RNA runtime error.
    pub const RNA_RUNTIME: ErrorCode = ErrorCode("RNA_RUNTIME");
}

/// Blender report level (`bpy.types.Operator.report` types; §13.2 `reports`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReportType {
    /// INFO
    Info,
    /// WARNING
    Warning,
    /// ERROR
    Error,
    /// OPERATOR
    Operator,
    /// PROPERTY
    Property,
}

impl ReportType {
    /// Blender's name.
    pub fn as_str(self) -> &'static str {
        match self {
            ReportType::Info => "INFO",
            ReportType::Warning => "WARNING",
            ReportType::Error => "ERROR",
            ReportType::Operator => "OPERATOR",
            ReportType::Property => "PROPERTY",
        }
    }
}

/// One report (Blender wording when Blender reports it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Level.
    pub level: ReportType,
    /// Message.
    pub message: String,
}

impl Report {
    /// New report.
    pub fn new(level: ReportType, message: impl Into<String>) -> Self {
        Report {
            level,
            message: message.into(),
        }
    }
}

/// Source span. `start`/`len`/`col` are UTF-8 byte offsets, `col_chars` counts code points;
/// `line`/`col` are 1-based.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct Span {
    /// Byte offset into the statement source.
    pub start: u32,
    /// Byte length.
    pub len: u32,
    /// 1-based line.
    pub line: u32,
    /// 1-based column in bytes.
    pub col: u32,
    /// 1-based column in code points.
    pub col_chars: u32,
}

/// The error every front-end and library error converts into.
#[derive(Clone, Debug)]
pub struct FrapError {
    /// Python exception class.
    pub exception: PyExc,
    /// Machine code.
    pub code: ErrorCode,
    /// Blender wording where Blender has one.
    pub message: String,
    /// Source location.
    pub span: Option<Span>,
    /// Structured help: did_you_mean, valid, operator, argument, ...
    pub detail: Map<String, Json>,
    /// Human fix suggestion.
    pub hint: Option<String>,
}

impl FrapError {
    /// New error with empty detail.
    pub fn new(exception: PyExc, code: ErrorCode, message: impl Into<String>) -> Self {
        FrapError {
            exception,
            code,
            message: message.into(),
            span: None,
            detail: Map::new(),
            hint: None,
        }
    }
    /// Adds a detail entry.
    pub fn with_detail(mut self, key: &str, v: impl Into<Json>) -> Self {
        self.detail.insert(key.to_owned(), v.into());
        self
    }
    /// Sets the hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    /// Sets the span when none is set yet.
    pub fn with_span(mut self, span: Span) -> Self {
        self.span.get_or_insert(span);
        self
    }
    /// Converts a caught panic payload into an `INTERNAL` error.
    pub fn internal_panic(payload: Box<dyn std::any::Any + Send>) -> Self {
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic".to_owned());
        FrapError::new(
            PyExc::RuntimeError,
            ErrorCode::INTERNAL,
            format!("Error: frap internal error: {msg}"),
        )
    }
}

impl fmt::Display for FrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.exception.as_str(), self.message)
    }
}
impl std::error::Error for FrapError {}

/// What went wrong in RNA access or value coercion. The caller decides the message prefix
/// (`"bpy_struct: item.attr = val: "` for assignments, `"Converting py args to operator
/// properties: "` for operator arguments) and the error code family (§13.4).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub enum RnaErrorKind {
    /// Wrong Python type.
    Type,
    /// Unknown enum item.
    Enum,
    /// Wrong sequence length.
    Length,
    /// Read-only property.
    ReadOnly,
    /// Unknown attribute.
    NoAttribute,
    /// Write to a `bpy.context` member.
    ContextReadOnly,
    /// Missing collection key.
    KeyNotFound,
    /// Collection index out of range.
    IndexRange,
    /// Bad RNA function arguments.
    FuncArgs,
    /// Mesh data write in Edit Mode.
    EditModeWrite,
    /// 4.x member removed in 5.x.
    RemovedIn5x,
    /// Not implemented by frap yet.
    NotImplemented,
    /// Anything else.
    Other,
}

impl RnaErrorKind {
    /// Default error code of the kind (§13.4 table).
    pub fn default_code(self) -> ErrorCode {
        match self {
            RnaErrorKind::Type => codes::RNA_TYPE,
            RnaErrorKind::Enum => codes::RNA_ENUM,
            RnaErrorKind::Length => codes::RNA_LENGTH,
            RnaErrorKind::ReadOnly => codes::RNA_READONLY,
            RnaErrorKind::NoAttribute => codes::RNA_NO_ATTRIBUTE,
            RnaErrorKind::ContextReadOnly => codes::CONTEXT_READONLY,
            RnaErrorKind::KeyNotFound => codes::RNA_KEY_NOT_FOUND,
            RnaErrorKind::IndexRange => codes::RNA_INDEX_RANGE,
            RnaErrorKind::FuncArgs => codes::RNA_FUNC_ARGS,
            RnaErrorKind::EditModeWrite => codes::RNA_EDITMODE_WRITE,
            RnaErrorKind::RemovedIn5x => codes::RNA_REMOVED_IN_5X,
            RnaErrorKind::NotImplemented => codes::RNA_NOT_IMPLEMENTED,
            RnaErrorKind::Other => codes::RNA_RUNTIME,
        }
    }
}

/// RNA-level error, before it gets a context-specific prefix.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{exception:?}: {message}")]
pub struct RnaError {
    /// Kind (selects the code family).
    pub kind: RnaErrorKind,
    /// Python exception class.
    pub exception: PyExc,
    /// Message without context prefix (Blender wording).
    pub message: String,
    /// Optional hint.
    pub hint: Option<String>,
    /// Optional structured detail.
    pub detail: Map<String, Json>,
}

impl RnaError {
    /// New error.
    pub fn new(kind: RnaErrorKind, exception: PyExc, message: impl Into<String>) -> Self {
        RnaError {
            kind,
            exception,
            message: message.into(),
            hint: None,
            detail: Map::new(),
        }
    }
    /// Sets the hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
    /// Adds a detail entry.
    pub fn with_detail(mut self, key: &str, v: impl Into<Json>) -> Self {
        self.detail.insert(key.to_owned(), v.into());
        self
    }
    /// Converts with a context prefix and a code chosen from the kind.
    ///
    /// Blender's prefixes end in a separator space (`"bpy_struct: item.attr = val: "`); when
    /// the message body is empty Blender emits the prefix without it
    /// (`TypeError: bpy_struct: item.attr = val:` for `obj.location = 5`, verified on 5.2.4), so
    /// trailing whitespace of the prefix is dropped in that case.
    pub fn into_frap(
        self,
        prefix: &str,
        code_for: impl Fn(RnaErrorKind) -> ErrorCode,
    ) -> FrapError {
        let message = if self.message.is_empty() {
            prefix.trim_end().to_owned()
        } else {
            format!("{prefix}{}", self.message)
        };
        FrapError {
            exception: self.exception,
            code: code_for(self.kind),
            message,
            span: None,
            detail: self.detail,
            hint: self.hint,
        }
    }
}

impl From<RnaError> for FrapError {
    /// No message prefix; the code is the kind's default (§13.4).
    fn from(e: RnaError) -> FrapError {
        e.into_frap("", RnaErrorKind::default_code)
    }
}
