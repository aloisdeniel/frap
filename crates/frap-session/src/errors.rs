// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes raised by the session: resolver and executor (§13.4).

use frap_rna::ErrorCode;

/// `name 'x' is not defined`
pub const NAME: ErrorCode = ErrorCode("NAME");
/// Unknown attribute.
pub const RNA_NO_ATTRIBUTE: ErrorCode = ErrorCode("RNA_NO_ATTRIBUTE");
/// Read-only property.
pub const RNA_READONLY: ErrorCode = ErrorCode("RNA_READONLY");
/// `bpy.context` is read-only.
pub const CONTEXT_READONLY: ErrorCode = ErrorCode("CONTEXT_READONLY");
/// Missing collection key.
pub const RNA_KEY_NOT_FOUND: ErrorCode = ErrorCode("RNA_KEY_NOT_FOUND");
/// Collection index out of range.
pub const RNA_INDEX_RANGE: ErrorCode = ErrorCode("RNA_INDEX_RANGE");
/// Wrong value type in an assignment.
pub const RNA_TYPE: ErrorCode = ErrorCode("RNA_TYPE");
/// Unknown enum item in an assignment.
pub const RNA_ENUM: ErrorCode = ErrorCode("RNA_ENUM");
/// Wrong sequence length in an assignment.
pub const RNA_LENGTH: ErrorCode = ErrorCode("RNA_LENGTH");
/// Valid in Blender, not implemented by frap in this milestone.
pub const NOT_IMPLEMENTED: ErrorCode = ErrorCode("NOT_IMPLEMENTED");
