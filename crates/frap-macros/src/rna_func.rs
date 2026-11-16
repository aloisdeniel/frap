// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[rna_func]`. Owned by WP 1.2.

use proc_macro::TokenStream;

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = (attr, item);
    crate::not_yet("#[rna_func]", "WP 1.2")
}
