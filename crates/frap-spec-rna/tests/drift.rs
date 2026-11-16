// SPDX-License-Identifier: GPL-3.0-or-later
//! Drift test (ARCHITECTURE §4.2): every generated `StructDef` equals the committed Blender 5.2.4
//! dump in `spec/blender-5.2/rna/` (own props and functions in Blender order, base chain, name
//! property), plus the naming rule of RFC 0001 F-GEN-1 (`OBJECT`, `object::LOCATION`).

mod common;

use common::{check_prop, load};
use frap_rna::{PropFlags, PropKind, StructDef};
use frap_spec_rna as rna;

#[test]
fn structs_match_dumps() {
    let meta = load("blender-5.2/rna/_meta.json");
    let names = meta["structs"].as_array().unwrap();
    assert_eq!(names.len(), rna::ALL_STRUCTS.len());
    assert!(names.len() > 400, "the full struct set is dumped");
    for n in names {
        let n = n.as_str().unwrap();
        let s: &'static StructDef = rna::find_struct(n).unwrap_or_else(|| panic!("{n} missing"));
        let j = load(&format!("blender-5.2/rna/{n}.json"));
        assert_eq!(s.identifier, n);
        assert_eq!(j["name"], s.name, "{n}");
        assert_eq!(j["description"], s.description, "{n}");
        assert_eq!(j["base"].as_str(), s.base.map(|b| b.identifier), "{n} base");
        assert_eq!(j["name_property"].as_str(), s.name_property, "{n}");
        assert_eq!(j["is_id"].as_bool().unwrap(), s.is_id, "{n} is_id");
        let jp = j["props"].as_array().unwrap();
        assert!(s.props.len() >= jp.len(), "{n}: props missing");
        for (p, jp) in s.props.iter().zip(jp) {
            check_prop(n, p, jp);
        }
        // Appended: Python-only members (overlay) and context members (F-DEF-5) only.
        for extra in &s.props[jp.len()..] {
            assert!(
                extra.flags.contains(PropFlags::PYTHON_ONLY)
                    || extra.flags.contains(PropFlags::CONTEXT_MEMBER),
                "{n}.{}: not dumped",
                extra.identifier
            );
        }
        let jf = j["funcs"].as_array().unwrap();
        assert!(s.funcs.len() >= jf.len(), "{n}: funcs missing");
        for (f, jf) in s.funcs.iter().zip(jf) {
            assert_eq!(jf["identifier"], f.identifier, "{n}");
            assert_eq!(jf["description"], f.description, "{n}.{}", f.identifier);
            let ctx = format!("{n}.{}()", f.identifier);
            let params = jf["params"].as_array().unwrap();
            let returns = jf["returns"].as_array().unwrap();
            assert_eq!(params.len(), f.params.len(), "{ctx}");
            assert_eq!(returns.len(), f.returns.len(), "{ctx}");
            for (p, jp) in f
                .params
                .iter()
                .zip(params)
                .chain(f.returns.iter().zip(returns))
            {
                check_prop(&ctx, p, jp);
            }
            assert_eq!(
                jf["use_self"] == false,
                f.flags.contains(frap_rna::FuncFlags::NO_SELF),
                "{ctx} use_self"
            );
        }
    }
    // Sorted for binary search.
    assert!(
        rna::ALL_STRUCTS
            .windows(2)
            .all(|w| w[0].identifier < w[1].identifier)
    );
}

#[test]
fn naming_and_identity() {
    // §9.6 worked example and RFC 0001 F-GEN-1: module-scoped consts, pointer-identical to lookup.
    assert!(std::ptr::eq(
        rna::OBJECT.find_prop("location").unwrap(),
        rna::object::LOCATION
    ));
    assert!(std::ptr::eq(
        rna::OBJECT.find_prop("name").unwrap(),
        rna::id::NAME
    ));
    assert!(std::ptr::eq(
        rna::find_struct("Object").unwrap(),
        &rna::OBJECT
    ));
    assert_eq!(rna::OBJECT.base.unwrap().identifier, "ID");
    let loc = rna::object::LOCATION;
    assert_eq!(loc.subtype.as_str(), "TRANSLATION");
    assert_eq!(loc.unit.as_str(), "LENGTH");
    assert_eq!(loc.array, frap_rna::ArrayShape::Fixed(3));
    assert!(loc.flags.contains(PropFlags::ANIMATABLE));
    // F-GEN-1: the two props whose flat names collided are now distinct consts.
    assert_eq!(
        rna::nodes_modifier::BAKE_DIRECTORY.identifier,
        "bake_directory"
    );
    assert_eq!(rna::nodes_modifier_bake::DIRECTORY.identifier, "directory");
    // The stand-in names of M0 still resolve (used by W0 code).
    for s in [
        &rna::ID,
        &rna::MESH,
        &rna::CAMERA,
        &rna::LIGHT,
        &rna::MATERIAL,
        &rna::SCENE,
        &rna::COLLECTION,
        &rna::WORLD,
        &rna::IMAGE,
        &rna::ARMATURE,
        &rna::ACTION,
        &rna::KEY,
        &rna::NODE_TREE,
        &rna::TEXTURE,
        &rna::LIBRARY,
        &rna::POINT_CLOUD,
    ] {
        assert!(s.is_a("ID"), "{}", s.identifier);
    }
    // F-DEF-3 deprecation metadata.
    let use_nodes = rna::material::USE_NODES;
    let d = use_nodes.deprecated.unwrap();
    assert_eq!((d.version, d.removal_version), ([5, 0, 0], [6, 0, 0]));
    // ID.name length_max includes the NUL byte (Appendix A: 256).
    assert!(matches!(
        rna::id::NAME.kind,
        PropKind::String { max_bytes: 256, .. }
    ));
}

#[test]
fn context_members_and_python_only() {
    // F-DEF-5: `bpy.context.object` etc. are synthesised READONLY | CONTEXT_MEMBER props.
    for m in [
        "object",
        "active_object",
        "selected_objects",
        "edit_object",
        "objects_in_mode",
    ] {
        let p = rna::CONTEXT
            .find_prop(m)
            .unwrap_or_else(|| panic!("Context.{m}"));
        assert!(
            p.flags
                .contains(PropFlags::CONTEXT_MEMBER | PropFlags::READONLY),
            "{m}"
        );
    }
    assert!(
        !rna::context::SCENE
            .flags
            .contains(PropFlags::CONTEXT_MEMBER)
    );
    // Python-only members (python_only.toml).
    assert!(rna::object::CHILDREN.flags.contains(PropFlags::PYTHON_ONLY));
    let f = rna::MESH.find_func("from_pydata").unwrap();
    assert_eq!(f.params.len(), 4);
    assert!(f.flags.contains(frap_rna::FuncFlags::PYTHON_ONLY));
    let kf = rna::BPY_STRUCT_FUNCS
        .iter()
        .find(|f| f.identifier == "keyframe_insert")
        .unwrap();
    assert_eq!(kf.keyword_only_from, Some(1));
    // Dynamic enums take the overlay superset (render.engine lists only EEVEE in -b).
    let PropKind::Enum { items, dynamic, .. } = &rna::render_settings::ENGINE.kind else {
        panic!()
    };
    assert_eq!(
        items.iter().map(|i| i.identifier).collect::<Vec<_>>(),
        ["BLENDER_EEVEE", "BLENDER_WORKBENCH", "CYCLES"]
    );
    assert_eq!(*dynamic, Some("RenderSettings.engine"));
}
