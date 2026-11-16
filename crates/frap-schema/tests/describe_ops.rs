// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe` snapshot per registered operator (ARCHITECTURE §22.1): one file per op,
//! `tests/snapshots/op__<idname>.snap`, created by the WP that implements the op (bless with
//! `INSTA_UPDATE=always cargo test -p frap-schema --test describe_ops` or `cargo insta review`).
//! Every operator of every `frap-ops-*` crate is linked in through `frap_ops_all::link()`, so a
//! new operator file without its snapshot fails here.

#[test]
fn describe_every_registered_operator() {
    frap_ops_all::link();
    let mut defs: Vec<&frap_op::OperatorDef> = frap_op::OPERATORS.iter().collect();
    defs.sort_by_key(|d| d.spec.idname);
    assert!(
        !defs.is_empty(),
        "no operator linked: frap_ops_all::link() must keep every ops crate"
    );
    for def in defs {
        let fragment = frap_schema::manifest::describe_op(def);
        insta::with_settings!({ prepend_module_to_snapshot => false, omit_expression => true }, {
            insta::assert_json_snapshot!(format!("op__{}", def.spec.idname), fragment);
        });
    }
}
