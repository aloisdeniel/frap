// SPDX-License-Identifier: GPL-3.0-or-later
//! `Registry::build` rules (§10.1): duplicates, implemented operators without a typed spec, and
//! implementations that carry a copy of their generated spec; lookup categories.

mod common;

use common::{dummy, spec};
use frap_op::prelude::*;
use frap_op::{Lookup, Registry};
use frap_rna::OperatorSpec;

fn exec(_cx: &mut OpCtx, _a: &OpArgs) -> OpResult {
    Ok(OpStatus::Finished)
}

const fn def(spec: &'static OperatorSpec) -> OperatorDef {
    OperatorDef {
        spec,
        poll: poll::ALWAYS,
        exec,
        kind: OpKind::Normal,
        dyn_enums: &[],
        tier: Tier::T0,
        support: Support::Full,
        notes: &[],
    }
}

static ADD: OperatorDef = def(&dummy::OPSPEC_DUMMY_OT_ADD);
static ADD_AGAIN: OperatorDef = def(&dummy::OPSPEC_DUMMY_OT_ADD);
static FAKE_BEVEL_SPEC: OperatorSpec = spec("mesh.select_nth", "MESH_OT_select_nth", &[], &[]);
static FAKE_BEVEL: OperatorDef = def(&FAKE_BEVEL_SPEC);
static COPY_CUBE_SPEC: OperatorSpec = spec(
    "mesh.primitive_cube_add",
    "MESH_OT_primitive_cube_add",
    &[],
    &[],
);
static COPY_CUBE: OperatorDef = def(&COPY_CUBE_SPEC);
static CUBE: OperatorDef = def(&frap_spec_ops_mesh::mesh::OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD);

fn catalog() -> impl Iterator<Item = &'static OperatorSpec> {
    frap_spec_catalog::catalog().iter().copied()
}

#[test]
fn builds_and_classifies() {
    let r = Registry::build([&ADD, &CUBE], Registry::typed_specs(), catalog()).unwrap();
    assert!(matches!(r.lookup("dummy.add"), Lookup::Implemented(d) if std::ptr::eq(d, &ADD)));
    assert!(matches!(
        r.lookup("mesh.primitive_cube_add"),
        Lookup::Implemented(_)
    ));
    assert!(
        matches!(r.lookup("mesh.bevel"), Lookup::KnownNotImplemented(s) if s.bl_idname == "MESH_OT_bevel")
    );
    assert!(matches!(
        r.lookup("object.delete"),
        Lookup::KnownNotImplemented(_)
    ));
    match r.lookup("mesh.primitive_cub_add") {
        Lookup::Unknown { suggestions } => assert_eq!(suggestions[0], "mesh.primitive_cube_add"),
        _ => panic!("expected Unknown"),
    }
    assert_eq!(r.idnames(), vec!["dummy.add", "mesh.primitive_cube_add"]);
    assert!(!r.known_idnames().contains(&"mesh.primitive_cube_add"));
    assert!(r.get_by_bl_idname("MESH_OT_primitive_cube_add").is_some());
    assert!(r.get_by_bl_idname("MESH_OT_bevel").is_none());
    assert_eq!(
        r.spec_by_bl_idname("MESH_OT_bevel").unwrap().idname,
        "mesh.bevel"
    );
    assert_eq!(r.spec("dummy.add").unwrap().bl_idname, "DUMMY_OT_add");
    // Every curated namespace has dumped operators.
    for ns in [
        "object.",
        "mesh.",
        "transform.",
        "uv.",
        "export_scene.",
        "render.",
        "wm.",
    ] {
        assert!(r.known_idnames().iter().any(|i| i.starts_with(ns)), "{ns}");
    }
}

#[test]
fn duplicate_idnames_fail() {
    let e = Registry::build([&ADD, &ADD_AGAIN], Registry::typed_specs(), catalog())
        .err()
        .unwrap();
    assert_eq!(
        e,
        "duplicate operator registration: dummy.add (2 OperatorDefs share this idname)"
    );
}

#[test]
fn implemented_operator_needs_a_typed_spec() {
    let e = Registry::build([&FAKE_BEVEL], Registry::typed_specs(), catalog())
        .err()
        .unwrap();
    assert!(
        e.starts_with("operator mesh.select_nth is implemented but has no typed spec"),
        "{e}"
    );
    let e = Registry::build([&COPY_CUBE], Registry::typed_specs(), catalog())
        .err()
        .unwrap();
    assert!(
        e.starts_with("operator mesh.primitive_cube_add does not use its generated spec static"),
        "{e}"
    );
}

#[test]
fn global_registry_of_this_binary_has_no_operator() {
    // Nothing is registered in this test binary: every dumped operator is known.
    let r = Registry::global();
    assert!(r.idnames().is_empty());
    assert!(r.known_idnames().len() > 900, "{}", r.known_idnames().len());
}
