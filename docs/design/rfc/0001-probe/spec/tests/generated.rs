// SPDX-License-Identifier: GPL-3.0-or-later
//! The generated statics equal the §9.6 / §10.4 worked examples (values verified on Blender
//! 5.2.4) and are internally consistent. Nothing here is hand-edited: `src/generated/**` comes
//! from `frap-probe-codegen` (drift-checked in that crate's tests).

use frap_probe_rna::*;
use frap_probe_spec as spec;
use frap_probe_spec::ops::mesh::{OPSPEC_MESH_OT_PRIMITIVE_CUBE_ADD as CUBE, PrimitiveCubeAdd, primitive_cube_add::Align};

#[test]
fn object_location_matches_9_6() {
    let p = spec::OBJECT_LOCATION;
    assert_eq!(
        (p.identifier, p.name, p.description),
        ("location", "Location", "Location of the object")
    );
    assert_eq!(
        (p.subtype, p.unit, p.array),
        (Subtype::Translation, Unit::Length, ArrayShape::Fixed(3))
    );
    assert_eq!(p.flags, PropFlags::ANIMATABLE);
    let PropKind::Float {
        default: Defaults::Array(d),
        hard,
        soft,
        precision,
        storage,
    } = &p.kind
    else {
        panic!()
    };
    assert_eq!(*d, &[0.0, 0.0, 0.0]);
    assert_eq!(*hard, (-(f32::MAX as f64), f32::MAX as f64));
    assert_eq!(*soft, *hard);
    assert_eq!((*precision, *storage), (5, FloatStorage::F32));
    assert!(core::ptr::eq(spec::OBJECT.find_prop("location").unwrap(), p));
    assert_eq!(spec::OBJECT.base.unwrap().identifier, "ID");
    assert!(spec::OBJECT.find_prop("name").is_some(), "inherited from ID through `base`");
    assert_eq!(spec::OBJECT.name_property, Some("name"));
}

#[test]
fn primitive_cube_add_matches_10_4() {
    assert_eq!(
        (CUBE.idname, CUBE.bl_idname, CUBE.label),
        ("mesh.primitive_cube_add", "MESH_OT_primitive_cube_add", "Add Cube")
    );
    assert_eq!(CUBE.description, "Construct a cube mesh that consists of six square faces");
    assert_eq!(CUBE.options, OpOptions::REGISTER | OpOptions::UNDO);
    assert_eq!(
        CUBE.poll_matrix,
        PollMatrix {
            plain: [PollCell::Y; 5],
            view3d: [PollCell::Y; 5]
        }
    );
    let ids: Vec<_> = CUBE.props.iter().map(|p| p.identifier).collect();
    assert_eq!(
        ids,
        ["size", "calc_uvs", "enter_editmode", "align", "location", "rotation", "scale"]
    );
    let size = &CUBE.props[0];
    let PropKind::Float {
        default: Defaults::Scalar(d),
        hard,
        soft,
        ..
    } = &size.kind
    else {
        panic!()
    };
    assert_eq!((*d, *hard), (2.0, (0.0, 999999995904.0)));
    assert_eq!(*soft, (0.001f32 as f64, 100.0));
    assert_eq!((size.subtype, size.unit), (Subtype::Distance, Unit::Length));
    assert!(CUBE.props[2].flags.contains(PropFlags::HIDDEN | PropFlags::SKIP_SAVE));
    assert!(CUBE.props[6].flags.contains(PropFlags::HIDDEN | PropFlags::SKIP_SAVE));
    assert!(CUBE.props[4].flags.contains(PropFlags::SKIP_SAVE) && !CUBE.props[4].flags.contains(PropFlags::HIDDEN));
    assert_eq!(
        (CUBE.props[4].subtype, CUBE.props[5].subtype, CUBE.props[5].unit),
        (Subtype::XyzLength, Subtype::Euler, Unit::Rotation)
    );
    let PropKind::Enum {
        items,
        default: EnumDefault::Single(d),
        ..
    } = &CUBE.props[3].kind
    else {
        panic!()
    };
    assert_eq!(items.iter().map(|i| i.identifier).collect::<Vec<_>>(), ["WORLD", "VIEW", "CURSOR"]);
    assert_eq!(*d, "WORLD");
}

#[test]
fn typed_props_from_args() {
    let mut a = OpArgs::default();
    for p in CUBE.props {
        a.values.insert(p.identifier, default_value(p));
    }
    let loc = coerce(
        &CUBE.props[4],
        Input::Tuple(vec![Input::Int(0), Input::Int(0), Input::Int(1)]),
        &CoerceCx::OpArgs { owner: CUBE.bl_idname },
    )
    .unwrap();
    a.values.insert("location", loc.value);
    a.is_set.insert(4);
    let p = PrimitiveCubeAdd::from_args(&a);
    assert_eq!(p.size, Prop { value: 2.0, is_set: false });
    assert_eq!(
        p.location,
        Prop {
            value: [0.0, 0.0, 1.0],
            is_set: true
        }
    );
    assert_eq!(
        p.align,
        Prop {
            value: Align::World,
            is_set: false
        }
    );
    assert_eq!(p.scale.value, [0.0; 3]);
    assert!(core::ptr::eq(<PrimitiveCubeAdd as OpProps>::spec(), &CUBE));
}

#[test]
fn context_members_are_generated_props() {
    let o = spec::CONTEXT.find_prop("object").unwrap();
    assert!(o.flags.contains(PropFlags::CONTEXT_MEMBER | PropFlags::READONLY));
    assert!(matches!(
        o.kind,
        PropKind::Pointer {
            fixed_type: "Object",
            is_id: true
        }
    ));
    let s = spec::CONTEXT.find_prop("selected_objects").unwrap();
    assert!(matches!(s.kind, PropKind::Collection { item_type: "Object", .. }));
    // `scene` is a real RNA prop of Context (different read-only wording)
    assert!(!spec::CONTEXT.find_prop("scene").unwrap().flags.contains(PropFlags::CONTEXT_MEMBER));
}

#[test]
fn deprecation_is_carried() {
    let p = spec::MATERIAL.find_prop("use_nodes").unwrap();
    let d = p.deprecated.expect("Material.use_nodes is deprecated in 5.2");
    assert_eq!((d.version, d.removal_version), ([5, 0, 0], [6, 0, 0]));
}

/// Integrity survey of the generated subset. Prints the numbers recorded in RFC 0001.
#[test]
fn integrity_survey() {
    let mut bad_default = vec![];
    let mut empty_enums = vec![];
    let mut macro_mismatch = vec![];
    let all_ops = spec::ops::mesh::ALL.iter().chain(spec::ops::object::ALL.iter());
    for op in all_ops {
        let is_macro = op.options.contains(OpOptions::MACRO);
        if is_macro != !op.macro_steps.is_empty() {
            macro_mismatch.push(op.idname);
        }
        for p in op.props {
            if let PropKind::Enum {
                items,
                default: EnumDefault::Single(d),
                ..
            } = &p.kind
            {
                if items.is_empty() {
                    empty_enums.push(format!("{}.{}", op.idname, p.identifier));
                } else if !items.iter().any(|i| i.identifier == *d) {
                    bad_default.push(format!("{}.{} = {d:?}", op.idname, p.identifier));
                }
            }
        }
    }
    for s in spec::generated::rna::ALL.iter() {
        for p in s.props {
            if let PropKind::Enum {
                items,
                default: EnumDefault::Single(d),
                ..
            } = &p.kind
            {
                if items.is_empty() {
                    empty_enums.push(format!("{}.{}", s.identifier, p.identifier));
                } else if !items.iter().any(|i| i.identifier == *d) {
                    bad_default.push(format!("{}.{} = {d:?}", s.identifier, p.identifier));
                }
            }
        }
    }
    eprintln!("empty enum item lists ({}): {:?}", empty_enums.len(), empty_enums);
    eprintln!("enum default not among items ({}): {:?}", bad_default.len(), bad_default);
    eprintln!(
        "MACRO flag vs macro_steps mismatch ({}): {:?}",
        macro_mismatch.len(),
        macro_mismatch
    );
    assert!(macro_mismatch.is_empty());
}
