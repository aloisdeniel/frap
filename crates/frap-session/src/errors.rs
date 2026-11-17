// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes raised by the session: resolver and executor (§13.4), registered for
//! `frap --describe=errors`. The RNA codes are the `frap_rna::codes` constants (RFC 0001
//! F-ERR-1); the resolver and executor are where they are raised, so they are documented here.

use frap_rna::{ERROR_CODES, ErrorCode, ErrorCodeDoc, PyExc, codes};

/// `name 'x' is not defined`
pub const NAME: ErrorCode = ErrorCode("NAME");
/// Unknown attribute.
pub const RNA_NO_ATTRIBUTE: ErrorCode = codes::RNA_NO_ATTRIBUTE;
/// Read-only property.
pub const RNA_READONLY: ErrorCode = codes::RNA_READONLY;
/// `bpy.context` is read-only.
pub const CONTEXT_READONLY: ErrorCode = codes::CONTEXT_READONLY;
/// Missing collection key.
pub const RNA_KEY_NOT_FOUND: ErrorCode = codes::RNA_KEY_NOT_FOUND;
/// Collection index out of range.
pub const RNA_INDEX_RANGE: ErrorCode = codes::RNA_INDEX_RANGE;
/// Wrong value type in an assignment.
pub const RNA_TYPE: ErrorCode = codes::RNA_TYPE;
/// Unknown enum item in an assignment.
pub const RNA_ENUM: ErrorCode = codes::RNA_ENUM;
/// Wrong sequence length in an assignment.
pub const RNA_LENGTH: ErrorCode = codes::RNA_LENGTH;
/// Valid in Blender, not implemented by frap in this milestone.
pub const NOT_IMPLEMENTED: ErrorCode = ErrorCode("NOT_IMPLEMENTED");
/// `ZeroDivisionError` (CPython / `mathutils` message). `PyExc` has no such class, so it is
/// raised as `ValueError` with `detail.python_exception = "ZeroDivisionError"`.
pub const ZERO_DIVISION: ErrorCode = ErrorCode("ZERO_DIVISION");

macro_rules! doc {
    ($name:ident, $code:expr, $exc:ident, $summary:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(ERROR_CODES)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: ErrorCodeDoc = ErrorCodeDoc {
            code: $code,
            exception: PyExc::$exc,
            summary: $summary,
        };
    };
}

doc!(
    DOC_NOT_IMPLEMENTED,
    NOT_IMPLEMENTED,
    NotImplementedError,
    "a statement form, path or RNA hook valid in Blender 5.2 that frap does not implement yet"
);
doc!(
    DOC_ZERO_DIVISION,
    ZERO_DIVISION,
    ValueError,
    "division by zero in an expression (Python's ZeroDivisionError, with its message)"
);
doc!(
    DOC_INTERNAL,
    ErrorCode::INTERNAL,
    RuntimeError,
    "a caught panic; the statement was rolled back and the message names the panic"
);
doc!(
    DOC_RNA_NO_ATTRIBUTE,
    codes::RNA_NO_ATTRIBUTE,
    AttributeError,
    "'Object' object has no attribute 'foo' (with did_you_mean)"
);
doc!(
    DOC_RNA_READONLY,
    codes::RNA_READONLY,
    AttributeError,
    "bpy_struct: attribute \"type\" from \"Object\" is read-only"
);
doc!(
    DOC_CONTEXT_READONLY,
    codes::CONTEXT_READONLY,
    AttributeError,
    "bpy.context members are read-only (bpy_struct: Context property \"object\" is read-only)"
);
doc!(
    DOC_RNA_REMOVED_IN_5X,
    codes::RNA_REMOVED_IN_5X,
    AttributeError,
    "a 4.x member removed in 5.x; the hint names its replacement"
);
doc!(
    DOC_RNA_KEY_NOT_FOUND,
    codes::RNA_KEY_NOT_FOUND,
    KeyError,
    "bpy_prop_collection[key]: key \"Nope\" not found"
);
doc!(
    DOC_RNA_INDEX_RANGE,
    codes::RNA_INDEX_RANGE,
    IndexError,
    "bpy_prop_collection[index]: index 9 out of range, size 3 (and array/vector index errors)"
);
doc!(
    DOC_RNA_TYPE,
    codes::RNA_TYPE,
    TypeError,
    "wrong value type in an assignment or expression, with Blender's message"
);
doc!(
    DOC_RNA_ENUM,
    codes::RNA_ENUM,
    TypeError,
    "enum \"FOO\" not found in ('A', 'B'): the valid items are listed"
);
doc!(
    DOC_RNA_LENGTH,
    codes::RNA_LENGTH,
    ValueError,
    "sequences of dimension 0 should contain 3 items, not 2"
);
doc!(
    DOC_RNA_FUNC_ARGS,
    codes::RNA_FUNC_ARGS,
    TypeError,
    "bad RNA function arguments (BlendDataObjects.new(): required parameter \"name\" not specified)"
);
doc!(
    DOC_RNA_NOT_IMPLEMENTED,
    codes::RNA_NOT_IMPLEMENTED,
    NotImplementedError,
    "an RNA property that exists in Blender 5.2 but has no frap binding yet"
);
doc!(
    DOC_RNA_RUNTIME,
    codes::RNA_RUNTIME,
    RuntimeError,
    "an RNA function's own error, in Blender's wording"
);
