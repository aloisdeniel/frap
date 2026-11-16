// SPDX-License-Identifier: GPL-3.0-or-later
//! Binding checks (§4.3): every hand-written `rna_struct!` binding of WP 1.10 matches its
//! `StructDef`; the hand-written ones (`Light` per type, `Modifier`, `Context`) and the
//! generated plain settings structs are checked property by property.

mod support;

use frap_rna::testing::check_bindings;
use frap_rna::{ArrayShape, Field, PropDef, PropKind, Rna, StructDef, Value};
use frap_scene::generated::plain::*;
use frap_scene::id::{
    Camera, Collection, Image, Library, Light, Material, Modifier, ModifierSettings, Object, Scene,
    World,
};
use frap_scene::{HostRead, LayerCollection, ViewLayer, WindowState};

#[test]
fn rna_struct_bindings_match_their_struct_defs() {
    check_bindings::<Object>();
    check_bindings::<Scene>();
    check_bindings::<ViewLayer>();
    check_bindings::<LayerCollection>();
    check_bindings::<Collection>();
    check_bindings::<Camera>();
    check_bindings::<World>();
    check_bindings::<Material>();
    check_bindings::<Image>();
    check_bindings::<Library>();
    check_bindings::<WindowState>();
}

/// The `Value` a property reads as must have the property's type and array length.
fn value_matches(prop: &PropDef, v: &Value) -> bool {
    let n = prop.array_len();
    match (&prop.kind, v) {
        (PropKind::Bool { .. }, Value::Bool(_)) => matches!(prop.array, ArrayShape::Scalar),
        (PropKind::Bool { .. }, Value::BoolArray(a)) => a.len() == n,
        (PropKind::Int { .. }, Value::Int(_)) => matches!(prop.array, ArrayShape::Scalar),
        (PropKind::Int { .. }, Value::IntArray(a)) => a.len() == n,
        (PropKind::Float { .. }, Value::Float(_) | Value::Double(_)) => {
            matches!(prop.array, ArrayShape::Scalar)
        }
        (PropKind::Float { .. }, Value::FloatArray { data, .. }) => {
            matches!(prop.array, ArrayShape::Dynamic) || data.len() == n
        }
        (PropKind::String { .. }, Value::Str(_)) => true,
        // Item membership is not checked: several enums list only what `-b` reports
        // (`UnitSettings.length_unit` dumps `DEFAULT` only while the factory stores
        // `METERS`), which is the dump's business, not the binding's.
        (PropKind::Enum { .. }, Value::Enum(_)) => matches!(prop.array, ArrayShape::Scalar),
        (PropKind::Enum { .. }, Value::EnumSet(_)) => true,
        _ => false,
    }
}

/// Reads every property of `srna` (base chain included) from `s`: values must match their
/// definitions; returns how many were bound.
fn check_reads(s: &dyn Rna, srna: &'static StructDef) -> usize {
    let mut bound = 0;
    for prop in srna.all_props() {
        match s.get(prop) {
            Ok(Field::Value(v)) => {
                assert!(
                    value_matches(prop, &v),
                    "{}.{}: {:?} does not match {} {:?}",
                    srna.identifier,
                    prop.identifier,
                    v,
                    prop.type_str(),
                    prop.array
                );
                bound += 1;
            }
            Ok(_) => bound += 1,
            Err(e) => assert!(
                matches!(e.kind, frap_rna::RnaErrorKind::NoAttribute),
                "{}.{}: {}",
                srna.identifier,
                prop.identifier,
                e.message
            ),
        }
    }
    bound
}

/// `Light` is re-typed by `type` (`PointLight`/`SunLight`/`SpotLight`/`AreaLight`): every
/// property of each refined struct reads with the right type, values survive a re-type, and
/// another type's properties are refused (`'PointLight' object has no attribute 'spot_size'`,
/// verified on 5.2.4).
#[test]
fn light_bindings_for_every_type() {
    for (ty, srna) in [
        ("POINT", &frap_spec_rna::POINT_LIGHT),
        ("SUN", &frap_spec_rna::SUN_LIGHT),
        ("SPOT", &frap_spec_rna::SPOT_LIGHT),
        ("AREA", &frap_spec_rna::AREA_LIGHT),
    ] {
        let l = Light::new("", ty);
        assert_eq!(l.srna().identifier, srna.identifier);
        let n = check_reads(&l, srna);
        assert!(n > 25, "{ty}: only {n} properties bound");
    }
    let mut l = Light::new("", "SPOT");
    let size = frap_spec_rna::SPOT_LIGHT.find_prop("spot_size").unwrap();
    let energy = frap_spec_rna::SPOT_LIGHT.find_prop("energy").unwrap();
    frap_rna::RnaMut::set(&mut l, energy, Value::Float(5.0)).unwrap();
    let ty = frap_spec_rna::LIGHT.find_prop("type").unwrap();
    frap_rna::RnaMut::set(&mut l, ty, Value::Enum("POINT".into())).unwrap();
    assert_eq!(l.srna().identifier, "PointLight");
    let e = l
        .get(size)
        .err()
        .expect("spot_size is not a PointLight property");
    assert_eq!(
        e.message,
        "'PointLight' object has no attribute 'spot_size'"
    );
    let energy = frap_spec_rna::POINT_LIGHT.find_prop("energy").unwrap();
    assert!(matches!(l.get(energy), Ok(Field::Value(Value::Float(5.0)))));
}

/// Generated plain structs (`RenderSettings`, `View3DCursor`, …): every stored property reads
/// with its definition's type and shape.
#[test]
fn generated_settings_structs_read_their_definitions() {
    let structs: Vec<Box<dyn Rna>> = vec![
        Box::new(RenderSettings::default()),
        Box::new(ImageFormatSettings::default()),
        Box::new(ColorManagedViewSettings::default()),
        Box::new(ColorManagedDisplaySettings::default()),
        Box::new(ColorManagedInputColorspaceSettings::default()),
        Box::new(View3DShading::default()),
        Box::new(SceneDisplay::default()),
        Box::new(SceneEEVEE::default()),
        Box::new(ToolSettings::default()),
        Box::new(UnitSettings::default()),
        Box::new(View3DCursor::default()),
        Box::new(CameraDOFSettings::default()),
    ];
    for s in &structs {
        let n = check_reads(&**s, s.srna());
        assert!(n > 0, "{}: nothing bound", s.srna().identifier);
    }
    // Camera fields through the Camera binding.
    assert!(check_reads(&Camera::new(""), &frap_spec_rna::CAMERA) > 50);
}

#[test]
fn modifier_binding_is_refined_by_type() {
    let m = Modifier {
        name: "Bevel".into(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: true,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid: 1,
        settings: ModifierSettings::new(
            frap_scene::generated::modifier_settings::ModifierType::Bevel,
        ),
    };
    assert_eq!(m.srna().identifier, "BevelModifier");
    assert!(check_reads(&m, m.srna()) > 20);
    let ty = frap_spec_rna::MODIFIER.find_prop("type").unwrap();
    assert!(matches!(m.get(ty), Ok(Field::Value(Value::Enum(t))) if t == "BEVEL"));
}

/// Every context member frap answers is a `Context` property of the right kind, and the
/// verified reprs of the struct-valued ones (F-TR-2).
#[test]
fn context_members() {
    let h = support::TestHost::factory();
    let ctx = h.ctx();
    let srna = &frap_spec_rna::CONTEXT;
    let get = |m: &str| ctx.get(srna.find_prop(m).unwrap());
    for m in [
        "window",
        "blend_data",
        "scene",
        "view_layer",
        "engine",
        "collection",
        "layer_collection",
        "tool_settings",
        "mode",
        "object",
        "active_object",
        "edit_object",
        "pose_object",
        "selected_objects",
        "selected_editable_objects",
        "visible_objects",
        "selectable_objects",
        "editable_objects",
        "objects_in_mode",
        "objects_in_mode_unique_data",
        "active_bone",
        "active_pose_bone",
        "selected_bones",
        "selected_pose_bones",
    ] {
        assert!(get(m).is_ok(), "context.{m}");
    }
    let repr = |m: &str| match get(m).unwrap() {
        Field::Ptr(p) => p.repr(&h.main),
        Field::Id(Some(a)) => frap_rna::RnaPtr::id(a, &frap_spec_rna::ID).repr(&h.main),
        _ => panic!("{m} is not a pointer"),
    };
    assert_eq!(
        repr("tool_settings"),
        "bpy.data.scenes['Scene'].tool_settings"
    );
    assert_eq!(
        repr("view_layer"),
        "bpy.data.scenes['Scene'].view_layers[\"ViewLayer\"]"
    );
    assert_eq!(repr("collection"), "bpy.data.collections['Collection']");
    assert_eq!(repr("scene"), "bpy.data.scenes['Scene']");
    assert_eq!(repr("object"), "bpy.data.objects['Cube']");
    assert_eq!(
        repr("layer_collection"),
        "bpy.data.scenes['Scene'].view_layers[\"ViewLayer\"].layer_collection.children[\"Collection\"]"
    );
    assert!(matches!(get("mode").unwrap(), Field::Value(Value::Enum(m)) if m == "OBJECT"));
    assert!(matches!(get("edit_object").unwrap(), Field::Id(None)));
    let Field::Coll(sel) = get("selected_objects").unwrap() else {
        panic!()
    };
    assert_eq!(sel.len(), 1);
    let Field::Coll(vis) = get("visible_objects").unwrap() else {
        panic!()
    };
    assert_eq!(vis.len(), 3);
    let Field::Coll(m) = get("objects_in_mode").unwrap() else {
        panic!()
    };
    assert_eq!(m.len(), 0);
    // UI members are not implemented headless; unknown names are AttributeErrors.
    let e = get("area").err().unwrap();
    assert_eq!(e.exception.as_str(), "NotImplementedError");
    // The master collection is the context collection of an empty file.
    let h = support::TestHost::new(frap_scene::startup::empty());
    let ctx = h.ctx();
    match ctx.get(srna.find_prop("collection").unwrap()).unwrap() {
        Field::Ptr(p) => assert_eq!(p.repr(&h.main), "bpy.data.scenes['Scene'].collection"),
        _ => panic!("the master collection is an embedded pointer"),
    }
}
