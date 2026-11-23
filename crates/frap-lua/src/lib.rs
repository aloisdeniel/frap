// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-lua` (layer L12): a Lua front-end spike over the `Session` API (ARCHITECTURE §11.6,
//! M8 WP 8.10, RFC `docs/design/rfc/0002-frap-lua.md`).
//!
//! With the `lua` feature (off by default; `mlua` with a vendored Lua 5.4 built by `cc`),
//! [`LuaFrontend`] runs Lua chunks against one [`frap_session::Session`]:
//!
//! ```lua
//! bpy.ops.object.select_all{action='SELECT'}
//! bpy.ops.object.delete()
//! bpy.ops.mesh.primitive_cylinder_add{vertices=16, radius=0.5, depth=2, location={0, 0, 1}}
//! local steel = bpy.data.materials.new("Steel")
//! steel.node_tree.nodes['Principled BSDF'].inputs['Metallic'].default_value = 1.0
//! bpy.context.object.data.materials:append(steel)
//! print(bpy.data.objects['Cylinder'].location.z)      --> 1
//! ```
//!
//! - `bpy.ops.<ns>.<op>{…}` → [`Session::invoke`](frap_session::Session::invoke) (keywords
//!   only; returns the status string and the response as a table);
//! - reading `bpy.data…`/`bpy.context…` → [`Session::get`](frap_session::Session::get):
//!   structs, collections and functions come back as proxies, whole array properties as
//!   write-through array proxies, other values as Lua values;
//! - assigning → [`Session::set`](frap_session::Session::set);
//! - calling a function proxy (`f(…)` or `owner:f(…)`) →
//!   [`Session::call`](frap_session::Session::call).
//!
//! A failed call raises a Lua error whose value is a table `{exception, code, message,
//! hint, detail}` (`tostring` gives `TypeError: …`); [`LuaFrontend::exec`] returns the
//! session's own [`frap_rna::FrapError`]. The state has no `os`, `io`, `package`, `require`,
//! `debug`, `dofile`, `loadfile` or binary chunks, a heap limit, and an instruction hook that
//! stops a chunk at `--stmt-timeout` (`Limits::stmt_timeout`).
//!
//! Without the feature the crate is this constant: the default build and the `cc`-only
//! container are unaffected, and nothing depends on the crate.
#![deny(unsafe_code)]

/// Whether this build has the Lua runtime (`lua` feature).
pub const HAS_LUA: bool = cfg!(feature = "lua");

#[cfg(feature = "lua")]
pub mod errors;
#[cfg(feature = "lua")]
mod front;

#[cfg(feature = "lua")]
pub use front::{DEFAULT_MEMORY_LIMIT, LuaConfig, LuaFrontend};
