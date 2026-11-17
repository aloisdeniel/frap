// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON-RPC 2.0 response objects and the standard error codes.

use serde_json::{Value as J, json};

/// Invalid JSON.
pub const PARSE_ERROR: i64 = -32700;
/// Not a valid request object.
pub const INVALID_REQUEST: i64 = -32600;
/// Unknown method.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Invalid method parameters (also MCP's "unknown tool").
pub const INVALID_PARAMS: i64 = -32602;

/// A success response.
pub fn result(id: J, result: J) -> J {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// An error response.
pub fn error(id: J, code: i64, message: &str) -> J {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
