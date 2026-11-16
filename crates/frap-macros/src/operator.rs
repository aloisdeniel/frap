// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[operator]`. Owned by WP 1.12.

use proc_macro::TokenStream;

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = (attr, item);
    crate::not_yet("#[operator]", "WP 1.12")
}
