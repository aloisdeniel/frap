// SPDX-License-Identifier: GPL-3.0-or-later
//! Drift test (ARCHITECTURE §4.2): every generated `OperatorSpec` of `frap-spec-ops-data` equals
//! its Blender 5.2.4 dump (props, options, macro steps, poll matrix), and the typed props
//! structs exist exactly for the OPERATORS.md operators of this group with `M` ≤ the current
//! milestone (the full cross-crate check is `cargo test -p xtask`).

#[path = "../../frap-spec-rna/tests/common/mod.rs"]
mod common;

#[test]
fn typed_specs_match_dumps() {
    let all = frap_spec_ops_data::ALL_OPS;
    assert_eq!(all.len(), 9, "typed operators of the `data` group in M4 (M3+M4 W0)");
    for s in all {
        common::check_op(s);
    }
    assert!(all.windows(2).all(|w| w[0].idname < w[1].idname));
}
