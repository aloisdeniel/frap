// SPDX-License-Identifier: GPL-3.0-or-later
//! §13.4 Errors (`frap-rna/src/error.rs`, CONTRACT) — the probe keeps `detail` as a plain
//! vector instead of `serde_json::Map` to avoid the dependency in the scratch crate.

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum PyExc {
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

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct ErrorCode(pub &'static str);

#[derive(Copy, Clone, Eq, PartialEq, Debug, Default)]
pub struct Span {
    pub start: u32,
    pub len: u32,
    pub line: u32,
    pub col: u32,
    pub col_chars: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrapError {
    pub exception: PyExc,
    pub code: ErrorCode,
    pub message: String,
    pub span: Option<Span>,
    pub detail: Vec<(String, String)>,
    pub hint: Option<String>,
}

/// `RnaError` is listed as CONTRACT in §3.4 but never defined (RFC 0001 F-ERR-1). The probe
/// makes it the RNA-layer error with the same three observable fields as `FrapError`.
#[derive(Clone, Debug, PartialEq)]
pub struct RnaError {
    pub exception: PyExc,
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
}

impl RnaError {
    pub fn new(exception: PyExc, code: ErrorCode, message: impl Into<String>) -> Self {
        RnaError {
            exception,
            code,
            message: message.into(),
            hint: None,
        }
    }
}

impl From<RnaError> for FrapError {
    fn from(e: RnaError) -> Self {
        FrapError {
            exception: e.exception,
            code: e.code,
            message: e.message,
            span: None,
            detail: vec![],
            hint: e.hint,
        }
    }
}

pub mod codes {
    use super::ErrorCode;
    pub const RNA_NO_ATTRIBUTE: ErrorCode = ErrorCode("RNA_NO_ATTRIBUTE");
    pub const RNA_READONLY: ErrorCode = ErrorCode("RNA_READONLY");
    pub const CONTEXT_READONLY: ErrorCode = ErrorCode("CONTEXT_READONLY");
    pub const RNA_KEY_NOT_FOUND: ErrorCode = ErrorCode("RNA_KEY_NOT_FOUND");
    pub const RNA_INDEX_RANGE: ErrorCode = ErrorCode("RNA_INDEX_RANGE");
    pub const RNA_TYPE: ErrorCode = ErrorCode("RNA_TYPE");
    pub const RNA_ENUM: ErrorCode = ErrorCode("RNA_ENUM");
    pub const RNA_LENGTH: ErrorCode = ErrorCode("RNA_LENGTH");
    pub const RNA_EDITMODE_WRITE: ErrorCode = ErrorCode("RNA_EDITMODE_WRITE");
    pub const INTERNAL: ErrorCode = ErrorCode("INTERNAL");
    pub const NOT_IMPLEMENTED: ErrorCode = ErrorCode("NOT_IMPLEMENTED");
}

/// Blender report levels (§13.2 `reports`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReportType {
    Info,
    Warning,
    Error,
    Operator,
    Property,
}

/// RFC 0001 F-LAYER-1: §3.4 places `Report` in `frap-op` (L9), but `Host::report` (frap-scene,
/// L6), `EvaluatedObject.warnings` (L6), `ModEvalCx::report` (L7) and
/// `LoweredMaterial.diagnostics` (L7) all name it. It must live in `frap-rna` (L1).
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub level: ReportType,
    pub message: String,
}
