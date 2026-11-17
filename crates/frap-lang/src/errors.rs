// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes raised by `frap-lang` (§13.4), registered for `frap --describe=errors`.

use frap_rna::{ERROR_CODES, ErrorCode, ErrorCodeDoc, PyExc};

/// Python-level syntax error (`'(' was never closed`, `invalid syntax`): CPython's wording.
pub const SYNTAX: ErrorCode = ErrorCode("SYNTAX");
/// Valid Python that frap does not accept (variables, loops, f-strings, ...).
pub const UNSUPPORTED_SYNTAX: ErrorCode = ErrorCode("UNSUPPORTED_SYNTAX");
/// A name that is neither a path root nor a supported built-in (`name 'cube' is not defined`).
/// frap has no variables, so the front-end knows statically that the name is unbound.
pub const NAME: ErrorCode = ErrorCode("NAME");

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(ERROR_CODES)]
#[linkme(crate = frap_rna::linkme)]
static DOC_SYNTAX: ErrorCodeDoc = ErrorCodeDoc {
    code: SYNTAX,
    exception: PyExc::SyntaxError,
    summary: "Python syntax error, with CPython's message (e.g. '(' was never closed)",
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(ERROR_CODES)]
#[linkme(crate = frap_rna::linkme)]
static DOC_UNSUPPORTED_SYNTAX: ErrorCodeDoc = ErrorCodeDoc {
    code: UNSUPPORTED_SYNTAX,
    exception: PyExc::SyntaxError,
    summary: "valid Python outside frap's statement subset (variables, loops, f-strings, ...)",
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(ERROR_CODES)]
#[linkme(crate = frap_rna::linkme)]
static DOC_NAME: ErrorCodeDoc = ErrorCodeDoc {
    code: NAME,
    exception: PyExc::NameError,
    summary: "a name that is not bpy, D, C, a math/mathutils built-in or a literal",
};
