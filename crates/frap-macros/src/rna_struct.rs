// SPDX-License-Identifier: GPL-3.0-or-later
//! `rna_struct!` (§9.6). Owned by WP 1.2.

use proc_macro::TokenStream;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    let _ = input;
    crate::not_yet("rna_struct!", "WP 1.2")
}
