// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-bmesh` (layer L5): `BMesh` with `BLI_mempool`-identical LIFO pools, kernel Euler ops
//! with Blender names, CustomData, select flush/history, walkers, `from_mesh`/`to_mesh`, one
//! file per `bmesh.ops` operator (ARCHITECTURE §6.2).
//!
//! M1 W0: `BMesh` is an opaque placeholder so that `EditState` (§8) and `OpCtx::bmesh_mut`
//! (§10.1), both frozen at M1 W0, compile. The `BMesh` public API is a CONTRACT frozen in
//! **M2 W0** (§3.4), which replaces this file's contents.
#![deny(unsafe_code)]

/// Edit-mode mesh (placeholder until M2 W0).
#[derive(Clone, Debug, Default)]
pub struct BMesh {
    _private: (),
}
