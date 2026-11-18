// SPDX-License-Identifier: GPL-3.0-or-later
//! Registry self-check (ARCHITECTURE §10.5). Runs on every OS and in the release profile (LTO
//! is where `linkme` entries get dropped). The expected set comes from the source tree, never
//! from the binary: a dropped crate would vanish from every slice at once.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("invariant: crates/")
        .to_path_buf()
}

/// `.rs` file stems of a directory, skipping `_`-prefixed placeholders and `mod.rs`.
fn stems(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut v: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .filter(|s| !s.starts_with('_') && s != "mod")
        .collect();
    v.sort();
    v
}

/// `VertexGroups` → `vertex_groups` (the §21.1 `funcs/<struct>_<func>.rs` naming).
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Expected idnames from `crates/frap-ops-*/src/ops/*.rs` (§21.1: `ops/<op>.rs` in the crate's
/// primary namespace, `ops/<ns>__<op>.rs` otherwise).
fn expected_operators() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (krate, primary) in frap_ops_all::OPS_CRATES {
        let dir = crates_dir().join(krate).join("src/ops");
        for stem in stems(&dir) {
            let idname = match (stem.split_once("__"), primary) {
                (Some((ns, op)), _) => format!("{ns}.{op}"),
                (None, Some(p)) => format!("{p}.{stem}"),
                (None, None) => panic!(
                    "{krate}/src/ops/{stem}.rs: this crate has no primary namespace; name it <namespace>__<op>.rs (§21.1)"
                ),
            };
            if let Some(prev) = out.insert(idname.clone(), format!("{krate}/src/ops/{stem}.rs")) {
                panic!("{idname} has two files: {prev} and {krate}/src/ops/{stem}.rs");
            }
        }
    }
    out
}

/// Expected RNA function file stems from every `crates/*/src/funcs/*.rs`.
fn expected_funcs() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for e in std::fs::read_dir(crates_dir()).expect("crates/").flatten() {
        out.extend(stems(&e.path().join("src/funcs")));
    }
    out
}

#[test]
fn every_operator_file_is_registered_and_nothing_else() {
    frap_ops_all::link();
    let expected = expected_operators();
    let mut registered = BTreeSet::new();
    for def in frap_op::OPERATORS.iter() {
        assert!(
            registered.insert(def.spec.idname),
            "duplicate operator registration: {}",
            def.spec.idname
        );
    }
    let exp: BTreeSet<&str> = expected.keys().map(String::as_str).collect();
    let missing: Vec<_> = exp.difference(&registered).collect();
    let extra: Vec<_> = registered.difference(&exp).collect();
    assert!(
        missing.is_empty(),
        "operator files whose registration is missing from the binary (dropped by the linker?): {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "registered operators without an ops/ file: {extra:?}"
    );
    // Registry::global() refuses duplicates and indexes every entry.
    let r = frap_op::Registry::global();
    assert_eq!(r.idnames().len(), registered.len());
    // M1 W0 baseline: the two M0 operators survive the move into their ops crates.
    assert!(registered.contains("mesh.primitive_cube_add"));
    assert!(registered.contains("export_scene.gltf"));
}

#[test]
fn every_rna_function_file_is_registered_and_unique() {
    frap_ops_all::link();
    let mut seen = BTreeSet::new();
    let mut stems_registered = BTreeSet::new();
    for f in frap_scene::RNA_FUNCS.iter() {
        assert!(
            seen.insert((f.srna, f.func)),
            "duplicate RNA function {}.{}",
            f.srna,
            f.func
        );
        stems_registered.insert(format!("{}_{}", snake(f.srna), f.func));
    }
    assert_eq!(stems_registered, expected_funcs());
}

#[test]
fn hooks_are_unique_and_complete() {
    frap_ops_all::link();
    let mut hooks = BTreeSet::new();
    for h in frap_scene::RNA_HOOKS.iter() {
        assert!(
            hooks.insert((h.srna, h.prop)),
            "duplicate hook {}.{}",
            h.srna,
            h.prop
        );
        assert!(
            h.get.is_some() || h.set.is_some(),
            "hook {}.{} has neither get nor set",
            h.srna,
            h.prop
        );
    }
    let decls: BTreeSet<(&str, &str)> = frap_rna::testing::HOOK_DECLS
        .iter()
        .map(|d| (d.srna, d.prop))
        .collect();
    let undeclared: Vec<_> = hooks.difference(&decls).collect();
    let unimplemented: Vec<_> = decls.difference(&hooks).collect();
    assert!(
        unimplemented.is_empty(),
        "`=> hook` bindings without an RNA_HOOKS entry: {unimplemented:?}"
    );
    assert!(
        undeclared.is_empty(),
        "RNA_HOOKS entries no binding declares: {undeclared:?}"
    );
}

#[test]
fn file_name_mapping_follows_section_21_1() {
    assert_eq!(snake("VertexGroups"), "vertex_groups");
    assert_eq!(snake("Object"), "object");
    assert_eq!(snake("BlendDataMaterials"), "blend_data_materials");
}

/// Every `crates/frap-modifiers/src/types/<type>.rs` registers exactly its type (§7.2: the file
/// stem is the lower-case Blender identifier, `simple_deform.rs` → `SIMPLE_DEFORM`), once, and
/// survives linking (`frap_modifiers::link`, M3+M4 W0).
#[test]
fn every_modifier_type_file_is_registered_once() {
    frap_ops_all::link();
    let expected: BTreeSet<String> = stems(&crates_dir().join("frap-modifiers/src/types"))
        .into_iter()
        .map(|s| s.to_ascii_uppercase())
        .collect();
    let mut registered = BTreeSet::new();
    for info in frap_modifiers::MODIFIER_TYPES.iter() {
        let id = info.ty.identifier().to_owned();
        assert!(
            registered.insert(id.clone()),
            "modifier type {id} registered twice"
        );
    }
    assert_eq!(
        registered, expected,
        "registered modifier types differ from the `types/<type>.rs` files"
    );
}
