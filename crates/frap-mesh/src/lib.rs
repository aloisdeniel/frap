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
// Data-API helpers (`attributes.new/remove`, `uv_layers.new`, M1 stabilization).
pub use mesh::{AttributeError, MAX_UV_MAPS, default_column};
pub use rna::attribute_srna;
// WP 1.4 additions (Mesh API: normals domain, flag attributes, internal attribute names).
pub use mesh::{
    CUSTOM_NORMAL, NormalsDomain, REMOVED_WHEN_ALL_FALSE, SELECT_EDGE, SELECT_POLY, SELECT_VERT,
    SHARP_EDGE, UV_SEAM, UV_SELECT_EDGE, UV_SELECT_FACE, UV_SELECT_VERT,
};
