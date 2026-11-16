// SPDX-License-Identifier: GPL-3.0-or-later
//! Catalog tests (ARCHITECTURE §4.2): every curated operator without a typed props struct
//! decodes from the blob, equals its dump, and has a stable `&'static` address.

#[path = "../../frap-spec-rna/tests/common/mod.rs"]
mod common;

use std::collections::BTreeSet;

use frap_spec_catalog::catalog;
use frap_spec_catalog::generated::{
    CATALOG_IDNAMES, CATALOG_LEN, TYPED_IDNAMES, contains, find, is_typed,
};

#[test]
fn every_catalog_operator_decodes_and_matches_its_dump() {
    let c = catalog();
    assert_eq!(c.len(), CATALOG_LEN);
    assert!(c.len() > 900, "≈ 1,000 catalog operators");
    let ids: Vec<&str> = c.iter().map(|s| s.idname).collect();
    assert_eq!(ids, CATALOG_IDNAMES.to_vec());
    for s in c {
        common::check_op(s);
    }
}

#[test]
fn typed_and_catalog_partition_the_dumps() {
    let all = common::all_dumped_idnames();
    let cat: BTreeSet<String> = CATALOG_IDNAMES.iter().map(|s| s.to_string()).collect();
    let typed: BTreeSet<String> = TYPED_IDNAMES.iter().map(|s| s.to_string()).collect();
    assert!(cat.is_disjoint(&typed));
    assert_eq!(cat.union(&typed).cloned().collect::<BTreeSet<_>>(), all);
    assert_eq!(typed.len(), 64, "OPERATORS.md: 64 M1 operators");
    assert!(is_typed("mesh.primitive_cube_add") && !contains("mesh.primitive_cube_add"));
    assert!(contains("mesh.bevel") && !is_typed("mesh.bevel"));
    assert!(TYPED_IDNAMES.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn addresses_are_stable() {
    let a = catalog();
    let b = catalog();
    assert!(std::ptr::eq(a, b));
    let bevel = find("mesh.bevel").unwrap();
    assert!(std::ptr::eq(bevel, find("mesh.bevel").unwrap()));
    assert!(a.iter().any(|s| std::ptr::eq(*s, bevel)));
    let t = std::thread::spawn(|| find("mesh.bevel").unwrap() as *const _ as usize)
        .join()
        .unwrap();
    assert_eq!(t, bevel as *const _ as usize);
    assert!(find("mesh.primitive_cube_add").is_none());
    assert!(find("mesh.no_such_op").is_none());
}

#[test]
fn bevel_spec() {
    let s = find("mesh.bevel").unwrap();
    assert_eq!(s.bl_idname, "MESH_OT_bevel");
    assert!(s.prop("offset").is_some());
    assert!(s.prop("segments").is_some());
    assert!(s.poll_matrix.is_some());
    // A macro: steps decoded in order.
    let m = find("mesh.extrude_region_move").unwrap();
    assert_eq!(
        m.macro_steps,
        ["MESH_OT_extrude_region", "TRANSFORM_OT_translate"]
    );
    // A dynamic enum from the overlay: user-data enums are empty and flagged.
    let vg = find("object.vertex_group_set_active")
        .unwrap()
        .prop("group")
        .unwrap();
    assert!(vg.flags.contains(frap_rna::PropFlags::DYNAMIC_ENUM));
}
