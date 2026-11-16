// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-ops-all` (layer L10): aggregator of every `frap-ops-*` crate (ARCHITECTURE §10.5).
//!
//! A `linkme` slice is a link-time static: a crate the linker drops vanishes from every slice.
//! `link()` references one symbol per ops crate, which keeps all of that crate's entries
//! (verified in M1 W0 on macOS for debug and fat-LTO release builds; the registry test in
//! `tests/registry.rs` runs on every OS and in the release profile).
//!
//! Owned by the integrator. Feature flags: ARCHITECTURE §3.6.
#![deny(unsafe_code)]

/// Keeps every operator, RNA function and hook of every ops crate in the final binary.
/// Call it once at session start (`frap_session::Session::new` does).
pub fn link() {
    std::hint::black_box(frap_ops_object::link as fn());
    std::hint::black_box(frap_ops_transform::link as fn());
    std::hint::black_box(frap_ops_mesh::link as fn());
    std::hint::black_box(frap_ops_uv::link as fn());
    std::hint::black_box(frap_ops_anim::link as fn());
    std::hint::black_box(frap_ops_data::link as fn());
    std::hint::black_box(frap_ops_io::link as fn());
    std::hint::black_box(frap_ops_render::link as fn());
    std::hint::black_box(frap_ops_frap::link as fn());
    frap_ops_object::link();
    frap_ops_transform::link();
    frap_ops_mesh::link();
    frap_ops_uv::link();
    frap_ops_anim::link();
    frap_ops_data::link();
    frap_ops_io::link();
    frap_ops_render::link();
    frap_ops_frap::link();
}

/// The ops crates `link()` covers, with their primary namespace (§21.1; `None` = every file
/// is `ops/<namespace>__<op>.rs`). Used by the registry test to map file names to idnames.
pub const OPS_CRATES: &[(&str, Option<&str>)] = &[
    ("frap-ops-object", Some("object")),
    ("frap-ops-transform", Some("transform")),
    ("frap-ops-mesh", Some("mesh")),
    ("frap-ops-uv", Some("uv")),
    ("frap-ops-anim", Some("anim")),
    ("frap-ops-data", None),
    ("frap-ops-io", None),
    ("frap-ops-render", None),
    ("frap-ops-frap", Some("frap")),
];
