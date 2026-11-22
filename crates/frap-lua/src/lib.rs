// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-lua` (layer L12): a Lua front-end spike over the `Session` API (ARCHITECTURE §11.6):
//! `bpy.ops` / `bpy.data` userdata whose `__index` / `__newindex` / `__call` map onto
//! `Session::{invoke, get, set, call}`, errors as Lua errors carrying the `FrapError` code,
//! and a sandbox without `os` / `io`. Nothing below `frap-session` changes.
//!
//! M8 W0 stub (decision log M8W0-9): WP 8.10 owns `crates/frap-lua/**` and writes the spike
//! behind the `lua` feature (`mlua`, Lua 5.4 vendored), with its RFC
//! (`docs/design/rfc/0005-frap-lua.md`) recommending whether to ship it.
#![deny(unsafe_code)]

/// Whether this build has the Lua runtime (`lua` feature).
pub const HAS_LUA: bool = cfg!(feature = "lua");
