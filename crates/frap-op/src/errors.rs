// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes raised by the operator framework (§13.4).

use frap_rna::ErrorCode;

/// Unknown operator.
pub const OP_UNKNOWN: ErrorCode = ErrorCode("OP_UNKNOWN");
/// Known to Blender, not implemented by frap.
pub const OP_NOT_IMPLEMENTED: ErrorCode = ErrorCode("OP_NOT_IMPLEMENTED");
/// Unknown keyword argument.
pub const OP_ARG_UNKNOWN: ErrorCode = ErrorCode("OP_ARG_UNKNOWN");
/// Wrong argument type.
pub const OP_ARG_TYPE: ErrorCode = ErrorCode("OP_ARG_TYPE");
/// Unknown enum item.
pub const OP_ARG_ENUM: ErrorCode = ErrorCode("OP_ARG_ENUM");
/// Non-context positional argument (deviation: Blender ignores it silently).
pub const OP_ARG_POSITIONAL: ErrorCode = ErrorCode("OP_ARG_POSITIONAL");
/// Wrong sequence length.
pub const OP_ARG_LENGTH: ErrorCode = ErrorCode("OP_ARG_LENGTH");
/// Poll failed.
pub const OP_POLL_FAILED: ErrorCode = ErrorCode("OP_POLL_FAILED");
/// Error raised during exec.
pub const OP_EXEC_ERROR: ErrorCode = ErrorCode("OP_EXEC_ERROR");
/// File I/O failure.
pub const IO_ERROR: ErrorCode = ErrorCode("IO_ERROR");
/// Unsupported file format.
pub const IO_UNSUPPORTED_FORMAT: ErrorCode = ErrorCode("IO_UNSUPPORTED_FORMAT");
/// No GPU adapter, or built without the `gpu` feature.
pub const GPU_UNAVAILABLE: ErrorCode = ErrorCode("GPU_UNAVAILABLE");
/// `--max-elements` / `--stmt-timeout` exceeded.
pub const LIMIT_EXCEEDED: ErrorCode = ErrorCode("LIMIT_EXCEEDED");
