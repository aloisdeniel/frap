// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-nodes` (layer L4): `NodeTree`/`Node`/`Socket`/`Link`, the node-type table (generated
//! `NodeTypeDef`s), dynamic socket availability per node type, built-in node groups
//! ("Smooth by Angle", "glTF Material Output") and RNA bindings (ARCHITECTURE §16.1).
//!
//! Ownership (M1): WP 1.11 owns everything except `src/generated/**` (WP 1.1) and this file.
#![deny(unsafe_code)]

mod tree;

/// Per-node socket availability (`availability/<node>.rs`, one file per node type whose
/// sockets depend on a mode property). `automod`: adding a file needs no `mod` edit.
pub mod availability {
    automod::dir!(pub "src/availability");
}
/// Built-in node groups (`builtin_groups/<group>.rs`).
pub mod builtin_groups {
    automod::dir!(pub "src/builtin_groups");
}
/// Generated `NodeTypeDef` per `bl_idname` (`cargo xtask codegen`, WP 1.1).
#[rustfmt::skip]
#[allow(missing_docs, clippy::all)]
pub mod generated;
/// RNA bindings of node trees, nodes and sockets (WP 1.11).
pub mod rna;

pub use tree::*;
