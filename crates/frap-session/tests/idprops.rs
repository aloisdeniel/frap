// SPDX-License-Identifier: GPL-3.0-or-later
//! ID properties (`ID["key"]`, M3+M4 stabilization round 2): Python's conversion rules and
//! errors (verified on Blender 5.2.4), the mapping methods, and the glTF import → read path.

mod support;

use serde_json::json;
use support::*;

const OB: &str = "bpy.data.objects['Cube']";

#[test]
fn assign_read_and_convert_like_python() {
    let mut s = session();
    let (exc, code, msg) = err(&mut s, &format!("{OB}['tag']"));
    assert_eq!(
        (exc.as_str(), code.as_str(), msg.as_str()),
        (
            "KeyError",
            "RNA_KEY_NOT_FOUND",
            "'bpy_struct[key]: key \"tag\" not found'"
        )
    );
    for (src, stored) in [
        ("'hero'", json!("hero")),
        ("3", json!(3)),
        ("0.25", json!(0.25)),
        ("True", json!(true)),
        ("[1, 2, 3]", json!([1, 2, 3])),
        ("(1, 2.5)", json!([1, 2.5])),
        ("[True, False]", json!([true, false])),
        ("['a', 'b']", json!(["a", "b"])),
        (
            "{'k': 1, 'g': {'x': 'y'}}",
            json!({"k": 1, "g": {"x": "y"}}),
        ),
        ("[]", json!([])),
    ] {
        let r = one(&mut s, &format!("{OB}['v'] = {src}"));
        assert_eq!(r["status"], "FINISHED", "{src}: {r}");
        assert_eq!(r["path"], format!("{OB}['v']"), "{src}");
        assert_eq!(r["value"], stored, "{src}: echo");
        assert_eq!(val(&mut s, &format!("{OB}['v']")), stored, "{src}: read");
    }
    one(&mut s, &format!("{OB}['n'] = 2"));
    one(&mut s, &format!("{OB}['n'] += 3"));
    assert_eq!(val(&mut s, &format!("{OB}['n']")), json!(5));
    one(&mut s, &format!("{OB}['g'] = {{'inner': {{'x': [4, 5]}}}}"));
    assert_eq!(
        val(&mut s, &format!("{OB}['g']['inner']['x'][1]")),
        json!(5)
    );
    let (exc, _, msg) = err(&mut s, &format!("{OB}['g']['nope']"));
    assert_eq!((exc.as_str(), msg.as_str()), ("KeyError", "'nope'"));
}

#[test]
fn python_errors() {
    let mut s = session();
    let cases = [
        (
            format!("{OB}['big'] = 2**40"),
            "ValueError",
            "Python int too large to convert to C int",
        ),
        (
            format!("{OB}['mix'] = [1, 'a']"),
            "TypeError",
            "only floats, ints, booleans and dicts are allowed in ID property arrays",
        ),
        (
            format!("{OB}[5] = 1"),
            "KeyError",
            "'invalid id-property key, expected a string, not a int'",
        ),
        (
            format!("{OB}['{}'] = 1", "x".repeat(64)),
            "KeyError",
            "'the length of IDProperty names is limited to 63 characters'",
        ),
        (
            format!("{OB}[1]"),
            "TypeError",
            "bpy_struct[key]: only strings are allowed as keys of ID properties",
        ),
        (
            "bpy.data.meshes['Cube'].vertices[0]['x']".to_owned(),
            "TypeError",
            "this type doesn't support IDProperties",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, &src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
    let r = one(&mut s, &format!("{OB}['none'] = None"));
    assert_eq!(r["error"]["code"], "RNA_NOT_IMPLEMENTED", "{r}");
    // `ob.name = …` is an attribute, not an ID property.
    one(&mut s, &format!("{OB}.name = 'Box'"));
    assert_eq!(val(&mut s, "bpy.data.objects['Box'].name"), json!("Box"));
}

#[test]
fn mapping_methods() {
    let mut s = session();
    one(&mut s, &format!("{OB}['a'] = 1"));
    one(&mut s, &format!("{OB}['b'] = 'x'"));
    assert_eq!(val(&mut s, &format!("{OB}.keys()")), json!(["a", "b"]));
    assert_eq!(val(&mut s, &format!("{OB}.values()")), json!([1, "x"]));
    assert_eq!(
        val(&mut s, &format!("{OB}.items()")),
        json!([["a", 1], ["b", "x"]])
    );
    assert_eq!(val(&mut s, &format!("{OB}.get('b')")), json!("x"));
    assert_eq!(val(&mut s, &format!("{OB}.get('c')")), json!(null));
    assert_eq!(val(&mut s, &format!("{OB}.get('c', 7)")), json!(7));
    // Other IDs and the context's.
    one(&mut s, "bpy.data.materials['Material']['m'] = 2");
    assert_eq!(val(&mut s, "bpy.context.object['a']"), json!(1));
    assert_eq!(
        val(&mut s, "bpy.data.materials['Material'].keys()"),
        json!(["m"])
    );
}
