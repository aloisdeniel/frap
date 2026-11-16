// SPDX-License-Identifier: GPL-3.0-or-later
//! Probe of `frap-scene` contracts: `Main`/`IdMap`/`ChangeLog` (§5.2), `Object`/`Material`/
//! `Scene` (subset), `Context` (§8), `HostRead`/`Host`/`Evaluator` (§9.5, §7.3), RNA hooks and
//! the derived `MaterialSlot` (§5.6).
// RFC 0001 F-LINK-1: `forbid(unsafe_code)` (§21.6) is incompatible with linkme registration
// (`#[link_section]` trips the `unsafe_code` lint); `deny` + item-level `allow` works.
#![deny(unsafe_code)]

pub mod context;
/// §5.2 puts `Main` in `frap-scene/src/main.rs`; Cargo auto-detects `src/main.rs` as a binary
/// target next to `lib.rs` (RFC 0001 F-ED-2), so the file is `data.rs` here.
pub mod data;
pub mod host;
pub mod material;
pub mod object;
pub mod scene;
pub mod slots;

pub use context::*;
pub use data::*;
pub use host::*;
pub use material::*;
pub use object::*;
pub use scene::*;
