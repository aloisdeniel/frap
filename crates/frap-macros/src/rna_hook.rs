// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[rna_hook]`. Owned by WP 1.2.

use proc_macro::TokenStream;

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = (attr, item);
    crate::not_yet("#[rna_hook]", "WP 1.2")
}
