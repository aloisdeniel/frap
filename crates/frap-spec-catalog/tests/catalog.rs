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
    assert!(c.len() > 550, "≈ 600 catalog operators in M8");
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
    // M1 (63, decision log S1-1), every M2 operator of OPERATORS.md (M2 W0), the 31 M3 and
    // 15 M4 operators (M3+M4 W0), the 54 M6 operators (M5+M6 W0; M5 adds none) and the 11 M7
    // `marker.*` operators (M7 W0), and the 192 M8 operators (M8 W0: 183 for the M8 WPs, the
    // nine glTF variant operators).
    assert_eq!(typed.len(), 491, "OPERATORS.md: operators with M <= 8");
    assert!(is_typed("mesh.primitive_cube_add") && !contains("mesh.primitive_cube_add"));
    assert!(contains("mesh.knife_project") && !is_typed("mesh.knife_project"));
    assert!(is_typed("mesh.select_nth") && !contains("mesh.select_nth"));
    assert!(
        contains("object.track_set") && !is_typed("object.track_set"),
        "M9: post-M8"
    );
    assert!(is_typed("mesh.decimate") && is_typed("uv.unwrap"));
    assert!(is_typed("mesh.bevel") && !contains("mesh.bevel"));
    assert!(TYPED_IDNAMES.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn addresses_are_stable() {
    let a = catalog();
    let b = catalog();
    assert!(std::ptr::eq(a, b));
    let bevel = find("mesh.knife_project").unwrap();
    assert!(std::ptr::eq(bevel, find("mesh.knife_project").unwrap()));
    assert!(a.iter().any(|s| std::ptr::eq(*s, bevel)));
    let t = std::thread::spawn(|| find("mesh.knife_project").unwrap() as *const _ as usize)
        .join()
        .unwrap();
    assert_eq!(t, bevel as *const _ as usize);
    assert!(find("mesh.primitive_cube_add").is_none());
    assert!(find("mesh.no_such_op").is_none());
}

#[test]
fn bevel_spec() {
    // `mesh.bevel` is typed since M2, `uv.unwrap` since M3 W0, `uv.minimize_stretch` since M8
    // W0; `object.track_set` (M9, post-M8) stays in the catalog.
    assert!(find("uv.unwrap").is_none() && find("uv.minimize_stretch").is_none());
    let s = find("object.track_set").unwrap();
    assert_eq!(s.bl_idname, "OBJECT_OT_track_set");
    assert!(s.prop("type").is_some());
    assert!(s.poll_matrix.is_some());
    // A macro: steps decoded in order (an OUT macro; the M8 ones are typed since M8 W0).
    let m = find("nla.duplicate_move").unwrap();
    assert_eq!(
        m.macro_steps,
        ["NLA_OT_duplicate", "TRANSFORM_OT_translate"]
    );
    // A dynamic enum from the overlay: user-data enums are empty and flagged
    // (`object.vertex_group_set_active` is typed since M5+M6 W0; constraints are OUT).
    let vg = find("object.constraint_add").unwrap().prop("type").unwrap();
    assert!(vg.flags.contains(frap_rna::PropFlags::DYNAMIC_ENUM));
}
