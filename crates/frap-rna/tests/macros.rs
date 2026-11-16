// SPDX-License-Identifier: GPL-3.0-or-later
//! `rna_struct!` on test structs: value, read-only, computed, hook, hook-coll, Python-only,
//! set-hook, ID, nested-struct, embedded, collection, ID-collection and SoA-proxy bindings;
//! `check_bindings` (accepting and rejecting); `HOOK_DECLS` emission; `#[rna_func]` and
//! `#[rna_hook]` registration (into test slices through `krate = …`).

mod support;

use std::collections::BTreeSet;

use frap_math::glam::Mat4;
use frap_rna::testing::{
    BindingDecl, BindingKind, HOOK_DECLS, RnaBindings, binding_problems, check_bindings,
};
use frap_rna::traits::RnaStruct;
use frap_rna::*;
use smol_str::SmolStr;
use support::*;

// --- RNA definitions of the test structs ------------------------------------------------------

static FLAG_ITEMS: [EnumItem; 3] = [item("A", 1), item("B", 2), item("C", 4)];
static KIND_ITEMS: [EnumItem; 2] = [item("MESH", 0), item("EMPTY", 1)];

static THING_PROPS: [PropDef; 21] = [
    float(
        "location",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Translation,
        NONE,
    ),
    enumeration("rotation_mode", &ROTATION_MODES, NONE),
    float(
        "rotation_euler",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Euler,
        NONE,
    ),
    float(
        "scale",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Xyz,
        NONE,
    ),
    int("count", ArrayShape::Scalar, (0, 100), NONE),
    boolean("visible", ArrayShape::Scalar, NONE),
    enumeration("flags", &FLAG_ITEMS, PropFlags::ENUM_FLAG),
    float(
        "matrix",
        ArrayShape::Matrix(4, 4),
        (-FLT_MAX, FLT_MAX),
        Subtype::Matrix,
        NONE,
    ),
    enumeration("type", &KIND_ITEMS, RO),
    string("label", 64, NN),
    float(
        "area",
        ArrayShape::Scalar,
        (0.0, FLT_MAX),
        Subtype::None,
        RO,
    ),
    enumeration("mode", &KIND_ITEMS, NONE),
    collection("slots", "Material", RO),
    collection("children", "Thing", PropFlags::PYTHON_ONLY.union(RO)),
    pointer("data", "ID", true, NONE),
    pointer("settings", "ThingSettings", false, NN),
    pointer("node_tree", "NodeTree", true, RO),
    collection("items", "ThingItem", NONE),
    collection("materials", "Material", NONE),
    collection("points", "ThingPoint", NONE),
    enumeration("order", &ROTATION_MODES, NONE),
];
static THING: StructDef = st("Thing", Some(&ID), &THING_PROPS, Some("name"), true);

static SETTINGS_PROPS: [PropDef; 1] = [float(
    "size",
    ArrayShape::Scalar,
    (0.0, 10.0),
    Subtype::None,
    NONE,
)];
static SETTINGS: StructDef = st("ThingSettings", None, &SETTINGS_PROPS, None, false);

static ITEM_PROPS: [PropDef; 2] = [
    string("name", 64, NN),
    int("value", ArrayShape::Scalar, (-10, 10), NONE),
];
static ITEM: StructDef = st("ThingItem", None, &ITEM_PROPS, Some("name"), false);

static POINT_PROPS: [PropDef; 4] = [
    float(
        "co",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Translation,
        NONE,
    ),
    int("index", ArrayShape::Scalar, (0, i32::MAX as i64), RO),
    boolean("select", ArrayShape::Scalar, NONE),
    boolean("hide", ArrayShape::Scalar, NONE),
];
static POINT: StructDef = st("ThingPoint", None, &POINT_PROPS, None, false);

// --- Rust data ------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Settings {
    size: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    name: SmolStr,
}

#[derive(Clone, Debug, Default)]
pub struct ThingItem {
    name: SmolStr,
    value: i32,
}

#[derive(Clone, Debug)]
pub struct Thing {
    name: SmolStr,
    location: [f32; 3],
    rotation_mode: SmolStr,
    rotation_euler: [f32; 3],
    scale: [f32; 3],
    count: i32,
    visible: bool,
    flags: BTreeSet<SmolStr>,
    matrix: Mat4,
    label: String,
    data: Option<IdAny>,
    settings: Settings,
    node_tree: Tree,
    items: Vec<ThingItem>,
    materials: Vec<Option<IdRef<MaterialId>>>,
    co: Vec<[f32; 3]>,
    select: Vec<bool>,
    order: frap_rna::EulerOrder,
}

impl Thing {
    fn new() -> Self {
        Thing {
            name: "Thing".into(),
            location: [1.0, 2.0, 3.0],
            rotation_mode: "ZYX".into(),
            rotation_euler: [0.1, 0.2, 0.3],
            scale: [1.0; 3],
            count: 5,
            visible: true,
            flags: ["A".into()].into_iter().collect(),
            matrix: Mat4::from_translation(frap_math::glam::Vec3::new(7.0, 8.0, 9.0)),
            label: "hello".into(),
            data: None,
            settings: Settings { size: 2.0 },
            node_tree: Tree {
                name: "Shader Nodetree".into(),
            },
            items: vec![
                ThingItem {
                    name: "first".into(),
                    value: 1,
                },
                ThingItem {
                    name: "second".into(),
                    value: 2,
                },
            ],
            materials: vec![None, Some(IdRef::from_key(IdKey::next()))],
            co: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            select: vec![false, true, false],
            order: frap_rna::EulerOrder::XYZ,
        }
    }
    fn object_type(&self) -> &'static str {
        "MESH"
    }
    fn set_position(&mut self, i: usize, v: [f32; 3]) {
        self.co[i] = v;
    }
}

rna_struct! { Settings as SETTINGS {
    size => size,
}}

rna_struct! { Tree as NODE_TREE {
    name => name (set = hook),
}}

rna_struct! { ThingItem as ITEM {
    name => name,
    value => value,
}}

rna_struct! { Thing as THING {
    name => name (set = hook),
    location => location (tag = Vector),
    rotation_mode => hook,
    rotation_euler => rotation_euler (tag = Euler(rotation_mode)),
    scale => scale,
    count => count,
    visible => visible,
    flags => flags,
    matrix => matrix,
    type => readonly(object_type()),
    label => get(|t| t.label.to_uppercase()) set(|t, v: String| t.label = v.to_lowercase()),
    area => readonly(|t| t.scale[0] * t.scale[1]),
    mode => hook(on = "ID"),
    slots => hook_coll,
    children => python_only hook,
    data => id(data),
    settings => struct(settings),
    node_tree => embedded(node_tree, NodeTree),
    items => coll(items, api = "ThingItems"),
    materials => id_coll(materials, item = MATERIAL),
    order => order,
    points => soa(pub ThingPoint as POINT, len = |t| t.co.len(),
                  foreach_get = |t, prop, range| {
                      if prop.identifier == "co" {
                          Ok(Bulk::F32(t.co[range].iter().flatten().copied().collect()))
                      } else {
                          Err(traits::wrong_access(&POINT, prop.identifier, "bulk-readable"))
                      }
                  }) {
        co => get(|t, i| t.co[i]) set(|t, i, v| t.set_position(i, v)) (tag = Vector),
        index => get(|_t, i| i as i32),
        select => get(|t, i| t.select[i]) set(|t, i, v| { t.select[i] = v; }),
        hide => hook,
    },
}}

fn p(id: &str) -> &'static PropDef {
    prop(&THING, id)
}

fn value(t: &Thing, id: &str) -> Value {
    match t.get(p(id)).unwrap() {
        Field::Value(v) => v,
        _ => panic!("{id} is not a value"),
    }
}

// --- Tests ----------------------------------------------------------------------------------

#[test]
fn value_bindings_get_and_set() {
    let mut t = Thing::new();
    assert_eq!(t.srna().identifier, "Thing");
    assert!(std::ptr::eq(<Thing as RnaStruct>::struct_def(), &THING));
    assert_eq!(
        value(&t, "location"),
        Value::FloatArray {
            data: [1.0, 2.0, 3.0].into_iter().collect(),
            shape: ArrayShape::Fixed(3),
            tag: MathTag::Vector
        }
    );
    // Euler tag reads its order from the owner.
    let Value::FloatArray { tag, .. } = value(&t, "rotation_euler") else {
        panic!()
    };
    assert_eq!(tag, MathTag::Euler(frap_rna::EulerOrder::ZYX));
    // Default tag comes from the subtype.
    let Value::FloatArray { tag, .. } = value(&t, "scale") else {
        panic!()
    };
    assert_eq!(tag, MathTag::Vector);
    let Value::FloatArray { data, tag, shape } = value(&t, "matrix") else {
        panic!()
    };
    assert_eq!((tag, shape), (MathTag::Matrix, ArrayShape::Matrix(4, 4)));
    assert_eq!(&data[0..4], [1.0, 0.0, 0.0, 7.0], "row-major");
    assert_eq!(value(&t, "count"), Value::Int(5));
    assert_eq!(value(&t, "visible"), Value::Bool(true));
    assert_eq!(
        value(&t, "flags"),
        Value::EnumSet(["A".into()].into_iter().collect())
    );
    assert_eq!(value(&t, "order"), Value::Enum("XYZ".into()));
    assert_eq!(value(&t, "name"), Value::Str("Thing".into()));

    // Writes go through coerce first, as the session does.
    let cx = CoerceCx::RnaSet { owner: "Thing" };
    let set = |t: &mut Thing, id: &str, input: Input| {
        let v = coerce(p(id), input, &cx).unwrap().value;
        t.set(p(id), v).unwrap()
    };
    assert_eq!(
        set(
            &mut t,
            "location",
            Input::Tuple(vec![Input::Int(4), Input::Int(5), Input::Int(6)])
        ),
        SetOutcome::Done
    );
    assert_eq!(t.location, [4.0, 5.0, 6.0]);
    set(&mut t, "count", Input::Int(500));
    assert_eq!(t.count, 100, "clamped by coerce");
    set(&mut t, "visible", Input::Bool(false));
    assert!(!t.visible);
    set(
        &mut t,
        "flags",
        Input::Set(vec![Input::Str("B".into()), Input::Str("C".into())]),
    );
    assert_eq!(t.flags.len(), 2);
    set(&mut t, "order", Input::Str("YZX".into()));
    assert_eq!(t.order, frap_rna::EulerOrder::YZX);
    let rows = MathValue::Matrix(vec![
        vec![1.0, 0.0, 0.0, 1.0],
        vec![0.0, 1.0, 0.0, 2.0],
        vec![0.0, 0.0, 1.0, 3.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ]);
    set(&mut t, "matrix", Input::Math(rows));
    assert_eq!(t.matrix.w_axis.truncate().to_array(), [1.0, 2.0, 3.0]);
    // `(set = hook)`: read from the field, written by a hook.
    assert_eq!(
        set(&mut t, "name", Input::Str("Other".into())),
        SetOutcome::NeedsHook
    );
    assert_eq!(t.name, "Thing");
    // Unknown property.
    let bogus = float("bogus", ArrayShape::Scalar, (0.0, 1.0), Subtype::None, NONE);
    let bogus: &'static PropDef = Box::leak(Box::new(bogus));
    let e = t.get(bogus).err().unwrap();
    assert_eq!(e.message, "'Thing' object has no attribute 'bogus'");
    assert_eq!(e.kind, RnaErrorKind::NoAttribute);
}

#[test]
fn read_only_and_computed_bindings() {
    let mut t = Thing::new();
    assert_eq!(value(&t, "type"), Value::Enum("MESH".into()));
    let e = t.set(p("type"), Value::Enum("EMPTY".into())).unwrap_err();
    assert_eq!(
        e.message,
        "bpy_struct: attribute \"type\" from \"Thing\" is read-only"
    );
    assert_eq!(value(&t, "area"), Value::Float(1.0));
    assert!(t.set(p("area"), Value::Float(2.0)).is_err());
    // get/set closures.
    assert_eq!(value(&t, "label"), Value::Str("HELLO".into()));
    t.set(p("label"), Value::Str("WoRlD".into())).unwrap();
    assert_eq!(t.label, "world");
}

#[test]
fn hook_bindings() {
    let mut t = Thing::new();
    for id in ["rotation_mode", "mode", "slots", "children"] {
        assert!(matches!(t.get(p(id)).unwrap(), Field::NeedsHook), "{id}");
    }
    assert_eq!(
        t.set(p("rotation_mode"), Value::Enum("XYZ".into()))
            .unwrap(),
        SetOutcome::NeedsHook
    );
    assert_eq!(
        t.set(p("mode"), Value::Enum("MESH".into())).unwrap(),
        SetOutcome::NeedsHook
    );
    assert!(t.coll_mut(p("slots")).is_err());
}

#[test]
fn hook_decls_are_emitted() {
    let decls: BTreeSet<(&str, &str, bool)> = HOOK_DECLS
        .iter()
        .map(|d| (d.srna, d.prop, d.coll))
        .collect();
    for want in [
        ("Thing", "name", false),
        ("Thing", "rotation_mode", false),
        ("ID", "mode", false),
        ("Thing", "slots", true),
        ("Thing", "children", false),
        ("ThingPoint", "hide", false),
        ("NodeTree", "name", false),
    ] {
        assert!(
            decls.contains(&want),
            "missing HookDecl {want:?} in {decls:?}"
        );
    }
    // Plain value bindings declare nothing.
    assert!(
        !decls
            .iter()
            .any(|(s, p, _)| *s == "Thing" && *p == "location")
    );
}

#[test]
fn id_struct_and_embedded_bindings() {
    let mut t = Thing::new();
    assert!(matches!(t.get(p("data")).unwrap(), Field::Id(None)));
    let mesh = IdAny {
        code: IdCode::ME,
        key: IdKey::next(),
    };
    t.set(p("data"), Value::Id(Some(mesh))).unwrap();
    assert!(matches!(t.get(p("data")).unwrap(), Field::Id(Some(a)) if a == mesh));
    // Nested struct: borrowed for reads, reborrowed for writes.
    let Field::Struct(s) = t.get(p("settings")).unwrap() else {
        panic!()
    };
    assert_eq!(s.srna().identifier, "ThingSettings");
    assert_eq!(
        s.get(prop(&SETTINGS, "size"))
            .ok()
            .map(|f| matches!(f, Field::Value(Value::Float(v)) if v == 2.0)),
        Some(true)
    );
    drop(s);
    t.struct_mut(p("settings"))
        .unwrap()
        .set(prop(&SETTINGS, "size"), Value::Float(3.5))
        .unwrap();
    assert_eq!(t.settings.size, 3.5);
    assert!(
        t.set(p("settings"), Value::None).is_err(),
        "struct pointers are read-only"
    );
    // Embedded ID.
    let Field::Embedded(slot, tree) = t.get(p("node_tree")).unwrap() else {
        panic!()
    };
    assert_eq!(slot, EmbeddedSlot::NodeTree);
    assert_eq!(tree.srna().identifier, "NodeTree");
    drop(tree);
    assert!(t.struct_mut(p("node_tree")).is_ok());
    assert!(t.struct_mut(p("location")).is_err());
}

#[test]
fn collection_bindings() {
    let mut t = Thing::new();
    let Field::Coll(c) = t.get(p("items")).unwrap() else {
        panic!()
    };
    assert_eq!(c.item_type().identifier, "ThingItem");
    assert_eq!(c.len(), 2);
    assert_eq!(c.key_of(1).as_deref(), Some("second"));
    assert_eq!(c.find("second"), Some(1));
    assert_eq!(c.find("nope"), None);
    assert_eq!(c.item_seg(0), Seg::Key("first".into()));
    let Item::Struct(it) = c.item(0).unwrap() else {
        panic!()
    };
    assert!(matches!(
        it.get(prop(&ITEM, "value")).unwrap(),
        Field::Value(Value::Int(1))
    ));
    drop(it);
    let e = c.item(5).err().unwrap();
    assert_eq!(
        e.message,
        "bpy_prop_collection[index]: index 5 out of range, size 2"
    );
    // Default bulk read loops over items.
    assert_eq!(
        c.foreach_get(prop(&ITEM, "value"), 0..2).unwrap(),
        Bulk::I32(vec![1, 2])
    );
    drop(c);
    {
        let mut cm = t.coll_mut(p("items")).unwrap();
        cm.item_mut(1)
            .unwrap()
            .set(prop(&ITEM, "value"), Value::Int(9))
            .unwrap();
        cm.foreach_set(prop(&ITEM, "value"), &Bulk::I32(vec![3, 4]))
            .unwrap();
        let e = cm
            .foreach_set(prop(&ITEM, "value"), &Bulk::I32(vec![3]))
            .unwrap_err();
        assert_eq!(e.message, "internal error setting the array");
    }
    assert_eq!((t.items[0].value, t.items[1].value), (3, 4));
    // ID collection: items are IDs (or None), names resolved by the caller.
    let Field::Coll(c) = t.get(p("materials")).unwrap() else {
        panic!()
    };
    assert_eq!(c.item_type().identifier, "Material");
    assert!(matches!(c.item(0).unwrap(), Item::Id(None)));
    assert!(matches!(
        c.item(1).unwrap(),
        Item::Id(Some(IdAny {
            code: IdCode::MA,
            ..
        }))
    ));
    assert_eq!(c.key_of(1), None);
    assert_eq!(c.item_seg(1), Seg::Index(1));
    drop(c);
    assert!(t.coll_mut(p("materials")).unwrap().item_mut(0).is_err());
}

#[test]
fn soa_proxies() {
    let mut t = Thing::new();
    let co = prop(&POINT, "co");
    let Field::Coll(c) = t.get(p("points")).unwrap() else {
        panic!()
    };
    assert_eq!((c.len(), c.item_type().identifier), (3, "ThingPoint"));
    assert_eq!(c.key_of(0), None);
    assert_eq!(c.item_seg(2), Seg::Index(2));
    let Item::Struct(v) = c.item(1).unwrap() else {
        panic!()
    };
    assert_eq!(v.srna().identifier, "ThingPoint");
    assert!(
        matches!(v.get(co).unwrap(), Field::Value(Value::FloatArray { ref data, tag: MathTag::Vector, .. }) if data[..] == [1.0, 0.0, 0.0])
    );
    assert!(matches!(
        v.get(prop(&POINT, "index")).unwrap(),
        Field::Value(Value::Int(1))
    ));
    assert!(matches!(
        v.get(prop(&POINT, "select")).unwrap(),
        Field::Value(Value::Bool(true))
    ));
    assert!(matches!(
        v.get(prop(&POINT, "hide")).unwrap(),
        Field::NeedsHook
    ));
    drop(v);
    // Overridden bulk read.
    assert_eq!(
        c.foreach_get(co, 1..3).unwrap(),
        Bulk::F32(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0])
    );
    assert!(c.foreach_get(prop(&POINT, "select"), 0..3).is_err());
    assert!(c.item(3).is_err());
    drop(c);
    {
        let mut cm = t.coll_mut(p("points")).unwrap();
        let mut it = cm.item_mut(2).unwrap();
        it.set(
            co,
            Value::FloatArray {
                data: [5.0, 6.0, 7.0].into_iter().collect(),
                shape: ArrayShape::Fixed(3),
                tag: MathTag::Vector,
            },
        )
        .unwrap();
        it.set(prop(&POINT, "select"), Value::Bool(true)).unwrap();
        assert!(it.set(prop(&POINT, "index"), Value::Int(0)).is_err());
        assert_eq!(
            it.set(prop(&POINT, "hide"), Value::Bool(true)).unwrap(),
            SetOutcome::NeedsHook
        );
        drop(it);
        // Default bulk write goes through item_mut().set().
        cm.foreach_set(prop(&POINT, "select"), &Bulk::Bool(vec![true, true, true]))
            .unwrap();
        assert!(cm.item_mut(3).is_err());
    }
    assert_eq!(t.co[2], [5.0, 6.0, 7.0]);
    assert_eq!(t.select, [true, true, true]);
    // The generated proxy types are nameable.
    let r = ThingPointRef { owner: &t, i: 0 };
    assert_eq!(r.srna().identifier, "ThingPoint");
    let _ = ThingPointColl { owner: &t }.len();
}

#[test]
fn check_bindings_accepts_correct_bindings() {
    check_bindings::<Thing>();
    check_bindings::<Settings>();
    check_bindings::<ThingItem>();
    check_bindings::<Tree>();
    let kinds: Vec<(&str, BindingKind)> =
        Thing::bindings().iter().map(|b| (b.prop, b.kind)).collect();
    assert!(kinds.contains(&("type", BindingKind::ReadOnly)));
    assert!(kinds.contains(&("name", BindingKind::SetHook)));
    assert!(kinds.contains(&("slots", BindingKind::HookColl)));
    assert!(kinds.contains(&("children", BindingKind::PythonOnlyHook)));
    assert!(kinds.contains(&("points", BindingKind::Soa)));
    assert!(kinds.contains(&("materials", BindingKind::IdColl)));
}

// A struct whose bindings are wrong in every way check_bindings can detect.
#[derive(Clone, Debug, Default)]
pub struct Bad {
    location: i32,
    count: [f32; 3],
    scale: [f32; 4],
    visible: Option<IdAny>,
    order: SmolStr,
    name: SmolStr,
    items: Vec<ThingItem>,
}

static WRONG_ITEMS: [EnumItem; 2] = [item("XYZ", 1), item("QUATERNION", 0)];
static BAD_PROPS: [PropDef; 9] = [
    float(
        "location",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Translation,
        NONE,
    ),
    int("count", ArrayShape::Scalar, (0, 100), NONE),
    float(
        "scale",
        ArrayShape::Fixed(3),
        (-FLT_MAX, FLT_MAX),
        Subtype::Xyz,
        NONE,
    ),
    boolean("visible", ArrayShape::Scalar, NONE),
    enumeration("order", &WRONG_ITEMS, NONE),
    enumeration("euler_order", &WRONG_ITEMS, NONE),
    string("name", 64, RO),
    collection("items", "Material", NONE),
    collection("hooked", "Material", NONE),
];
static BAD: StructDef = st("Bad", None, &BAD_PROPS, None, false);

impl Bad {
    fn euler_order(&self) -> frap_rna::EulerOrder {
        frap_rna::EulerOrder::XYZ
    }
}

rna_struct! { Bad as BAD {
    location => location,
    count => count,
    scale => scale,
    visible => id(visible),
    order => order,
    euler_order => readonly(euler_order()),
    name => name,
    items => coll(items),
    hooked => python_only hook,
    missing => order,
}}

#[test]
fn check_bindings_reports_mismatches() {
    let problems = binding_problems::<Bad>();
    let all = problems.join("\n");
    for want in [
        "Bad.location: location is FLOAT[3] but is bound to a Rust i32",
        "Bad.count: count is INT but is bound to a Rust [f32; 3]",
        "Bad.scale: scale is FLOAT[3] but is bound to a Rust [f32; 4]",
        "Bad.visible: BOOLEAN property bound as id(…)",
        "Bad.euler_order: euler_order: enum item 'XZY' of EulerOrder is not in the RNA items",
        "Bad.name: read-only property bound with a setter",
        "Bad.items: collection of Material bound with items of ThingItem",
        "Bad.hooked: bound as python_only but the property is not PYTHON_ONLY",
        "Bad.missing: no such property in the RNA definition",
    ] {
        assert!(
            problems.iter().any(|p| p == want),
            "missing problem {want:?} in:\n{all}"
        );
    }
    // `order` (a SmolStr bound to an enum) is fine.
    assert!(!all.contains("Bad.order"), "{all}");
    let r = std::panic::catch_unwind(check_bindings::<Bad>);
    assert!(r.is_err());
}

#[test]
fn check_bindings_reports_duplicates() {
    struct Dup;
    impl RnaStruct for Dup {
        fn struct_def() -> &'static StructDef {
            &SETTINGS
        }
    }
    impl RnaBindings for Dup {
        fn bindings() -> &'static [BindingDecl] {
            static B: [BindingDecl; 2] = [
                BindingDecl {
                    prop: "size",
                    kind: BindingKind::Hook,
                    check: None,
                    item: None,
                    soa: None,
                },
                BindingDecl {
                    prop: "size",
                    kind: BindingKind::Hook,
                    check: None,
                    item: None,
                    soa: None,
                },
            ];
            &B
        }
    }
    assert_eq!(
        binding_problems::<Dup>(),
        ["ThingSettings.size: bound twice"]
    );
}

// --- #[rna_func] / #[rna_hook] ---------------------------------------------------------------

/// Stand-in for the frap-scene registry types (frap-rna cannot depend on frap-scene).
mod fake_scene {
    use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value};

    pub struct Host;
    pub struct HostRead;
    pub struct FuncArgs;
    pub type RnaFuncImpl = fn(&mut Host, &RnaPtr, &mut FuncArgs) -> Result<Value, RnaError>;
    pub type RnaSetHook = fn(&mut Host, &RnaPtr, &'static PropDef, Value) -> Result<(), RnaError>;
    pub type RnaGetHook =
        for<'h> fn(&'h HostRead, &RnaPtr, &'static PropDef) -> Result<Field<'h>, RnaError>;
    pub struct RnaFuncReg {
        pub srna: &'static str,
        pub func: &'static str,
        pub imp: RnaFuncImpl,
    }
    pub struct RnaHookReg {
        pub srna: &'static str,
        pub prop: &'static str,
        pub get: Option<RnaGetHook>,
        pub set: Option<RnaSetHook>,
    }
    #[allow(unsafe_code)]
    #[linkme::distributed_slice]
    pub static RNA_FUNCS: [RnaFuncReg];
    #[allow(unsafe_code)]
    #[linkme::distributed_slice]
    pub static RNA_HOOKS: [RnaHookReg];
}

use fake_scene::{FuncArgs, Host, HostRead};

#[rna_func(srna = "Thing", func = "select_set", krate = crate::fake_scene)]
fn select_set(_h: &mut Host, _this: &RnaPtr, _a: &mut FuncArgs) -> Result<Value, RnaError> {
    Ok(Value::Bool(true))
}

#[rna_func("Thing", "hide_get", krate = crate::fake_scene)]
fn hide_get(_h: &mut Host, _this: &RnaPtr, _a: &mut FuncArgs) -> Result<Value, RnaError> {
    Ok(Value::Bool(false))
}

#[rna_hook(srna = "Thing", prop = "rotation_mode", krate = crate::fake_scene)]
fn rotation_mode_set(
    _h: &mut Host,
    _this: &RnaPtr,
    _p: &'static PropDef,
    _v: Value,
) -> Result<(), RnaError> {
    Ok(())
}

fn slots_get<'h>(
    _h: &'h HostRead,
    _this: &RnaPtr,
    _p: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    Ok(Field::Value(Value::Int(42)))
}

#[rna_hook(srna = "Thing", prop = "slots", get = slots_get, krate = crate::fake_scene)]
fn slots_set(
    _h: &mut Host,
    _this: &RnaPtr,
    _p: &'static PropDef,
    _v: Value,
) -> Result<(), RnaError> {
    Ok(())
}

#[rna_hook(srna = "ID", prop = "mode", krate = crate::fake_scene)]
mod mode_hook {
    use super::*;
    pub fn get<'h>(
        _h: &'h HostRead,
        _this: &RnaPtr,
        _p: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        Ok(Field::NeedsHook)
    }
    pub fn set(
        _h: &mut Host,
        _this: &RnaPtr,
        _p: &'static PropDef,
        _v: Value,
    ) -> Result<(), RnaError> {
        Ok(())
    }
}

#[test]
fn rna_func_and_rna_hook_register() {
    let funcs: BTreeSet<(&str, &str)> = fake_scene::RNA_FUNCS
        .iter()
        .map(|f| (f.srna, f.func))
        .collect();
    assert_eq!(
        funcs,
        [("Thing", "hide_get"), ("Thing", "select_set")]
            .into_iter()
            .collect()
    );
    let f = fake_scene::RNA_FUNCS
        .iter()
        .find(|f| f.func == "select_set")
        .unwrap();
    let this = RnaPtr::new(Root::Data, &THING);
    assert_eq!(
        (f.imp)(&mut Host, &this, &mut FuncArgs).unwrap(),
        Value::Bool(true)
    );

    let hooks: Vec<_> = fake_scene::RNA_HOOKS
        .iter()
        .map(|h| (h.srna, h.prop, h.get.is_some(), h.set.is_some()))
        .collect();
    assert_eq!(hooks.len(), 3);
    assert!(hooks.contains(&("Thing", "rotation_mode", false, true)));
    assert!(hooks.contains(&("Thing", "slots", true, true)));
    assert!(hooks.contains(&("ID", "mode", true, true)));
    let slots = fake_scene::RNA_HOOKS
        .iter()
        .find(|h| h.prop == "slots")
        .unwrap();
    let r = (slots.get.unwrap())(&HostRead, &this, p("slots")).unwrap();
    assert!(matches!(r, Field::Value(Value::Int(42))));
    assert!((slots.set.unwrap())(&mut Host, &this, p("slots"), Value::None).is_ok());
}
