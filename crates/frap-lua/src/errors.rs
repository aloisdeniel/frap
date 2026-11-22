// SPDX-License-Identifier: GPL-3.0-or-later
//! Error codes the Lua front-end raises itself (§13.4). Everything a session call raises
//! keeps the session's own code. The front-end is not linked into `frap`, so these codes are
//! not registered for `frap --describe=errors`; RFC 0002 lists them for the day it is.

use frap_rna::{ErrorCode, PyExc};

/// A Lua runtime error that is not a frap error (`attempt to index a nil value`, a string
/// passed to `error`): `RuntimeError`, the Lua message with its `lua:<line>:` position.
pub const LUA_ERROR: ErrorCode = ErrorCode("LUA_ERROR");
/// A Lua value that has no Python counterpart (a function, a table mixing a sequence and
/// string keys, a non-UTF-8 string): `TypeError`.
pub const LUA_VALUE: ErrorCode = ErrorCode("LUA_VALUE");
/// A Lua syntax error: `SyntaxError`, the code frap-lang uses for Python syntax errors.
pub const SYNTAX: ErrorCode = ErrorCode("SYNTAX");
/// The Lua heap passed `LuaConfig::memory_limit`, or a string-building library call would
/// pass 64 MiB: `RuntimeError`, the code of `--max-elements`.
pub const LIMIT_EXCEEDED: ErrorCode = ErrorCode("LIMIT_EXCEEDED");

/// The exception and code of an error the prelude raises by name (`P.fail(code, msg)`).
/// Unknown names fall back to `RuntimeError` `LUA_ERROR`, so the prelude can never mint a
/// code at run time.
pub(crate) fn prelude_code(name: &str) -> (PyExc, ErrorCode) {
    match name {
        "LUA_VALUE" => (PyExc::TypeError, LUA_VALUE),
        // frap-op's code (not a dependency of this crate).
        "OP_ARG_POSITIONAL" => (PyExc::TypeError, ErrorCode("OP_ARG_POSITIONAL")),
        "RNA_NO_ATTRIBUTE" => (PyExc::AttributeError, frap_rna::codes::RNA_NO_ATTRIBUTE),
        "RNA_TYPE" => (PyExc::TypeError, frap_rna::codes::RNA_TYPE),
        "TIMEOUT" => (PyExc::RuntimeError, frap_session::errors::TIMEOUT),
        "LIMIT_EXCEEDED" => (PyExc::RuntimeError, LIMIT_EXCEEDED),
        _ => (PyExc::RuntimeError, LUA_ERROR),
    }
}
