// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-mesh` (layer L4): the `Mesh` datablock with struct-of-arrays attributes on four
//! domains, primitive generators, normals, loop triangles, corner split, and the RNA bindings
//! of `Mesh` and its element structs (ARCHITECTURE §6.1). Points at materials and shape keys
//! through `IdRef<MaterialId>` / `IdRef<KeyId>` from `frap-rna`; never depends on `frap-scene`.
//!
//! Ownership (M1): WP 1.4 owns everything except `src/rna/**` (WP 1.27) and this file.
#![deny(unsafe_code)]

mod mesh;
mod normals;
pub mod primitives;
mod rna;
pub mod split;

pub use mesh::{AttrData, Attribute, AttributeSet, DeformVert, Domain, Mesh, SHARP_FACE, TexSpace};
