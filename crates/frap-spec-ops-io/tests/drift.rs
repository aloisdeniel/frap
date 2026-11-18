// SPDX-License-Identifier: GPL-3.0-or-later
//! Drift test (ARCHITECTURE §4.2): every generated `OperatorSpec` of `frap-spec-ops-io` equals
//! its Blender 5.2.4 dump (props, options, macro steps, poll matrix), and the typed props
//! structs exist exactly for the OPERATORS.md operators of this group with `M` ≤ the current
//! milestone (the full cross-crate check is `cargo test -p xtask`).

#[path = "../../frap-spec-rna/tests/common/mod.rs"]
mod common;

#[test]
fn typed_specs_match_dumps() {
    let all = frap_spec_ops_io::ALL_OPS;
    assert_eq!(all.len(), 8, "typed operators of the `io` group in M2");
    for s in all {
        common::check_op(s);
    }
    assert!(all.windows(2).all(|w| w[0].idname < w[1].idname));
}

/// The glTF settings structs carry all 110 / 20 props (ARCHITECTURE §4.2, §17.2) and keep the
/// M0 field types the exporter uses.
#[test]
fn gltf_settings_structs() {
    use frap_rna::{OpArgs, OpProps, Prop, Value};
    use frap_spec_ops_io::export_scene::ExportSceneGltf;
    use frap_spec_ops_io::import_scene::ImportSceneGltf;
    use smol_str::SmolStr;
    assert_eq!(ExportSceneGltf::spec().props.len(), 110);
    assert_eq!(ImportSceneGltf::spec().props.len(), 20);
    let s = ExportSceneGltf::spec();
    let mut a = OpArgs::default();
    for p in s.props {
        a.values.insert(p.identifier, Value::None);
    }
    a.values.insert("filepath", Value::Str("out/m0.glb".into()));
    a.values.insert("export_format", Value::Enum("GLB".into()));
    let p = ExportSceneGltf::from_args(&a);
    let f: &Prop<SmolStr> = &p.filepath;
    let e: &Prop<SmolStr> = &p.export_format;
    assert_eq!(f.value, "out/m0.glb");
    assert_eq!(e.value, "GLB");
    assert!(s.prop("export_gpu_instances").is_some());
    let fmt = s.prop("export_format").unwrap();
    assert!(fmt.flags.contains(frap_rna::PropFlags::DYNAMIC_ENUM));
}
