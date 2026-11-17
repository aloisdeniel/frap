// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[bmesh_op]`. Owned by M2 (WP 1.12 stub).

use proc_macro::TokenStream;

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = (attr, item);
    crate::not_yet("#[bmesh_op]", "M2 (WP 1.12 stub)")
}
