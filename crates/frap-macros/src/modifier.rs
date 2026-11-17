// SPDX-License-Identifier: GPL-3.0-or-later
//! `#[modifier]`. Owned by M4 (WP 1.12 stub).

use proc_macro::TokenStream;

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _ = (attr, item);
    crate::not_yet("#[modifier]", "M4 (WP 1.12 stub)")
}
