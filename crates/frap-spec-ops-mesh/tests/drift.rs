// SPDX-License-Identifier: GPL-3.0-or-later
//! Drift test (ARCHITECTURE §4.2): every generated `OperatorSpec` of `frap-spec-ops-mesh` equals
//! its Blender 5.2.4 dump (props, options, macro steps, poll matrix), and the typed props
//! structs exist exactly for the OPERATORS.md operators of this group with `M` ≤ the current
//! milestone (the full cross-crate check is `cargo test -p xtask`).

#[path = "../../frap-spec-rna/tests/common/mod.rs"]
mod common;

#[test]
fn typed_specs_match_dumps() {
    let all = frap_spec_ops_mesh::ALL_OPS;
    assert_eq!(
        all.len(),
        156,
        "typed operators of the `mesh` group in M8 (M8 W0)"
    );
    for s in all {
        common::check_op(s);
    }
    assert!(all.windows(2).all(|w| w[0].idname < w[1].idname));
}

/// `PrimitiveCubeAdd` equals the worked example of ARCHITECTURE §10.4.
#[test]
fn primitive_cube_add_equals_10_4() {
    use frap_rna::{
        ArrayShape, Defaults, EnumDefault, FloatStorage, MathTag, OpArgs, OpOptions, OpProps,
        PollCell, Prop, PropFlags, PropKind, Subtype, Unit, Value,
    };
    use frap_spec_ops_mesh::mesh::{
        OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD, PrimitiveCubeAdd, primitive_cube_add::Align,
    };

    assert_eq!(PrimitiveCubeAdd::IDNAME, "mesh.primitive_cube_add");
    assert_eq!(PrimitiveCubeAdd::BL_IDNAME, "MESH_OT_primitive_cube_add");
    let s = PrimitiveCubeAdd::spec();
    assert!(std::ptr::eq(s, &OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD));
    assert_eq!(s.label, "Add Cube");
    assert_eq!(
        s.description,
        "Construct a cube mesh that consists of six square faces"
    );
    assert_eq!(s.options, OpOptions::REGISTER | OpOptions::UNDO);
    let pm = s.poll_matrix.unwrap();
    assert_eq!(pm.plain, [PollCell::Y; 5]);
    assert!(s.macro_steps.is_empty());
    let ids: Vec<&str> = s.props.iter().map(|p| p.identifier).collect();
    assert_eq!(
        ids,
        [
            "size",
            "calc_uvs",
            "enter_editmode",
            "align",
            "location",
            "rotation",
            "scale"
        ]
    );

    let size = &s.props[0];
    assert_eq!(
        (size.subtype, size.unit, size.array),
        (Subtype::Distance, Unit::Length, ArrayShape::Scalar)
    );
    let PropKind::Float {
        default: Defaults::Scalar(d),
        hard,
        soft,
        storage,
        ..
    } = &size.kind
    else {
        panic!()
    };
    assert_eq!(*d, 2.0);
    assert_eq!(*hard, (0.0, 999999995904.0));
    assert_eq!(soft.1, 100.0);
    assert!((soft.0 - 0.001).abs() < 1e-9);
    assert_eq!(*storage, FloatStorage::F32);
    assert!(matches!(
        s.props[1].kind,
        PropKind::Bool {
            default: Defaults::Scalar(true)
        }
    ));
    assert!(
        s.props[2]
            .flags
            .contains(PropFlags::HIDDEN | PropFlags::SKIP_SAVE)
    );
    let PropKind::Enum {
        items,
        default,
        dynamic,
    } = &s.props[3].kind
    else {
        panic!()
    };
    assert_eq!(
        items.iter().map(|i| i.identifier).collect::<Vec<_>>(),
        ["WORLD", "VIEW", "CURSOR"]
    );
    assert!(matches!(default, EnumDefault::Item("WORLD")));
    assert!(dynamic.is_none());
    for (i, sub, unit) in [
        (4, Subtype::XyzLength, Unit::Length),
        (5, Subtype::Euler, Unit::Rotation),
        (6, Subtype::XyzLength, Unit::Length),
    ] {
        let p = &s.props[i];
        assert_eq!(
            (p.subtype, p.unit, p.array),
            (sub, unit, ArrayShape::Fixed(3)),
            "{}",
            p.identifier
        );
        assert!(p.flags.contains(PropFlags::SKIP_SAVE));
        assert!(matches!(
            p.kind,
            PropKind::Float {
                default: Defaults::Array(&[0.0, 0.0, 0.0]),
                ..
            }
        ));
    }
    assert!(s.props[6].flags.contains(PropFlags::HIDDEN));

    // The typed struct: field names and types of §10.4, extracted field by field.
    let mut a = OpArgs::default();
    let v3 = |x: f32, y: f32, z: f32, tag| Value::FloatArray {
        data: [x, y, z].into_iter().collect(),
        shape: ArrayShape::Fixed(3),
        tag,
    };
    a.values.insert("size", Value::Float(3.0));
    a.values.insert("calc_uvs", Value::Bool(true));
    a.values.insert("enter_editmode", Value::Bool(false));
    a.values.insert("align", Value::Enum("CURSOR".into()));
    a.values
        .insert("location", v3(0.0, 0.0, 1.0, MathTag::Vector));
    a.values.insert(
        "rotation",
        v3(0.0, 0.0, 0.0, MathTag::Euler(frap_rna::EulerOrder::XYZ)),
    );
    a.values.insert("scale", v3(0.0, 0.0, 0.0, MathTag::Vector));
    a.is_set.insert(0);
    a.is_set.insert(4);
    let p = PrimitiveCubeAdd::from_args(&a);
    let size: &Prop<f32> = &p.size;
    let calc_uvs: &Prop<bool> = &p.calc_uvs;
    let enter: &Prop<bool> = &p.enter_editmode;
    let align: &Prop<Align> = &p.align;
    type V3 = Prop<[f32; 3]>;
    let (loc, rot, scale): (&V3, &V3, &V3) = (&p.location, &p.rotation, &p.scale);
    assert_eq!((size.value, size.is_set), (3.0, true));
    assert_eq!((calc_uvs.value, calc_uvs.is_set), (true, false));
    assert!(!enter.value);
    assert_eq!(align.value, Align::Cursor);
    assert_eq!(align.clone().map(|a| a.identifier()).value, "CURSOR");
    assert_eq!((loc.value, loc.is_set), ([0.0, 0.0, 1.0], true));
    assert!(!rot.is_set && !scale.is_set);
    assert_eq!(Align::ALL, [Align::World, Align::View, Align::Cursor]);
    assert_eq!(Align::default(), Align::World);
}

/// All ten M1 primitives are typed.
#[test]
fn every_primitive_is_typed() {
    let ids: Vec<&str> = frap_spec_ops_mesh::ALL_OPS
        .iter()
        .map(|s| s.idname)
        .collect();
    for p in [
        "plane",
        "cube",
        "circle",
        "uv_sphere",
        "ico_sphere",
        "cylinder",
        "cone",
        "torus",
        "grid",
        "monkey",
    ] {
        assert!(
            ids.contains(&format!("mesh.primitive_{p}_add").as_str()),
            "{p}"
        );
    }
}
