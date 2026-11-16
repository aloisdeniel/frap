// SPDX-License-Identifier: GPL-3.0-or-later
//! Every bound mesh property read through frap's RNA equals Blender 5.2.4's read of the same
//! mesh after the same RNA writes (`blender_reads.json`, made by `dump_reads.py`), item by
//! item and through `foreach_get`.

use std::sync::Arc;

use frap_rna::{CoerceCx, Field, Input, RnaCollMut, Value};
use serde_json::Value as J;

use super::*;
use crate::mesh::{AttrData, Attribute, Domain};

const FIXTURE: &str = include_str!("blender_reads.json");

/// Builds the fixture mesh and applies the writes of `dump_reads.py` through the bindings.
pub(super) fn fixture_mesh() -> Mesh {
    let verts = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.3],
        [0.0, 1.0, 0.0],
        [2.0, 0.5, 0.1],
        [0.5, 2.0, 0.2],
        [-1.0, 1.5, 0.0],
        [3.0, 3.0, 3.0],
    ];
    let faces = [vec![0, 1, 2, 3], vec![1, 4, 2], vec![3, 2, 4, 5, 6]];
    let mut m = Mesh::new("T");
    m.from_pydata(&verts, &[[7, 0]], &faces, true);
    apply_writes(&mut m);
    m
}

/// `attributes.new` (WP 1.22) as Blender 5.2.4 does it: appended, default values, and the
/// first UV map / colour attribute becomes the active (and render) one.
fn new_attribute(m: &mut Mesh, name: &str, domain: Domain, data: AttrData) {
    let n = columns::domain_len(m, domain);
    let data = data.default_of_len(n);
    let (uv, color) = (
        domain == Domain::Corner && matches!(data, AttrData::Float2(_)),
        matches!(data, AttrData::FloatColor(_) | AttrData::ByteColor(_)),
    );
    m.attributes_mut().insert(Attribute {
        name: name.into(),
        domain,
        data,
    });
    if uv && m.uv_active.is_none() {
        m.uv_active = Some(name.into());
        m.uv_active_render = Some(name.into());
    }
    if color && m.color_active.is_none() {
        m.color_active = Some(name.into());
        m.color_default = Some(name.into());
    }
}

fn zeros<T>() -> Arc<Vec<T>> {
    Arc::new(Vec::new())
}

/// The `# WRITES` block of `dump_reads.py`.
fn apply_writes(m: &mut Mesh) {
    set_elem(m, "vertices", 1, "select", Input::Bool(false));
    set_elem(m, "vertices", 2, "hide", Input::Bool(true));
    set_elem(m, "edges", 0, "use_seam", Input::Bool(true));
    set_elem(m, "edges", 1, "use_edge_sharp", Input::Bool(true));
    set_elem(m, "polygons", 0, "use_smooth", Input::Bool(true));
    set_elem(m, "polygons", 1, "material_index", Input::Int(2));
    set_elem(m, "vertices", 7, "co", ints(&[3, 2, 1]));

    new_attribute(m, "w", Domain::Point, AttrData::Float(zeros()));
    attr_foreach_set(
        m,
        "w",
        "value",
        Bulk::F32((0..8).map(|i| i as f32 * 0.5).collect()),
    );
    new_attribute(m, "bc", Domain::Corner, AttrData::ByteColor(zeros()));
    let bc = (0..48).map(|i| ((i * 7) % 11) as f32 / 10.0).collect();
    attr_foreach_set(m, "bc", "color", Bulk::F32(bc));
    new_attribute(m, "fc", Domain::Point, AttrData::FloatColor(zeros()));
    let fc = (0..32).map(|i| ((i * 3) % 7) as f32 / 4.0).collect();
    attr_foreach_set(m, "fc", "color", Bulk::F32(fc));
    set_attr_elem(m, "fc", 1, "color_srgb", floats(&[0.5, 0.25, 0.75, 1.0]));
    new_attribute(m, "UVMap", Domain::Corner, AttrData::Float2(zeros()));
    let uv = (0..24).map(|i| i as f32 * 0.125).collect();
    attr_foreach_set(m, "UVMap", "vector", Bulk::F32(uv));
    {
        let mut uvs = m.coll_mut(prop(&MESH, "uv_layers")).unwrap();
        let k = uvs.find("UVMap").unwrap();
        let mut layer = uvs.item_mut(k).unwrap();
        let data = prop(&spec::MESH_UV_LOOP_LAYER, "data");
        let mut data = layer.coll_mut(data).unwrap();
        set(&mut *data.item_mut(3).unwrap(), "pin_uv", Input::Bool(true));
    }
    new_attribute(m, "q", Domain::Face, AttrData::Quaternion(zeros()));
    set_attr_elem(m, "q", 1, "value", floats(&[0.5, 0.5, 0.5, 0.5]));
    new_attribute(m, "m4", Domain::Edge, AttrData::Float4x4(zeros()));
    // `attributes.new` fills FLOAT4X4 with zeros (verified), not the identity.
    if let Some(Attribute {
        data: AttrData::Float4x4(v),
        ..
    }) = m.attributes_mut().get_mut("m4")
    {
        Arc::make_mut(v).iter_mut().for_each(|x| *x = [[0.0; 4]; 4]);
    }
    let rows = Input::Tuple(
        [
            [1, 2, 3, 4],
            [5, 6, 7, 8],
            [9, 10, 11, 12],
            [13, 14, 15, 16],
        ]
        .iter()
        .map(|r| ints(r))
        .collect(),
    );
    set_attr_elem(m, "m4", 0, "value", rows);
    new_attribute(m, "s2", Domain::Corner, AttrData::Int16x2(zeros()));
    set_attr_elem(m, "s2", 0, "value", ints(&[3, -4]));
    new_attribute(m, "i8", Domain::Point, AttrData::Int8(zeros()));
    set_attr_elem(m, "i8", 5, "value", Input::Int(-7));
    new_attribute(m, "w2", Domain::Point, AttrData::Float(zeros()));
    set_attr(m, "w", "name", Input::Str("w2".into()));
}

fn ints(v: &[i64]) -> Input {
    Input::Tuple(v.iter().map(|x| Input::Int(*x)).collect())
}

fn floats(v: &[f64]) -> Input {
    Input::Tuple(v.iter().map(|x| Input::Float(*x)).collect())
}

/// Coerces like an assignment and writes.
pub(super) fn set(t: &mut dyn RnaMut, name: &str, input: Input) {
    let srna = t.srna();
    let p = srna
        .find_prop(name)
        .unwrap_or_else(|| panic!("{}.{name}", srna.identifier));
    let cx = CoerceCx::RnaSet {
        owner: srna.identifier,
    };
    let v = frap_rna::coerce(p, input, &cx).unwrap().value;
    assert_eq!(t.set(p, v).unwrap(), SetOutcome::Done);
}

pub(super) fn set_elem(m: &mut Mesh, coll: &str, i: usize, name: &str, input: Input) {
    let mut c = m.coll_mut(prop(&MESH, coll)).unwrap();
    set(&mut *c.item_mut(i).unwrap(), name, input);
}

fn set_attr(m: &mut Mesh, attr: &str, name: &str, input: Input) {
    let mut c = m.coll_mut(prop(&MESH, "attributes")).unwrap();
    let k = c.find(attr).unwrap();
    set(&mut *c.item_mut(k).unwrap(), name, input);
}

fn with_attr_data<R>(m: &mut Mesh, attr: &str, f: impl FnOnce(&mut dyn RnaCollMut) -> R) -> R {
    let mut c = m.coll_mut(prop(&MESH, "attributes")).unwrap();
    let k = c.find(attr).unwrap();
    let mut a = c.item_mut(k).unwrap();
    let data = a.srna().find_prop("data").unwrap();
    let mut d = a.coll_mut(data).unwrap();
    f(&mut *d)
}

fn set_attr_elem(m: &mut Mesh, attr: &str, i: usize, name: &str, input: Input) {
    with_attr_data(m, attr, |d| set(&mut *d.item_mut(i).unwrap(), name, input));
}

fn attr_foreach_set(m: &mut Mesh, attr: &str, name: &str, data: Bulk) {
    with_attr_data(m, attr, |d| {
        let p = d.item_type_of(0).find_prop(name).unwrap();
        d.foreach_set(p, &data).unwrap();
    });
}

/// Item struct of item 0 (refined), for property lookup.
trait ItemType {
    fn item_type_of(&self, i: usize) -> &'static StructDef;
}

impl<T: RnaColl + ?Sized> ItemType for T {
    fn item_type_of(&self, i: usize) -> &'static StructDef {
        match self.item(i) {
            Ok(Item::Struct(s)) => s.srna(),
            _ => self.item_type(),
        }
    }
}

/// A flattened scalar, for comparisons with the JSON dump.
#[derive(Clone, Debug, PartialEq)]
enum S {
    N(f64),
    B(bool),
    T(String),
}

fn flat_json(j: &J) -> Vec<S> {
    match j {
        J::Bool(b) => vec![S::B(*b)],
        J::Number(n) => vec![S::N(n.as_f64().unwrap())],
        J::String(s) => vec![S::T(s.clone())],
        J::Array(a) => a.iter().flat_map(flat_json).collect(),
        other => panic!("unexpected {other}"),
    }
}

fn flat_value(v: &Value) -> Vec<S> {
    match v {
        Value::Bool(b) => vec![S::B(*b)],
        Value::Int(i) => vec![S::N(*i as f64)],
        Value::Float(f) => vec![S::N(f64::from(*f))],
        Value::Double(f) => vec![S::N(*f)],
        Value::Str(s) | Value::Enum(s) => vec![S::T(s.to_string())],
        Value::BoolArray(a) => a.iter().map(|b| S::B(*b)).collect(),
        Value::IntArray(a) => a.iter().map(|i| S::N(*i as f64)).collect(),
        Value::FloatArray { data, .. } => data.iter().map(|f| S::N(f64::from(*f))).collect(),
        other => panic!("unexpected {other:?}"),
    }
}

fn flat_bulk(b: &Bulk) -> Vec<S> {
    match b {
        Bulk::F32(v) => v.iter().map(|f| S::N(f64::from(*f))).collect(),
        Bulk::I32(v) => v.iter().map(|i| S::N(f64::from(*i))).collect(),
        Bulk::Bool(v) => v.iter().map(|b| S::B(*b)).collect(),
        other => panic!("unexpected {other:?}"),
    }
}

fn close(what: &str, f: &[S], b: &[S], tol: f64) -> Result<(), String> {
    if f.len() != b.len() {
        return Err(format!("{what}: frap {f:?} != blender {b:?} (length)"));
    }
    for (k, (x, y)) in f.iter().zip(b).enumerate() {
        let ok = match (x, y) {
            (S::N(x), S::N(y)) => (x - y).abs() <= tol,
            // Blender's foreach_get into a list of ints/floats gives numbers for booleans.
            (S::B(x), S::N(y)) => f64::from(u8::from(*x)) == *y,
            _ => x == y,
        };
        if !ok {
            return Err(format!("{what}[{k}]: frap {x:?} != blender {y:?}"));
        }
    }
    Ok(())
}

/// Tolerance per property: frap ports Blender's arithmetic for everything but the normals
/// (WP 1.4, matched to 1e-6 there) and the OCIO-based FLOAT_COLOR `color_srgb`.
fn tol(prop: &str) -> f64 {
    match prop {
        "color_srgb" => 5e-5,
        "normal" | "vector" | "split_normals" => 2e-6,
        _ => 1e-6,
    }
}

/// Compares one collection's properties (item reads and `foreach_get`).
fn compare_coll(what: &str, c: &dyn RnaColl, dump: &J, errs: &mut Vec<String>) {
    let J::Object(props) = dump else {
        panic!("{what}: not an object")
    };
    for (p, d) in props {
        let at = format!("{what}.{p}");
        let items = d["items"].as_array().unwrap();
        if items.len() != c.len() {
            errs.push(format!("{at}: {} items, blender {}", c.len(), items.len()));
            continue;
        }
        for (i, bj) in items.iter().enumerate() {
            let Ok(Item::Struct(s)) = c.item(i) else {
                errs.push(format!("{at}[{i}]: not a struct item"));
                continue;
            };
            let def = s.srna().find_prop(p).unwrap();
            match s.get(def) {
                Ok(Field::Value(v)) => {
                    if let Err(e) = close(
                        &format!("{at}[{i}]"),
                        &flat_value(&v),
                        &flat_json(bj),
                        tol(p),
                    ) {
                        errs.push(e);
                    }
                }
                Ok(_) => errs.push(format!("{at}[{i}]: not a value")),
                Err(e) => errs.push(format!("{at}[{i}]: {e}")),
            }
        }
        if c.is_empty() {
            continue;
        }
        let def = c.item_type_of(0).find_prop(p).unwrap();
        match (&d["foreach"], c.foreach_get(def, 0..c.len())) {
            (J::Null, _) => {}
            (J::String(msg), Err(e)) => {
                if format!("RuntimeError: {}", e.message) != *msg {
                    errs.push(format!("{at} foreach: frap {e} != blender {msg}"));
                }
            }
            (J::String(msg), Ok(b)) => {
                errs.push(format!("{at} foreach: frap {b:?}, blender raised {msg}"))
            }
            (j, Ok(b)) => {
                if let Err(e) = close(
                    &format!("{at} foreach"),
                    &flat_bulk(&b),
                    &flat_json(j),
                    tol(p),
                ) {
                    errs.push(e);
                }
            }
            (j, Err(e)) => errs.push(format!("{at} foreach: frap {e}, blender {j}")),
        }
    }
}

fn names_of(c: &dyn RnaColl) -> Vec<String> {
    (0..c.len())
        .map(|i| c.key_of(i).unwrap().to_string())
        .collect()
}

fn get_coll<'a>(t: &'a dyn Rna, name: &str) -> frap_rna::CollRef<'a> {
    match t.get(t.srna().find_prop(name).unwrap()).unwrap() {
        Field::Coll(c) => c,
        _ => panic!("{name} is not a collection"),
    }
}

fn get_value(t: &dyn Rna, name: &str) -> Value {
    match t.get(t.srna().find_prop(name).unwrap()).unwrap() {
        Field::Value(v) => v,
        _ => panic!("{name} is not a value"),
    }
}

#[test]
fn reads_equal_blender() {
    let want: J = serde_json::from_str(FIXTURE).unwrap();
    let m = fixture_mesh();
    let mut errs = Vec::new();

    // Attribute list, refined types, filtered collections.
    let attrs = get_coll(&m, "attributes");
    let mut got = Vec::new();
    for i in 0..attrs.len() {
        let Item::Struct(a) = attrs.item(i).unwrap() else {
            panic!()
        };
        let mut row = Vec::new();
        for p in ["name", "data_type", "domain", "is_internal", "is_required"] {
            row.extend(flat_value(&get_value(&*a, p)));
        }
        row.push(S::T(a.srna().identifier.to_string()));
        got.push(row);
    }
    let want_attrs: Vec<Vec<S>> = want["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .map(flat_json)
        .collect();
    assert_eq!(got, want_attrs, "attributes");
    let color = get_coll(&m, "color_attributes");
    assert_eq!(
        flat_json(&want["color_attributes"]),
        names_of(&*color).into_iter().map(S::T).collect::<Vec<_>>()
    );
    for (coll, key) in [
        ("uv_layers", "uv_layers"),
        ("vertex_colors", "vertex_colors"),
    ] {
        let c = get_coll(&m, coll);
        let mut got = Vec::new();
        for i in 0..c.len() {
            let Item::Struct(l) = c.item(i).unwrap() else {
                panic!()
            };
            for p in ["name", "active", "active_render"] {
                got.extend(flat_value(&get_value(&*l, p)));
            }
        }
        assert_eq!(got, flat_json(&want[key]), "{coll}");
    }

    // Mesh-level properties.
    for (p, j) in want["mesh"].as_object().unwrap() {
        if let Err(e) = close(p, &flat_value(&get_value(&m, p)), &flat_json(j), 1e-6) {
            errs.push(e);
        }
    }

    // Element collections.
    for (coll, d) in want["elements"].as_object().unwrap() {
        compare_coll(coll, &*get_coll(&m, coll), d, &mut errs);
    }
    // Attribute data.
    for (name, d) in want["attribute_data"].as_object().unwrap() {
        let k = attrs.find(name).unwrap();
        let Item::Struct(a) = attrs.item(k).unwrap() else {
            panic!()
        };
        compare_coll(
            &format!("attributes[{name:?}].data"),
            &*get_coll(&*a, "data"),
            d,
            &mut errs,
        );
    }
    // UV layer data and vertex colours.
    let uvs = get_coll(&m, "uv_layers");
    let Item::Struct(uv) = uvs.item(uvs.find("UVMap").unwrap()).unwrap() else {
        panic!()
    };
    compare_coll(
        "uv_layers['UVMap'].data",
        &*get_coll(&*uv, "data"),
        &want["uv_data"],
        &mut errs,
    );
    compare_coll(
        "uv_layers['UVMap'].uv",
        &*get_coll(&*uv, "uv"),
        &want["uv_vectors"],
        &mut errs,
    );
    let vcs = get_coll(&m, "vertex_colors");
    for (name, d) in want["vertex_color_data"].as_object().unwrap() {
        let Item::Struct(l) = vcs.item(vcs.find(name).unwrap()).unwrap() else {
            panic!()
        };
        compare_coll(
            &format!("vertex_colors[{name:?}].data"),
            &*get_coll(&*l, "data"),
            d,
            &mut errs,
        );
    }
    assert!(
        errs.is_empty(),
        "{} mismatches:\n{}",
        errs.len(),
        errs.join("\n")
    );
}
