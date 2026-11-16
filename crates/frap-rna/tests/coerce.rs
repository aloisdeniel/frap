// SPDX-License-Identifier: GPL-3.0-or-later
//! `coerce` table tests: every row of [RNA §1.2] and Blender's error wording, as printed by
//! Blender 5.2.4 for `setattr(...)` (`bpy_struct: item.attr = val:` prefix) and operator calls
//! (`Converting py args to operator properties:` prefix). Each expected string below was
//! captured from Blender (probe scripts kept with WP 1.2's notes); the prefix is added the way
//! callers do, with `RnaError::into_frap(cx.prefix(), …)`.

mod support;

use std::collections::BTreeSet;

use frap_rna::*;
use indexmap::IndexMap;
use smallvec::smallvec;
use smol_str::SmolStr;
use support::*;

fn rna_cx(owner: &'static str) -> CoerceCx {
    CoerceCx::RnaSet { owner }
}

fn op_cx() -> CoerceCx {
    CoerceCx::OpArgs {
        owner: "MESH_OT_primitive_cube_add",
    }
}

/// Full Blender text `Exception: message` of a failed coercion.
fn fail(def: &PropDef, input: Input, cx: &CoerceCx) -> String {
    match coerce(def, input.clone(), cx) {
        Ok(c) => panic!(
            "{} = {input:?} unexpectedly coerced to {:?}",
            def.identifier, c.value
        ),
        Err(e) => {
            let f = e.into_frap(cx.prefix(), RnaErrorKind::default_code);
            format!("{}: {}", f.exception.as_str(), f.message)
        }
    }
}

fn ok(def: &PropDef, input: Input, cx: &CoerceCx) -> Coerced {
    coerce(def, input.clone(), cx)
        .unwrap_or_else(|e| panic!("{} = {input:?} failed: {e:?}", def.identifier))
}

fn s(x: &str) -> Input {
    Input::Str(x.into())
}
fn t(v: Vec<Input>) -> Input {
    Input::Tuple(v)
}
fn ints(v: &[i64]) -> Input {
    Input::Tuple(v.iter().map(|i| Input::Int(*i)).collect())
}
fn fa(v: &Value) -> Vec<f32> {
    match v {
        Value::FloatArray { data, .. } => data.to_vec(),
        other => panic!("not a float array: {other:?}"),
    }
}

const P: &str = "bpy_struct: item.attr = val:";
const O: &str = "Converting py args to operator properties:";

#[test]
fn rna_1_2_whole_vector_any_sequence_ints_coerced() {
    // `o.location = (1,2,3)` and `[1, 2.5, 3]`, Vector/Euler/Color/set sources.
    let loc = prop(&OBJECT, "location");
    let cx = rna_cx("Object");
    let c = ok(loc, ints(&[1, 2, 3]), &cx);
    assert_eq!(
        c.value,
        Value::FloatArray {
            data: smallvec![1.0, 2.0, 3.0],
            shape: ArrayShape::Fixed(3),
            tag: MathTag::Vector
        }
    );
    assert!(!c.clamped);
    let c = ok(
        loc,
        Input::List(vec![Input::Int(1), Input::Float(2.5), Input::Int(3)]),
        &cx,
    );
    assert_eq!(fa(&c.value), [1.0, 2.5, 3.0]);
    for m in [
        MathValue::Vector(vec![1.0, 2.0, 3.0]),
        MathValue::Euler([1.0, 2.0, 3.0], EulerOrder::XYZ),
        MathValue::Color(vec![1.0, 2.0, 3.0]),
    ] {
        assert_eq!(fa(&ok(loc, Input::Math(m), &cx).value), [1.0, 2.0, 3.0]);
    }
    assert_eq!(
        fa(&ok(
            loc,
            Input::Set(vec![Input::Int(1), Input::Int(2), Input::Int(3)]),
            &cx
        )
        .value),
        [1.0, 2.0, 3.0]
    );
    // Booleans are numbers.
    assert_eq!(
        fa(&ok(
            loc,
            t(vec![Input::Bool(true), Input::Bool(false), Input::Int(3)]),
            &cx
        )
        .value),
        [1.0, 0.0, 3.0]
    );
}

#[test]
fn rna_1_2_wrong_length_value_error() {
    let loc = prop(&OBJECT, "location");
    let cx = rna_cx("Object");
    assert_eq!(
        fail(loc, ints(&[1, 2]), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 2")
    );
    assert_eq!(
        fail(loc, ints(&[1, 2, 3, 4]), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 4")
    );
    for m in [
        MathValue::Vector(vec![1.0, 2.0, 3.0, 4.0]),
        MathValue::Quaternion([1.0, 2.0, 3.0, 4.0]),
    ] {
        assert_eq!(
            fail(loc, Input::Math(m), &cx),
            format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 4")
        );
    }
    // Strings and dicts are sequences in Python.
    assert_eq!(
        fail(loc, s("ab"), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 2")
    );
    let mut d = IndexMap::new();
    d.insert("1".to_string(), Input::Int(2));
    assert_eq!(
        fail(loc, Input::Dict(d), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 1")
    );
    let diffuse = prop(&MATERIAL, "diffuse_color");
    assert_eq!(
        fail(diffuse, ints(&[1, 0, 0]), &rna_cx("Material")),
        format!("ValueError: {P} sequences of dimension 0 should contain 4 items, not 3")
    );
}

#[test]
fn rna_1_2_scalar_to_array_type_error_with_empty_body() {
    // `o.location = 5`: Blender prints the prefix alone, without the trailing space.
    let loc = prop(&OBJECT, "location");
    let cx = rna_cx("Object");
    assert_eq!(fail(loc, Input::Int(5), &cx), format!("TypeError: {P}"));
    assert_eq!(fail(loc, Input::None, &cx), format!("TypeError: {P}"));
    let e = coerce(loc, Input::Int(5), &cx).unwrap_err();
    assert_eq!(e.kind, RnaErrorKind::Type);
    assert!(e.hint.as_deref().unwrap().contains("(0, 0, 1)"));
    let lock = prop(&OBJECT, "lock_location");
    assert_eq!(
        fail(lock, Input::Bool(true), &cx),
        format!("TypeError: {P}")
    );
    let mw = prop(&OBJECT, "matrix_world");
    assert_eq!(fail(mw, Input::Int(5), &cx), format!("TypeError: {P}"));
    // Operator context: `primitive_cube_add(location=5)`.
    assert_eq!(
        fail(op_prop("location"), Input::Int(5), &op_cx()),
        format!("TypeError: {O}")
    );
    assert_eq!(
        fail(op_prop("location"), Input::None, &op_cx()),
        format!("TypeError: {O}")
    );
}

#[test]
fn array_item_types() {
    let cx = rna_cx("Object");
    let loc = prop(&OBJECT, "location");
    assert_eq!(
        fail(loc, t(vec![Input::Int(1), s("a"), Input::Int(3)]), &cx),
        format!("TypeError: {P} expected sequence items of type float, not str")
    );
    assert_eq!(
        fail(loc, s("abc"), &cx),
        format!("TypeError: {P} expected sequence items of type float, not str")
    );
    assert_eq!(
        fail(loc, t(vec![Input::Int(1), Input::None, Input::Int(3)]), &cx),
        format!("TypeError: {P} expected sequence items of type float, not NoneType")
    );
    assert_eq!(
        fail(
            loc,
            t(vec![ints(&[1, 2]), Input::Int(2), Input::Int(3)]),
            &cx
        ),
        format!("TypeError: {P} expected sequence items of type float, not tuple")
    );
    // Booleans: only True/False items.
    let lock = prop(&OBJECT, "lock_location");
    assert_eq!(
        fail(lock, t(vec![Input::Bool(true), Input::Bool(false)]), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 2")
    );
    assert_eq!(
        fail(lock, ints(&[1, 0, 1]), &cx),
        format!("TypeError: {P} expected sequence items of type boolean, not int")
    );
    assert_eq!(
        fail(
            lock,
            t(vec![Input::Bool(true), s("x"), Input::Bool(false)]),
            &cx
        ),
        format!("TypeError: {P} expected sequence items of type boolean, not str")
    );
    assert_eq!(
        fail(
            lock,
            t(vec![
                Input::Float(1.0),
                Input::Bool(false),
                Input::Bool(true)
            ]),
            &cx
        ),
        format!("TypeError: {P} expected sequence items of type boolean, not float")
    );
    assert_eq!(
        ok(
            lock,
            t(vec![
                Input::Bool(true),
                Input::Bool(false),
                Input::Bool(true)
            ]),
            &cx
        )
        .value,
        Value::BoolArray(smallvec![true, false, true])
    );
    // Ints: int and bool items, not floats.
    let pv = prop(&OBJECT, "parent_vertices");
    assert_eq!(
        fail(pv, ints(&[1, 2]), &cx),
        format!("ValueError: {P} sequences of dimension 0 should contain 3 items, not 2")
    );
    assert_eq!(
        fail(
            pv,
            t(vec![Input::Float(1.5), Input::Int(2), Input::Int(3)]),
            &cx
        ),
        format!("TypeError: {P} expected sequence items of type int, not float")
    );
    assert_eq!(
        ok(
            pv,
            t(vec![Input::Bool(true), Input::Int(2), Input::Int(3)]),
            &cx
        )
        .value,
        Value::IntArray(smallvec![1, 2, 3])
    );
}

#[test]
fn matrices() {
    let cx = rna_cx("Object");
    let mw = prop(&OBJECT, "matrix_world");
    let row = |a: [i64; 4]| ints(&a);
    // Three rows of four: total count message.
    assert_eq!(
        fail(
            mw,
            t(vec![
                row([1, 0, 0, 0]),
                row([0, 1, 0, 0]),
                row([0, 0, 1, 0])
            ]),
            &cx
        ),
        format!("ValueError: {P} Object.matrix_world, sequence must have 16 items total, not 12")
    );
    // A short row: dimension-1 message.
    assert_eq!(
        fail(
            mw,
            t(vec![
                ints(&[1, 0, 0]),
                row([0, 1, 0, 0]),
                row([0, 0, 1, 0]),
                row([0, 0, 0, 1])
            ]),
            &cx
        ),
        format!("ValueError: {P} sequences of dimension 1 should contain 4 items, not 3")
    );
    // Flat 16: items must be sequences.
    assert_eq!(
        fail(
            mw,
            ints(&[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
            &cx
        ),
        format!("TypeError: {P} expected a sequence of float, not int")
    );
    // A 3x3 mathutils.Matrix.
    let m3 = MathValue::Matrix(vec![
        vec![1.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![0.0, 0.0, 1.0],
    ]);
    assert_eq!(
        fail(mw, Input::Math(m3.clone()), &cx),
        format!(
            "ValueError: {P} Object.matrix_world, matrix assign dimension size mismatch, is 3x3, expected be 4x4"
        )
    );
    assert_eq!(
        fail(prop(&OBJECT, "location"), Input::Math(m3.clone()), &cx),
        format!("ValueError: {P} Object.location, matrix assign array with 1 dimensions")
    );
    assert_eq!(
        fail(prop(&OBJECT, "lock_location"), Input::Math(m3.clone()), &cx),
        format!("ValueError: {P} Object.lock_location, matrix assign to non float array")
    );
    assert_eq!(
        fail(prop(&OBJECT, "parent_vertices"), Input::Math(m3), &cx),
        format!("ValueError: {P} Object.parent_vertices, matrix assign to non float array")
    );
    // Nested sequences fill Blender's `m[col][row]`: the tuples are columns (verified: the
    // translation of `((1,0,0,5), …)` is not 5). Value data is row-major.
    let c = ok(
        mw,
        t(vec![
            row([1, 0, 0, 5]),
            row([0, 1, 0, 0]),
            row([0, 0, 1, 0]),
            row([7, 8, 9, 1]),
        ]),
        &cx,
    );
    let d = fa(&c.value);
    assert_eq!(&d[0..4], [1.0, 0.0, 0.0, 7.0]); // first row ends with translation x = 7
    assert_eq!(&d[12..16], [5.0, 0.0, 0.0, 1.0]);
    // A mathutils.Matrix is given by rows.
    let rows = MathValue::Matrix(vec![
        vec![1.0, 0.0, 0.0, 7.0],
        vec![0.0, 1.0, 0.0, 8.0],
        vec![0.0, 0.0, 1.0, 9.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ]);
    let c = ok(mw, Input::Math(rows), &cx);
    assert_eq!(&fa(&c.value)[0..4], [1.0, 0.0, 0.0, 7.0]);
    if let Value::FloatArray { shape, tag, .. } = c.value {
        assert_eq!((shape, tag), (ArrayShape::Matrix(4, 4), MathTag::Matrix));
    }
}

#[test]
fn rna_1_2_read_only_message() {
    // `o.type = 'EMPTY'`: the resolver refuses before coercion, with this helper.
    let e = traits::read_only(&OBJECT, "type");
    let f: FrapError = e.into_frap("", RnaErrorKind::default_code);
    assert_eq!(
        format!("{}: {}", f.exception.as_str(), f.message),
        "AttributeError: bpy_struct: attribute \"type\" from \"Object\" is read-only"
    );
    assert_eq!(f.code, codes::RNA_READONLY);
    assert!(prop(&OBJECT, "type").is_readonly());
}

#[test]
fn rna_1_2_enum_not_found_lists_items() {
    let rm = prop(&OBJECT, "rotation_mode");
    let cx = rna_cx("Object");
    let msg = fail(rm, s("FOO"), &cx);
    assert_eq!(
        msg,
        format!(
            "TypeError: {P} enum \"FOO\" not found in ('QUATERNION', 'XYZ', 'XZY', 'YXZ', 'YZX', 'ZXY', 'ZYX', 'AXIS_ANGLE')"
        )
    );
    assert_eq!(fail(rm, s("xyz"), &cx), msg.replace("\"FOO\"", "\"xyz\""));
    let e = coerce(rm, s("FOO"), &cx).unwrap_err();
    assert_eq!(e.kind, RnaErrorKind::Enum);
    assert_eq!(e.detail["valid"][1], "XYZ");
    assert_eq!(
        fail(rm, Input::Int(1), &cx),
        format!("TypeError: {P} expected a string enum, not int")
    );
    assert_eq!(
        fail(rm, Input::None, &cx),
        format!("TypeError: {P} expected a string enum, not NoneType")
    );
    assert_eq!(
        fail(rm, Input::Set(vec![s("XYZ")]), &cx),
        format!("TypeError: {P} expected a string enum, not set")
    );
    assert_eq!(ok(rm, s("ZYX"), &cx).value, Value::Enum("ZYX".into()));
    // Operator context.
    assert_eq!(
        fail(op_prop("align"), s("FOO"), &op_cx()),
        format!("TypeError: {O} enum \"FOO\" not found in ('WORLD', 'VIEW', 'CURSOR')")
    );
    assert_eq!(
        fail(op_prop("align"), Input::Int(1), &op_cx()),
        format!("TypeError: {O} expected a string enum, not int")
    );
    assert_eq!(
        fail(op_prop("align"), Input::List(vec![s("WORLD")]), &op_cx()),
        format!("TypeError: {O} expected a string enum, not list")
    );
}

#[test]
fn rna_1_2_enum_flag_takes_a_set_and_reads_back_as_a_set() {
    let delimit = prop(&DECIMATE, "delimit");
    let cx = rna_cx("DecimateModifier");
    let c = ok(delimit, Input::Set(vec![s("UV"), s("SEAM")]), &cx);
    let want: BTreeSet<SmolStr> = ["SEAM".into(), "UV".into()].into_iter().collect();
    assert_eq!(c.value, Value::EnumSet(want.clone()));
    assert_eq!(
        ok(delimit, Input::Set(vec![]), &cx).value,
        Value::EnumSet(BTreeSet::new())
    );
    // frap extension (§9.2): lists and tuples are accepted too (Blender: "expected a set").
    assert_eq!(
        ok(delimit, Input::List(vec![s("UV"), s("SEAM")]), &cx).value,
        Value::EnumSet(want)
    );
    assert_eq!(
        fail(delimit, s("UV"), &cx),
        format!("TypeError: {P}, DecimateModifier.delimit expected a set, not a str")
    );
    assert_eq!(
        fail(delimit, Input::None, &cx),
        format!("TypeError: {P}, DecimateModifier.delimit expected a set, not a NoneType")
    );
    assert_eq!(
        fail(delimit, Input::Set(vec![s("FOO")]), &cx),
        format!(
            "ValueError: {P}: 'FOO' not found in ('NORMAL', 'MATERIAL', 'SEAM', 'SHARP', 'UV')"
        )
    );
    assert_eq!(
        fail(delimit, Input::Set(vec![Input::Int(1)]), &cx),
        format!("TypeError: {P} expected a string, not int")
    );
    // Short form bare word: a one-item set.
    assert_eq!(
        ok(delimit, Input::Bare("UV".into()), &cx).value,
        Value::EnumSet(["UV".into()].into_iter().collect())
    );
}

#[test]
fn rna_1_2_hard_range_clamped_silently_frap_reports() {
    let cx = rna_cx("Object");
    let eds = prop(&OBJECT, "empty_display_size");
    let c = ok(eds, Input::Int(-5), &cx);
    assert_eq!(c.value, Value::Float(0.0001));
    assert!(c.clamped);
    let c = ok(eds, Input::Float(1e13), &cx);
    assert_eq!(c.value, Value::Float(1000.0));
    assert!(c.clamped);
    let c = ok(eds, Input::Float(2.5), &cx);
    assert!(!c.clamped);
    // 1e40 overflows f32 and is clamped to FLT_MAX (Blender reads 3.4028234663852886e+38).
    let c = ok(
        prop(&OBJECT, "location"),
        t(vec![Input::Float(1e40), Input::Int(0), Input::Int(0)]),
        &cx,
    );
    assert_eq!(fa(&c.value)[0], f32::MAX);
    assert!(c.clamped);
    // INT: clamped to the hard range, Python int range checked first.
    let fc = prop(&SCENE, "frame_current");
    let sc = rna_cx("Scene");
    let c = ok(fc, Input::Int(1_000_000_000), &sc);
    assert_eq!(c.value, Value::Int(1_048_574));
    assert!(c.clamped);
    assert_eq!(
        fail(fc, Input::Int(1_000_000_000_000), &sc),
        format!(
            "ValueError: {P} Scene.frame_current value not in 'int' range ((-2147483647 -1), 2147483647)"
        )
    );
    let cyl = CoerceCx::OpArgs {
        owner: "MESH_OT_primitive_cylinder_add",
    };
    assert_eq!(
        fail(&CYLINDER_VERTICES, Input::Int(1_000_000_000_000), &cyl),
        format!(
            "ValueError: {O} MESH_OT_primitive_cylinder_add.vertices value not in 'int' range ((-2147483647 -1), 2147483647)"
        )
    );
    let c = ok(&CYLINDER_VERTICES, Input::Int(1), &cyl);
    assert_eq!((c.value, c.clamped), (Value::Int(3), true));
    let c = ok(op_prop("size"), Input::Int(-5), &op_cx());
    assert_eq!((c.value, c.clamped), (Value::Float(0.0), true));
}

#[test]
fn clamp_report_wording() {
    // §9.2 / deviation D-03: one wording for assignments and operator arguments.
    let size = float(
        "size",
        ArrayShape::Scalar,
        (0.0, 999_999_995_904.0),
        Subtype::Distance,
        NONE,
    );
    assert_eq!(
        clamp_report(&size, &Input::Int(-5)),
        "frap: size=-5 clamped to hard min 0.0"
    );
    assert_eq!(
        clamp_report(&size, &Input::Float(1e13)),
        "frap: size=10000000000000.0 clamped to hard max 999999995904.0"
    );
    let loc = float(
        "location",
        ArrayShape::Fixed(3),
        (-999_999_995_904.0, 999_999_995_904.0),
        Subtype::Translation,
        NONE,
    );
    assert_eq!(
        clamp_report(&loc, &ints_f(&[1e16, 0.0, 0.0])),
        "frap: location=(1e+16, 0.0, 0.0) clamped to hard range [-999999995904.0, 999999995904.0]"
    );
    assert_eq!(
        clamp_report(
            &loc,
            &t(vec![Input::Float(1e16), Input::Int(0), Input::Int(0)])
        ),
        "frap: location=(1e+16, 0, 0) clamped to hard range [-999999995904.0, 999999995904.0]"
    );
    let fc = int(
        "frame_current",
        ArrayShape::Scalar,
        (-1_048_574, 1_048_574),
        NONE,
    );
    assert_eq!(
        clamp_report(&fc, &Input::Int(2_000_000)),
        "frap: frame_current=2000000 clamped to hard max 1048574"
    );
    assert_eq!(
        clamp_report(&fc, &Input::Int(-2_000_000)),
        "frap: frame_current=-2000000 clamped to hard min -1048574"
    );
}

fn ints_f(v: &[f64]) -> Input {
    Input::Tuple(v.iter().map(|f| Input::Float(*f)).collect())
}

#[test]
fn scalar_type_errors() {
    let cx = rna_cx("Object");
    let eds = prop(&OBJECT, "empty_display_size");
    for (input, name) in [
        (s("x"), "str"),
        (Input::None, "NoneType"),
        (ints(&[1]), "tuple"),
    ] {
        assert_eq!(
            fail(eds, input, &cx),
            format!("TypeError: {P} Object.empty_display_size expected a float type, not {name}")
        );
    }
    assert_eq!(ok(eds, Input::Bool(true), &cx).value, Value::Float(1.0));
    let fc = prop(&SCENE, "frame_current");
    let sc = rna_cx("Scene");
    assert_eq!(
        fail(fc, Input::Float(1.5), &sc),
        format!("TypeError: {P} Scene.frame_current expected an int type, not float")
    );
    assert_eq!(
        fail(fc, s("x"), &sc),
        format!("TypeError: {P} Scene.frame_current expected an int type, not str")
    );
    assert_eq!(ok(fc, Input::Bool(true), &sc).value, Value::Int(1));
    let hr = prop(&OBJECT, "hide_render");
    for (input, name) in [
        (Input::Int(2), "int"),
        (Input::Float(0.0), "float"),
        (s("x"), "str"),
        (Input::None, "NoneType"),
    ] {
        assert_eq!(
            fail(hr, input, &cx),
            format!("TypeError: {P} Object.hide_render expected True/False or 0/1, not {name}")
        );
    }
    assert_eq!(ok(hr, Input::Int(1), &cx).value, Value::Bool(true));
    assert_eq!(ok(hr, Input::Bool(false), &cx).value, Value::Bool(false));
    let name = prop(&OBJECT, "name");
    assert_eq!(
        fail(name, Input::Int(5), &cx),
        format!("TypeError: {P} Object.name expected a string type, not int")
    );
    assert_eq!(
        fail(name, Input::None, &cx),
        format!("TypeError: {P} Object.name doesn't support None from string types")
    );
    assert_eq!(
        fail(prop(&OBJECT, "parent_bone"), Input::None, &cx),
        format!("TypeError: {P} Object.parent_bone doesn't support None from string types")
    );
    // A string without NEVER_NULL takes None as "".
    let free = string("free", 0, NONE);
    assert_eq!(ok(&free, Input::None, &cx).value, Value::Str("".into()));
    // Operator context wording (Appendix A rows).
    assert_eq!(
        fail(op_prop("size"), s("x"), &op_cx()),
        format!("TypeError: {O} MESH_OT_primitive_cube_add.size expected a float type, not str")
    );
    assert_eq!(
        fail(op_prop("size"), Input::None, &op_cx()),
        format!(
            "TypeError: {O} MESH_OT_primitive_cube_add.size expected a float type, not NoneType"
        )
    );
    assert_eq!(
        fail(op_prop("calc_uvs"), Input::Int(2), &op_cx()),
        format!(
            "TypeError: {O} MESH_OT_primitive_cube_add.calc_uvs expected True/False or 0/1, not int"
        )
    );
    assert_eq!(
        fail(op_prop("location"), ints(&[1, 2]), &op_cx()),
        format!("ValueError: {O} sequences of dimension 0 should contain 3 items, not 2")
    );
    assert_eq!(
        fail(
            op_prop("location"),
            t(vec![Input::Int(1), s("a"), Input::Int(3)]),
            &op_cx()
        ),
        format!("TypeError: {O} expected sequence items of type float, not str")
    );
    let cyl = CoerceCx::OpArgs {
        owner: "MESH_OT_primitive_cylinder_add",
    };
    assert_eq!(
        fail(&CYLINDER_VERTICES, Input::Float(3.5), &cyl),
        format!(
            "TypeError: {O} MESH_OT_primitive_cylinder_add.vertices expected an int type, not float"
        )
    );
    let save = CoerceCx::OpArgs {
        owner: "WM_OT_save_as_mainfile",
    };
    assert_eq!(
        fail(&SAVE_FILEPATH, Input::Int(5), &save),
        format!("TypeError: {O} WM_OT_save_as_mainfile.filepath expected a string type, not int")
    );
}

#[test]
fn strings_truncate_on_utf8_boundary() {
    // ID names are 255 bytes: 200 × "é" becomes 127 characters (254 bytes) in Blender.
    let name = prop(&OBJECT, "name");
    let c = ok(name, s(&"é".repeat(200)), &rna_cx("Object"));
    let Value::Str(v) = c.value else { panic!() };
    assert_eq!((v.chars().count(), v.len()), (127, 254));
    // Sub-data names are 63 bytes: 40 × "é" → 62 bytes.
    let mod_name = prop(&DECIMATE, "name");
    let Value::Str(v) = ok(mod_name, s(&"é".repeat(40)), &rna_cx("DecimateModifier")).value else {
        panic!()
    };
    assert_eq!(v.len(), 62);
    // Unlimited strings are kept.
    let free = string("free", 0, NONE);
    let Value::Str(v) = ok(&free, s(&"x".repeat(5000)), &rna_cx("Object")).value else {
        panic!()
    };
    assert_eq!(v.len(), 5000);
}

#[test]
fn pointers_checked_against_fixed_type() {
    let cx = rna_cx("Object");
    let parent = prop(&OBJECT, "parent");
    let mesh_ptr = RnaPtr::id(
        IdAny {
            code: IdCode::ME,
            key: IdKey::next(),
        },
        &MESH,
    );
    assert_eq!(
        fail(parent, Input::Ptr(mesh_ptr.clone()), &cx),
        format!("TypeError: {P} Object.parent expected a Object type, not Mesh")
    );
    assert_eq!(
        fail(parent, Input::Int(5), &cx),
        format!("TypeError: {P} Object.parent expected a Object type, not int")
    );
    assert_eq!(
        fail(parent, s("Camera"), &cx),
        format!("TypeError: {P} Object.parent expected a Object type, not str")
    );
    assert_eq!(ok(parent, Input::None, &cx).value, Value::Id(None));
    let ob = IdAny {
        code: IdCode::OB,
        key: IdKey::next(),
    };
    assert_eq!(
        ok(parent, Input::Ptr(RnaPtr::id(ob, &OBJECT)), &cx).value,
        Value::Id(Some(ob))
    );
    // `Object.data` is an `ID` pointer: a Mesh is an ID; an Object too, but Blender rejects it
    // through the refined type (`expected a Mesh type`) in the setter, not in coercion.
    let data = prop(&OBJECT, "data");
    assert!(matches!(
        ok(data, Input::Ptr(mesh_ptr), &cx).value,
        Value::Id(Some(IdAny {
            code: IdCode::ME,
            ..
        }))
    ));
    // NEVER_NULL pointers refuse None (Blender: `ViewLayer.active_layer_collection`).
    let nn = pointer("active_layer_collection", "LayerCollection", false, NN);
    assert_eq!(
        fail(&nn, Input::None, &rna_cx("ViewLayer")),
        format!(
            "TypeError: {P} ViewLayer.active_layer_collection does not support a 'None' assignment LayerCollection type"
        )
    );
    // A pointer below an ID stays a pointer.
    let sock = RnaPtr::id(ob, &OBJECT).child(Seg::Prop("x"), &SOCKET);
    let p = pointer("socket", "NodeSocketColor", false, NONE);
    assert!(matches!(ok(&p, Input::Ptr(sock), &cx).value, Value::Ptr(_)));
    assert_eq!(ok(&p, Input::None, &cx).value, Value::None);
}

#[test]
fn bare_words_only_for_strings_and_enums() {
    let cx = rna_cx("Object");
    assert_eq!(
        ok(
            prop(&OBJECT, "rotation_mode"),
            Input::Bare("XYZ".into()),
            &cx
        )
        .value,
        Value::Enum("XYZ".into())
    );
    assert_eq!(
        ok(prop(&OBJECT, "name"), Input::Bare("Cube".into()), &cx).value,
        Value::Str("Cube".into())
    );
    let e = coerce(
        prop(&OBJECT, "empty_display_size"),
        Input::Bare("big".into()),
        &cx,
    )
    .unwrap_err();
    assert_eq!(e.exception, PyExc::TypeError);
    assert_eq!(
        e.message,
        "Object.empty_display_size expected a float type, not str"
    );
    assert_eq!(e.hint.as_deref(), Some("quote it or pass a number"));
    let e = coerce(prop(&OBJECT, "hide_render"), Input::Bare("yes".into()), &cx).unwrap_err();
    assert_eq!(e.hint.as_deref(), Some("quote it or pass a number"));
}

#[test]
fn operator_collection_props() {
    let files = collection("files", "OperatorFileListElement", NONE);
    let cx = CoerceCx::OpArgs {
        owner: "IMPORT_SCENE_OT_gltf",
    };
    let mut d = IndexMap::new();
    d.insert("name".to_string(), s("a.glb"));
    let c = ok(&files, Input::List(vec![Input::Dict(d)]), &cx);
    let Value::Structs(v) = c.value else { panic!() };
    assert_eq!(v[0]["name"], Value::Str("a.glb".into()));
    assert_eq!(
        fail(&files, s("a.glb"), &cx),
        format!(
            "TypeError: {O} IMPORT_SCENE_OT_gltf.files expected a sequence for an RNA collection, not str"
        )
    );
    assert_eq!(
        fail(&files, Input::List(vec![s("a.glb")]), &cx),
        format!(
            "TypeError: {O} IMPORT_SCENE_OT_gltf.files expected a each sequence member to be a dict for an RNA collection, not str"
        )
    );
}

#[test]
fn element_assignment_messages() {
    let loc = prop(&OBJECT, "location");
    let e = value::coerce_element(loc, s("x")).unwrap_err();
    assert_eq!(
        (e.exception, e.message.as_str()),
        (
            PyExc::TypeError,
            "vector[index] = x: assigned value not a number"
        )
    );
    let eul = prop(&OBJECT, "rotation_euler");
    assert_eq!(
        value::coerce_element(eul, s("x")).unwrap_err().message,
        "euler[attribute] = x: assigned value not a number"
    );
    let q = float(
        "rotation_quaternion",
        ArrayShape::Fixed(4),
        (-FLT_MAX, FLT_MAX),
        Subtype::Quaternion,
        NONE,
    );
    assert_eq!(
        value::coerce_element(&q, s("x")).unwrap_err().message,
        "quaternion[index] = x: assigned value not a number"
    );
    let col3 = float(
        "color",
        ArrayShape::Fixed(3),
        (0.0, FLT_MAX),
        Subtype::Color,
        NONE,
    );
    assert_eq!(
        value::coerce_element(&col3, s("x")).unwrap_err().message,
        "color[item] = x: assigned value not a number"
    );
    let diffuse = prop(&MATERIAL, "diffuse_color");
    assert_eq!(
        value::coerce_element(diffuse, s("x")).unwrap_err().message,
        "expected a float type"
    );
    assert_eq!(
        value::coerce_element(diffuse, Input::None)
            .unwrap_err()
            .message,
        "expected a float type"
    );
    assert_eq!(
        value::coerce_element(diffuse, Input::Int(2)).unwrap().value,
        Value::Float(2.0)
    );
    let lock = prop(&OBJECT, "lock_location");
    assert_eq!(
        value::coerce_element(lock, Input::Int(2))
            .unwrap_err()
            .message,
        "Python number not a bool (0/1)"
    );
    assert_eq!(
        value::coerce_element(lock, s("x")).unwrap_err().message,
        "'str' object cannot be interpreted as an integer"
    );
    assert_eq!(
        value::coerce_element(lock, Input::Float(1.0))
            .unwrap_err()
            .message,
        "'float' object cannot be interpreted as an integer"
    );
    assert_eq!(
        value::coerce_element(lock, Input::Int(1)).unwrap().value,
        Value::Bool(true)
    );
    let pv = prop(&OBJECT, "parent_vertices");
    assert_eq!(
        value::coerce_element(pv, Input::Float(1.5))
            .unwrap_err()
            .message,
        "expected an int type"
    );
    assert_eq!(
        value::coerce_element(pv, Input::Int(7)).unwrap().value,
        Value::Int(7)
    );
    let c = value::coerce_element(loc, Input::Float(1e40)).unwrap();
    assert_eq!((c.value, c.clamped), (Value::Float(f32::MAX), true));
}

#[test]
fn into_frap_prefix_rules() {
    // Empty body and bodies starting with ',' or ':' drop the prefix's trailing space.
    for (body, want) in [
        ("", "bpy_struct: item.attr = val:"),
        (
            ", X.y expected a set, not a list",
            "bpy_struct: item.attr = val:, X.y expected a set, not a list",
        ),
        (
            ": 'FOO' not found in ('A')",
            "bpy_struct: item.attr = val:: 'FOO' not found in ('A')",
        ),
        ("sequences", "bpy_struct: item.attr = val: sequences"),
    ] {
        let e = RnaError::new(RnaErrorKind::Type, PyExc::TypeError, body);
        assert_eq!(
            e.into_frap("bpy_struct: item.attr = val: ", RnaErrorKind::default_code)
                .message,
            want
        );
    }
    // Appendix A: `primitive_cube_add(sizee=2)`.
    let e = RnaError::new(
        RnaErrorKind::FuncArgs,
        PyExc::TypeError,
        ": keyword \"sizee\" unrecognized",
    );
    assert_eq!(
        e.into_frap(op_cx().prefix(), RnaErrorKind::default_code)
            .message,
        "Converting py args to operator properties:: keyword \"sizee\" unrecognized"
    );
}
