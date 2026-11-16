// SPDX-License-Identifier: GPL-3.0-or-later
//! `crates/frap-spec-rna/src/generated/**`: one `StructDef` static per dumped struct at the
//! crate root (`frap_spec_rna::OBJECT`) and one module per struct holding its property consts
//! (`frap_spec_rna::object::LOCATION`), RFC 0001 F-GEN-1 / decision W0-5.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use super::Files;
use super::model::{Res, Spec};
use super::rust::{EnumPool, func_lit, header, ident, lit_str, prop_lit, screaming, snake};

const DIR: &str = "crates/frap-spec-rna/src/generated";
const IMPORTS: &str = "use frap_rna::{ArrayShape, Defaults, Deprecation, EnumDefault, FloatStorage, FuncDef, FuncFlags, PropDef, PropFlags, PropKind, StructDef, Subtype, Unit};\n";

/// Names reserved inside a struct module.
const TABLES: [&str; 2] = ["RNA_PROPS", "RNA_FUNCS"];

pub fn generate(spec: &Spec, files: &mut Files) -> Res<()> {
    let root = spec.root.join(DIR);
    let mut pool = EnumPool::default();
    let mut statics: BTreeMap<String, String> = BTreeMap::new(); // SCREAMING -> identifier
    let mut modules: BTreeMap<String, String> = BTreeMap::new(); // module -> identifier
    for id in spec.structs.keys() {
        if let Some(prev) = statics.insert(screaming(id), id.clone()) {
            return Err(format!(
                "struct static name collision: {prev} and {id} both map to {}",
                screaming(id)
            ));
        }
        if let Some(prev) = modules.insert(snake(id), id.clone()) {
            return Err(format!(
                "struct module name collision: {prev} and {id} both map to {}",
                snake(id)
            ));
        }
    }
    if modules.contains_key("enums") {
        return Err("a struct module collides with the generated `enums` module".into());
    }
    for reserved in ["ALL_STRUCTS", "BPY_STRUCT_FUNCS"] {
        if statics.contains_key(reserved) {
            return Err(format!(
                "struct static {reserved} collides with a generated table"
            ));
        }
    }

    let mut root_mod = header(
        "`frap-spec-rna` generated tables: one `StructDef` static per dumped Blender 5.2 struct (here)\n//! and one module per struct with its property consts (`object::LOCATION`).",
    );
    root_mod.push_str(
        "#![allow(non_upper_case_globals, unused_imports, clippy::all)]\n\nuse frap_rna::{FuncDef, StructDef};\n\npub mod enums;\n\n",
    );
    for (id, st) in &spec.structs {
        let m = snake(id);
        let up = screaming(id);
        let mut s = header(&format!("`bpy.types.{id}`: property tables and consts."));
        s.push_str("#![allow(non_upper_case_globals, unused_imports, clippy::all)]\n");
        s.push_str(IMPORTS);
        s.push('\n');
        let mut consts = BTreeSet::new();
        for p in &st.props {
            let c = p.identifier.to_ascii_uppercase();
            if TABLES.contains(&c.as_str()) || !consts.insert(c.clone()) {
                return Err(format!("{id}.{}: const name collision ({c})", p.identifier));
            }
        }
        let _ = writeln!(
            s,
            "/// Own properties of `{id}` (base-chain properties live in the base struct)."
        );
        let _ = writeln!(s, "pub static RNA_PROPS: [PropDef; {}] = [", st.props.len());
        for p in &st.props {
            let _ = writeln!(
                s,
                "    {},",
                prop_lit(p, &mut pool, "crate::generated::enums")?
            );
        }
        s.push_str("];\n");
        let _ = writeln!(s, "/// Own functions of `{id}`.");
        let _ = writeln!(s, "pub static RNA_FUNCS: [FuncDef; {}] = [", st.funcs.len());
        for f in &st.funcs {
            let _ = writeln!(
                s,
                "    {},",
                func_lit(f, &mut pool, "crate::generated::enums")?
            );
        }
        s.push_str("];\n\n");
        for (i, p) in st.props.iter().enumerate() {
            let _ = writeln!(
                s,
                "/// `{id}.{}`\npub const {}: &PropDef = &RNA_PROPS[{i}];",
                p.identifier,
                p.identifier.to_ascii_uppercase()
            );
        }
        files.insert(root.join(format!("{m}.rs")), s.into_bytes());

        let base = match &st.base {
            Some(b) => format!("Some(&{})", screaming(b)),
            None => "None".into(),
        };
        let _ = writeln!(
            root_mod,
            "#[path = \"{m}.rs\"]\npub mod {};\n/// `bpy.types.{id}`\npub static {up}: StructDef = StructDef {{ identifier: {}, base: {base}, name: {}, description: {}, props: &{}::RNA_PROPS, funcs: &{}::RNA_FUNCS, name_property: {}, is_id: {} }};",
            ident(&m),
            lit_str(id),
            lit_str(&st.name),
            lit_str(&st.description),
            ident(&m),
            ident(&m),
            match &st.name_property {
                Some(n) => format!("Some({})", lit_str(n)),
                None => "None".into(),
            },
            st.is_id
        );
    }
    let _ = writeln!(
        root_mod,
        "\n/// Every dumped struct, sorted by identifier (`find_struct` binary-searches it).\npub static ALL_STRUCTS: [&StructDef; {}] = [",
        spec.structs.len()
    );
    for id in spec.structs.keys() {
        let _ = writeln!(root_mod, "    &{},", screaming(id));
    }
    root_mod.push_str("];\n\n");
    root_mod.push_str(
        "/// Looks a struct up by its Blender identifier (`\"Object\"`).\npub fn find_struct(identifier: &str) -> Option<&'static StructDef> {\n    ALL_STRUCTS.binary_search_by(|s| s.identifier.cmp(identifier)).ok().map(|i| ALL_STRUCTS[i])\n}\n\n",
    );
    root_mod.push_str("use frap_rna::{ArrayShape, Defaults, EnumDefault, FloatStorage, FuncFlags, PropDef, PropFlags, PropKind, Subtype, Unit};\n");
    let _ = writeln!(
        root_mod,
        "/// Python-only `bpy_struct` methods valid on every struct (`spec/overlay/python_only.toml`).\npub static BPY_STRUCT_FUNCS: [FuncDef; {}] = [",
        spec.bpy_struct_funcs.len()
    );
    for f in &spec.bpy_struct_funcs {
        let _ = writeln!(
            root_mod,
            "    {},",
            func_lit(f, &mut pool, "crate::generated::enums")?
        );
    }
    root_mod.push_str("];\n");
    files.insert(root.join("mod.rs"), root_mod.into_bytes());
    files.insert(
        root.join("enums.rs"),
        pool.module("Deduplicated enum item tables of `frap-spec-rna`.")
            .into_bytes(),
    );
    Ok(())
}
