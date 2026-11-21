// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 1.14 acceptance: path forms of [RNA §1.1], get/set/call/print/op_meta responses, value
//! encoding (§13.3: `$coll` with and without names, `$array`, `foreach_get`/`foreach_set`,
//! slices, `--max-array`), `changes`/`context`, reads without a transaction, compat errors and
//! the serve framing (`exec_request`, `DoneRecord`). Expected strings are Blender 5.2.4's
//! (verified with `blender -b --factory-startup --python`).

mod support;

use frap_rna::{FrapError, Input};
use frap_session::{DoneRecord, OnError, Request, Status};
use indexmap::IndexMap;
use serde_json::{Value as J, json};
use support::*;

// ------------------------------------------------------------------ path forms

#[test]
fn roots_aliases_and_quotes_resolve_to_the_same_id() {
    let mut s = session();
    for src in [
        "bpy.data.objects['Cube']",
        "bpy.data.objects[\"Cube\"]",
        "D.objects['Cube']",
        "data.objects['Cube']",
        "bpy.data.objects[1]",
        "bpy.data.objects[-2]",
        "bpy.data.objects['Cube', None]",
        "bpy.context.object",
        "C.active_object",
        "context.object",
    ] {
        let r = one(&mut s, src);
        assert_eq!(
            r["value"],
            json!({"$ref": "bpy.data.objects['Cube']"}),
            "{src}: {r}"
        );
        assert_eq!(r["kind"], "get");
    }
    assert_eq!(
        one(&mut s, "bpy.data.objects['Cube']")["path"],
        "bpy.data.objects['Cube']"
    );
    assert_eq!(
        val(&mut s, "bpy.data"),
        json!({"$ptr": "bpy.data", "type": "BlendData"})
    );
}

#[test]
fn collection_index_and_key_errors_use_blender_wording() {
    let mut s = session();
    let cases = [
        (
            "bpy.data.objects[9]",
            "IndexError",
            "RNA_INDEX_RANGE",
            "bpy_prop_collection[index]: index 9 out of range, size 3",
        ),
        (
            "bpy.data.objects[-9]",
            "IndexError",
            "RNA_INDEX_RANGE",
            "bpy_prop_collection[-9]: out of range.",
        ),
        (
            "bpy.data.objects['Nope']",
            "KeyError",
            "RNA_KEY_NOT_FOUND",
            "'bpy_prop_collection[key]: key \"Nope\" not found'",
        ),
        (
            "bpy.data.objects['Cube', 'x.blend']",
            "KeyError",
            "RNA_KEY_NOT_FOUND",
            "\"bpy_prop_collection[id, lib]: lib filepath 'x.blend' does not reference a valid library\"",
        ),
        (
            "bpy.data.objects.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "bpy_prop_collection: attribute \"foo\" not found",
        ),
        (
            "bpy.data.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'BlendData' object has no attribute 'foo'",
        ),
        (
            "bpy.context.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Context' object has no attribute 'foo'",
        ),
        (
            "bpy.data.objects['Cube'].foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Object' object has no attribute 'foo'",
        ),
        (
            "bpy.data.objects['Cube']['x']",
            "KeyError",
            "RNA_KEY_NOT_FOUND",
            "'bpy_struct[key]: key \"x\" not found'",
        ),
        (
            "bpy.data.objects['Cube'].name.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'str' object has no attribute 'foo'",
        ),
        (
            "bpy.data.meshes['Cube'].vertices['a']",
            "TypeError",
            "RNA_TYPE",
            "bpy_prop_collection[key]: Mesh.vertices does not support string lookups",
        ),
        (
            "bpy.data.meshes['Cube'].vertices[100]",
            "IndexError",
            "RNA_INDEX_RANGE",
            "bpy_prop_collection[index]: index 100 out of range, size 8",
        ),
        (
            "bpy.data.meshes['Cube'].vertices[0].foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'MeshVertex' object has no attribute 'foo'",
        ),
        (
            "bpy.context.selected_objects[5]",
            "IndexError",
            "RNA_INDEX_RANGE",
            "list index out of range",
        ),
        (
            "bpy.context.selected_objects['Cube']",
            "TypeError",
            "RNA_TYPE",
            "list indices must be integers or slices, not str",
        ),
        (
            "bpy.context.selected_objects.foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'list' object has no attribute 'foo'",
        ),
        (
            "bpy.data.objects[0:1].foo",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'list' object has no attribute 'foo'",
        ),
    ];
    for (src, exc, code, msg) in cases {
        assert_eq!(
            err(&mut s, src),
            (exc.into(), code.into(), msg.into()),
            "{src}"
        );
    }
    // `did_you_mean` on misspelled keys and attributes.
    let r = one(&mut s, "bpy.data.objects['Cub']");
    assert_eq!(r["error"]["detail"]["did_you_mean"], json!(["Cube"]));
    let r = one(&mut s, "bpy.data.objects['Cube'].locaton");
    assert_eq!(r["error"]["detail"]["did_you_mean"][0], "location");
}

#[test]
fn array_elements_components_and_slices() {
    let mut s = session();
    val(&mut s, "bpy.data.objects['Cube'].location = (1, 2, 3)");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[1]"),
        json!(2)
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[-1]"),
        json!(3)
    );
    assert_eq!(val(&mut s, "bpy.data.objects['Cube'].location.x"), json!(1));
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[0:2]"),
        json!([1, 2])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[-2:]"),
        json!([2, 3])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[1:9]"),
        json!([2, 3])
    );
    let r = one(&mut s, "bpy.data.objects['Cube'].location.z");
    assert_eq!(r["path"], "bpy.data.objects['Cube'].location.z");
    assert_eq!(r["rna"], json!({"type": "FLOAT", "subtype": "TRANSLATION"}));
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location.length"),
        json!(14f64.sqrt())
    );
    let cases = [
        (
            "bpy.data.objects['Cube'].location[5]",
            "IndexError",
            "vector[index]: out of range",
        ),
        (
            "bpy.data.objects['Cube'].location[-4]",
            "IndexError",
            "vector[index]: out of range",
        ),
        (
            "bpy.data.objects['Cube'].location.w",
            "AttributeError",
            "Vector.w: unavailable on 3d vector",
        ),
        (
            "bpy.data.objects['Cube'].location.foo",
            "AttributeError",
            "'Vector' object has no attribute 'foo'",
        ),
        (
            "bpy.data.objects['Cube'].location['x']",
            "TypeError",
            "vector indices must be integers, not str",
        ),
        (
            "bpy.data.objects['Cube'].location.x.y",
            "AttributeError",
            "'float' object has no attribute 'y'",
        ),
        (
            "bpy.context.scene.cursor.rotation_quaternion[4]",
            "IndexError",
            "quaternion[attribute]: array index out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_euler[3]",
            "IndexError",
            "euler[attribute]: array index out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle[7]",
            "IndexError",
            "bpy_prop_array[index]: index 7 out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle[-9]",
            "IndexError",
            "bpy_prop_array[index]: index -5 out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle.x",
            "AttributeError",
            "'bpy_prop_array' object has no attribute 'x'",
        ),
        (
            "bpy.context.scene.cursor.matrix[4]",
            "IndexError",
            "matrix[attribute]: array index out of range",
        ),
        (
            "bpy.context.scene.cursor.matrix[0][4]",
            "IndexError",
            "vector[index]: out of range",
        ),
        (
            "bpy.data.objects['Cube'].location()",
            "TypeError",
            "'Vector' object is not callable",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
    // Quaternion components are w-first; matrices are rows.
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.rotation_quaternion.w"),
        json!(1)
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[2][2]"),
        json!(1)
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[0]"),
        json!([1, 0, 0, 0])
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.rotation_euler.order"),
        json!("XYZ")
    );
}

#[test]
fn matrix_slices_are_rows_and_strings_and_tuples_index() {
    let mut s = session();
    // mathutils slices a Matrix by rows (Blender 5.2.4, verified; a non-identity matrix is
    // covered by the unit tests of `resolve::element`, the cursor is not writable here).
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[0:2]"),
        json!([[1, 0, 0, 0], [0, 1, 0, 0]])
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[-1:]"),
        json!([[0, 0, 0, 1]])
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[5:9]"),
        json!([])
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[1:3][0][1]"),
        json!(1)
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[3].w"),
        json!(1)
    );
    assert_eq!(
        val(&mut s, "bpy.context.scene.cursor.matrix[2].w"),
        json!(0)
    );
    // A call result keeps its mathutils type.
    assert_eq!(
        val(
            &mut s,
            "bpy.context.scene.cursor.path_resolve('matrix')[2:][0][2]"
        ),
        json!(1)
    );
    assert_eq!(
        err(&mut s, "bpy.context.scene.cursor.matrix[0:2].x").2,
        "'tuple' object has no attribute 'x'"
    );
    // Python str and tuple indexing.
    let n = "bpy.data.objects['Cube'].name";
    assert_eq!(val(&mut s, &format!("{n}[0]")), json!("C"));
    assert_eq!(val(&mut s, &format!("{n}[-1]")), json!("e"));
    assert_eq!(val(&mut s, &format!("{n}[1:3]")), json!("ub"));
    let l = "bpy.data.objects['Cube'].location";
    val(&mut s, &format!("{l} = (4, 5, 6)"));
    assert_eq!(val(&mut s, &format!("{l}[0:2][1]")), json!(5));
    let cases = [
        (format!("{n}[9]"), "IndexError", "string index out of range"),
        (
            format!("{n}['a']"),
            "TypeError",
            "string indices must be integers, not 'str'",
        ),
        (
            format!("{l}[0:2][5]"),
            "IndexError",
            "tuple index out of range",
        ),
        (
            format!("{l}[0:2]['a']"),
            "TypeError",
            "tuple indices must be integers or slices, not str",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, &src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
}

#[test]
fn context_members_re_root_at_their_data() {
    let mut s = session();
    assert_eq!(
        val(&mut s, "bpy.context.scene"),
        json!({"$ref": "bpy.data.scenes['Scene']"})
    );
    assert_eq!(
        val(&mut s, "bpy.context.selected_objects"),
        json!([{"$ref": "bpy.data.objects['Cube']"}])
    );
    assert_eq!(
        val(&mut s, "C.selected_objects[0]"),
        json!({"$ref": "bpy.data.objects['Cube']"})
    );
    assert_eq!(val(&mut s, "bpy.context.mode"), json!("OBJECT"));
    assert_eq!(val(&mut s, "bpy.context.edit_object"), J::Null);
    assert_eq!(
        val(&mut s, "bpy.context.view_layer"),
        json!({"$ptr": "bpy.data.scenes['Scene'].view_layers[\"ViewLayer\"]", "type": "ViewLayer"})
    );
    assert_eq!(
        val(&mut s, "bpy.context.tool_settings"),
        json!({"$ptr": "bpy.data.scenes['Scene'].tool_settings", "type": "ToolSettings"})
    );
    assert_eq!(
        val(&mut s, "bpy.context.collection"),
        json!({"$ref": "bpy.data.collections['Collection']"})
    );
    // Buttons-context members do not exist in the VIEW_3D context (Blender 5.2.4, verified).
    for m in [
        "mesh",
        "camera",
        "light",
        "world",
        "material",
        "material_slot",
    ] {
        assert_eq!(
            err(&mut s, &format!("bpy.context.{m}")),
            (
                "AttributeError".to_owned(),
                "RNA_NO_ATTRIBUTE".to_owned(),
                format!("'Context' object has no attribute '{m}'")
            )
        );
    }
    assert_eq!(
        err(&mut s, "bpy.context.mesh = None").2,
        "'Context' object has no attribute 'mesh'"
    );
    assert_eq!(
        val(&mut s, "bpy.context.blend_data"),
        json!({"$ptr": "bpy.data", "type": "BlendData"})
    );
    // Nested struct through a pointer property: canonical repr, then its own members.
    let r = one(&mut s, "bpy.context.scene.cursor");
    assert_eq!(
        r["value"],
        json!({"$ptr": "bpy.data.scenes['Scene'].cursor", "type": "View3DCursor"})
    );
    let r = one(&mut s, "bpy.context.scene.cursor.location");
    assert_eq!(r["path"], "bpy.data.scenes['Scene'].cursor.location");
    assert_eq!(r["value"], json!([0, 0, 0]));
    let r = one(&mut s, "bpy.context.scene.cursor.rotation_euler");
    assert_eq!(
        r["rna"],
        json!({"type": "FLOAT", "subtype": "EULER", "array": [3], "math": "Euler", "order": "XYZ"})
    );
    // Assigning a context member is refused with Blender's two wordings.
    assert_eq!(
        err(&mut s, "bpy.context.object = None"),
        (
            "AttributeError".into(),
            "CONTEXT_READONLY".into(),
            "bpy_struct: Context property \"object\" is read-only".into()
        )
    );
    assert_eq!(
        err(&mut s, "bpy.context.scene = bpy.data.scenes[0]"),
        (
            "AttributeError".into(),
            "CONTEXT_READONLY".into(),
            "bpy_struct: attribute \"scene\" from \"Context\" is read-only".into()
        )
    );
    assert!(one(&mut s, "bpy.context.object = None")["error"]["hint"].is_string());
}

#[test]
fn removed_4x_members_answer_with_their_5x_replacement() {
    let mut s = session();
    let r = one(&mut s, "bpy.data.meshes['Cube'].use_auto_smooth");
    assert_eq!(r["error"]["exception"], "AttributeError");
    assert_eq!(r["error"]["code"], "RNA_REMOVED_IN_5X");
    assert_eq!(
        r["error"]["message"],
        "'Mesh' object has no attribute 'use_auto_smooth'"
    );
    assert!(
        r["error"]["hint"]
            .as_str()
            .unwrap()
            .contains("shade_smooth_by_angle")
    );
    assert_eq!(r["error"]["detail"]["removed_in"], "4.1");
    let r = one(&mut s, "bpy.data.objects['Cube'].face_maps");
    assert_eq!(r["error"]["code"], "RNA_REMOVED_IN_5X");
    let r = one(&mut s, "bpy.data.meshes['Cube'].calc_normals()");
    assert_eq!(r["error"]["code"], "RNA_REMOVED_IN_5X");
    assert_eq!(r["kind"], "call");
}

// ------------------------------------------------------------------ value encoding

#[test]
fn named_and_nameless_collections() {
    let mut s = session_with(|o| o.max_items = 2);
    let r = one(&mut s, "bpy.data.objects");
    assert_eq!(
        r["value"],
        json!({"$coll": "bpy.data.objects", "item_type": "Object", "len": 3,
               "items": ["Camera", "Cube"], "truncated": true})
    );
    assert_eq!(r["rna"]["type"], "COLLECTION");
    let r = one(&mut s, "bpy.data.meshes['Cube'].vertices");
    assert_eq!(
        r["value"],
        json!({"$coll": "bpy.data.meshes['Cube'].vertices", "item_type": "MeshVertex", "len": 8})
    );
    assert_eq!(r["path"], "bpy.data.meshes['Cube'].vertices");
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0]"),
        json!({"$ptr": "bpy.data.meshes['Cube'].vertices[0]", "type": "MeshVertex"})
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[-1].index"),
        json!(7)
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co"),
        json!([1, 1, 1])
    );
    // Slices are Python lists of items.
    assert_eq!(
        val(&mut s, "bpy.data.objects[1:]"),
        json!([{"$ref": "bpy.data.objects['Cube']"}, {"$ref": "bpy.data.objects['Light']"}])
    );
    assert_eq!(val(&mut s, "bpy.data.objects[5:1]"), json!([]));
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[2:4][1]"),
        json!({"$ptr": "bpy.data.meshes['Cube'].vertices[3]", "type": "MeshVertex"})
    );
}

#[test]
fn bulk_reads_slices_and_truncation() {
    let mut s = session();
    let r = one(&mut s, "bpy.data.meshes['Cube'].vertices.foreach_get('co')");
    assert_eq!(r["kind"], "call");
    assert_eq!(r["func"], "bpy.data.meshes['Cube'].vertices.foreach_get");
    let v = &r["value"];
    assert_eq!(v["shape"], json!([8, 3]));
    assert_eq!(v["dtype"], "float32");
    assert_eq!(v["len"], 8);
    assert_eq!(v["range"], json!([0, 8]));
    assert_eq!(v["truncated"], false);
    assert_eq!(v["$array"].as_array().unwrap().len(), 24);
    assert_eq!(
        v["$array"].as_array().unwrap()[0..3],
        [json!(1), json!(1), json!(1)]
    );
    // Paging through a slice.
    let v = val(
        &mut s,
        "bpy.data.meshes['Cube'].vertices[6:8].foreach_get('co')",
    );
    assert_eq!(v["range"], json!([6, 8]));
    assert_eq!(v["$array"].as_array().unwrap().len(), 6);
    let v = val(
        &mut s,
        "bpy.data.meshes['Cube'].vertices.foreach_get('index')",
    );
    assert_eq!(v["shape"], json!([8]));
    assert_eq!(v["dtype"], "int32");
    assert_eq!(v["$array"], json!([0, 1, 2, 3, 4, 5, 6, 7]));
    // Blender's two-argument form returns the filled list.
    let v = val(
        &mut s,
        "bpy.data.meshes['Cube'].vertices[0:1].foreach_get('co', [0, 0, 0])",
    );
    assert_eq!(v, json!([1, 1, 1]));
    // print() of the one-argument form prints the array (M5+M6 stabilization round 7).
    let r = one(
        &mut s,
        "print(bpy.data.meshes['Cube'].vertices.foreach_get('index'), 1)",
    );
    assert_eq!(r["kind"], "print");
    assert_eq!(r["value"][0]["$array"], json!([0, 1, 2, 3, 4, 5, 6, 7]));
    assert_eq!(r["value"][1], 1);
    // --max-array caps whole items.
    let mut s = session_with(|o| o.max_array = 7);
    let v = val(&mut s, "bpy.data.meshes['Cube'].vertices.foreach_get('co')");
    assert_eq!(v["len"], 2);
    assert_eq!(v["shape"], json!([2, 3]));
    assert_eq!(v["range"], json!([0, 2]));
    assert_eq!(v["truncated"], true);
    assert_eq!(v["$array"].as_array().unwrap().len(), 6);
    // Errors.
    let mut s = session();
    let cases = [
        (
            "bpy.data.meshes['Cube'].vertices.foreach_get('co', [0, 0, 0])",
            "RuntimeError",
            "internal error setting the array",
        ),
        (
            "bpy.data.meshes['Cube'].vertices.foreach_get('co', 5)",
            "TypeError",
            "couldn't access the py sequence",
        ),
        (
            "bpy.data.meshes['Cube'].vertices.foreach_get(3, [])",
            "TypeError",
            "foreach_get/set() argument 1 must be str, not int",
        ),
        (
            "bpy.data.meshes['Cube'].vertices.foreach_set('co')",
            "TypeError",
            "foreach_get/set() takes exactly 2 arguments (1 given)",
        ),
        (
            "bpy.data.meshes['Cube'].vertices.foreach_set('co', [0, 0, 0])",
            "RuntimeError",
            "internal error setting the array",
        ),
        (
            "bpy.data.objects.foreach_get('name', ['', '', ''])",
            "RuntimeError",
            "internal error setting the array",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
    // An empty buffer short-circuits before the attribute lookup (Blender 5.2.4, verified).
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['Cube'].vertices.foreach_get('foo', [])"
        ),
        J::Null
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['Cube'].vertices.foreach_set('co', [])"
        ),
        J::Null
    );
    let (_, _, m) = err(
        &mut s,
        "bpy.data.meshes['Cube'].vertices.foreach_get('foo', [0]*24)",
    );
    assert_eq!(
        m,
        format!(
            "foreach_get(..) 'Mesh.{:>200}[...]' elements have no attribute 'foo'",
            "vertices"
        )
    );
}

#[test]
fn bulk_writes() {
    let mut s = session();
    let flat: Vec<String> = (0..24).map(|i| i.to_string()).collect();
    let r = one(
        &mut s,
        &format!(
            "bpy.data.meshes['Cube'].vertices.foreach_set('co', [{}])",
            flat.join(", ")
        ),
    );
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(r["value"], J::Null);
    assert_eq!(r["changes"]["modified"], json!(["bpy.data.meshes['Cube']"]));
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[2].co"),
        json!([6, 7, 8])
    );
    // A slice writes only its items.
    val(
        &mut s,
        "bpy.data.meshes['Cube'].vertices[0:1].foreach_set('co', (9, 9, 9))",
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co"),
        json!([9, 9, 9])
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[1].co"),
        json!([3, 4, 5])
    );
    // Read-only properties are ignored, as in Blender, with a frap WARNING.
    let r = one(
        &mut s,
        "bpy.data.meshes['Cube'].vertices.foreach_set('index', [0, 0, 0, 0, 0, 0, 0, 0])",
    );
    assert_eq!(r["status"], "FINISHED");
    assert_eq!(r["reports"][0]["type"], "WARNING");
}

#[test]
fn special_floats_and_strings() {
    let mut s = session();
    let r = one(&mut s, "bpy.data.objects['Cube'].location.x = float('nan')");
    assert_eq!(r["value"], json!({"$float": "nan"}));
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location")[0],
        json!({"$float": "nan"})
    );
    // -inf is clamped to the hard minimum (-FLT_MAX), as Blender's RNA clamp does.
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].location = (0.1, -float('inf'), 1e3)",
    );
    assert_eq!(r["reports"][0]["type"], "INFO", "{r}");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([0.1, -3.4028235e38, 1000])
    );
    assert_eq!(val(&mut s, "bpy.data.objects['Cube'].name"), json!("Cube"));
    // A clamping component write reports like a whole-array write.
    let r = one(&mut s, "bpy.data.objects['Cube'].location.y = float('inf')");
    assert_eq!(r["reports"][0]["type"], "INFO", "{r}");
    assert_eq!(r["value"], json!(3.4028235e38));
    // -0.0 keeps its sign (as Python prints it).
    let r = one(&mut s, "bpy.data.objects['Cube'].location.z = -0.0");
    assert_eq!(r["value"].to_string(), "-0.0");
    let loc = val(&mut s, "bpy.data.objects['Cube'].location");
    assert_eq!(loc[2].to_string(), "-0.0");
    assert_eq!(val(&mut s, "print(-0.0, 0.0)").to_string(), "[-0.0,0]");
    // A non-finite double (`Vector.length`) is tagged, not `null`.
    val(&mut s, "bpy.data.objects['Cube'].location.x = float('nan')");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location.length"),
        json!({"$float": "nan"})
    );
}

// ------------------------------------------------------------------ set

#[test]
fn assignments_coerce_echo_and_log_changes() {
    let mut s = session();
    let r = one(&mut s, "bpy.data.objects['Cube'].location = (1, 2, 3)");
    assert_eq!(r["kind"], "set");
    assert_eq!(r["path"], "bpy.data.objects['Cube'].location");
    assert_eq!(r["value"], json!([1, 2, 3]));
    assert!(r.get("rna").is_none());
    assert_eq!(
        r["changes"],
        json!({"added": [], "removed": [], "modified": ["bpy.data.objects['Cube']"], "renamed": []})
    );
    let r = one(&mut s, "data.objects['Cube'].location = 4,5,6");
    assert_eq!(r["value"], json!([4, 5, 6]));
    let r = one(&mut s, "bpy.data.objects['Cube'].location.x = 7");
    assert_eq!(r["path"], "bpy.data.objects['Cube'].location.x");
    assert_eq!(r["value"], json!(7));
    val(&mut s, "bpy.data.objects['Cube'].location[1] = 8");
    val(&mut s, "bpy.data.objects['Cube'].location[-1] = 9");
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([7, 8, 9])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location[0:2] = (1, 2)"),
        json!([1, 2])
    );
    // Augmented assignment follows mathutils.
    val(&mut s, "bpy.data.objects['Cube'].location *= 2");
    val(
        &mut s,
        "bpy.data.objects['Cube'].location += Vector((1, 1, 1))",
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([3, 5, 19])
    );
    val(&mut s, "bpy.data.objects['Cube'].location.x -= 1");
    assert_eq!(val(&mut s, "bpy.data.objects['Cube'].location.x"), json!(2));
    // Expressions and mathutils.
    val(
        &mut s,
        "bpy.data.objects['Cube'].location = Matrix.Translation((1, 2, 3)) @ Vector((1, 1, 1))",
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([2, 3, 4])
    );
    val(
        &mut s,
        "bpy.data.objects['Cube'].location = -Vector((1, 2, 3)) * 2",
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([-2, -4, -6])
    );
    // Nested struct writes.
    val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co = (0, 0, 5)");
    val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co.x = 2");
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].co"),
        json!([2, 0, 5])
    );
}

#[test]
fn assignment_errors_use_blender_wording() {
    let mut s = session();
    let cases = [
        (
            "bpy.data.objects['Cube'].location = (1, 2)",
            "ValueError",
            "RNA_LENGTH",
            "bpy_struct: item.attr = val: sequences of dimension 0 should contain 3 items, not 2",
        ),
        (
            "bpy.data.objects['Cube'].location = 'abc'",
            "TypeError",
            "RNA_TYPE",
            "bpy_struct: item.attr = val: expected sequence items of type float, not str",
        ),
        (
            "bpy.data.objects['Cube'].location = 5",
            "TypeError",
            "RNA_TYPE",
            "bpy_struct: item.attr = val:",
        ),
        (
            "bpy.data.objects['Cube'].location += (1, 1, 1)",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "Vector addition: (Vector += tuple) invalid type for this operation",
        ),
        (
            "bpy.data.objects['Cube'].location *= (1, 1, 1)",
            "TypeError",
            "RNA_TYPE",
            "In place element-wise multiplication: not supported between 'Vector' and 'tuple' types",
        ),
        (
            "bpy.data.objects['Cube'].location /= 0",
            "ValueError",
            "ZERO_DIVISION",
            "Vector division: divide by zero error",
        ),
        (
            "bpy.data.objects['Cube'].location.x /= 0",
            "ValueError",
            "ZERO_DIVISION",
            "float division by zero",
        ),
        (
            "bpy.data.objects['Cube'].location[5] = 1",
            "IndexError",
            "RNA_INDEX_RANGE",
            "vector[index] = x: assignment index out of range",
        ),
        (
            "bpy.data.objects['Cube'].location[1] = 'a'",
            "TypeError",
            "RNA_TYPE",
            "vector[index] = x: assigned value not a number",
        ),
        (
            "bpy.data.objects['Cube'].location[0:2] = (1, 2, 3)",
            "ValueError",
            "RNA_LENGTH",
            "vector[slice] = seq: sequence size is 3, expected 2",
        ),
        (
            "bpy.data.objects['Cube'].location[0:3] = 5",
            "TypeError",
            "RNA_TYPE",
            "vector[slice] = seq",
        ),
        (
            "bpy.data.objects['Cube'].location.foo = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Vector' object has no attribute 'foo' and no __dict__ for setting new attributes",
        ),
        (
            "bpy.data.objects['Cube'].name.x = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'str' object has no attribute 'x' and no __dict__ for setting new attributes",
        ),
        (
            "bpy.data.objects['Cube'].foo = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Object' object has no attribute 'foo'",
        ),
        (
            "bpy.context.scene.cursor.foo = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'View3DCursor' object has no attribute 'foo'",
        ),
        (
            "bpy.context.object.foo = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'Object' object has no attribute 'foo'",
        ),
        (
            "bpy.data.objects['Cube'].type = 'MESH'",
            "AttributeError",
            "RNA_READONLY",
            "bpy_struct: attribute \"type\" from \"Object\" is read-only",
        ),
        (
            "bpy.data.objects['Cube'].modifiers = []",
            "AttributeError",
            "RNA_READONLY",
            "bpy_struct: attribute \"modifiers\" from \"Object\" is read-only",
        ),
        (
            "bpy.data.objects = 3",
            "AttributeError",
            "RNA_READONLY",
            "bpy_struct: attribute \"objects\" from \"BlendData\" is read-only",
        ),
        (
            "bpy.data.filepath = 'x'",
            "AttributeError",
            "RNA_READONLY",
            "bpy_struct: attribute \"filepath\" from \"BlendData\" is read-only",
        ),
        (
            "bpy.data.objects['Cube'] = 3",
            "TypeError",
            "RNA_TYPE",
            "bpy_prop_collection[key] = value: invalid, expected a StructRNA type or None, not a int",
        ),
        (
            "bpy.data.objects[0] = 3",
            "TypeError",
            "RNA_TYPE",
            "bpy_prop_collection[key] = value: invalid, expected a StructRNA type or None, not a int",
        ),
        (
            "bpy.data.meshes['Cube'].vertices = 3",
            "AttributeError",
            "RNA_READONLY",
            "bpy_struct: attribute \"vertices\" from \"Mesh\" is read-only",
        ),
        (
            "bpy.context.scene.cursor.rotation_quaternion[4] = 1",
            "IndexError",
            "RNA_INDEX_RANGE",
            "quaternion[attribute] = x: array assignment index out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle[9] = 1",
            "IndexError",
            "RNA_INDEX_RANGE",
            "bpy_prop_array[index] = value: index out of range",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle[1] = 'x'",
            "TypeError",
            "RNA_TYPE",
            "expected a float type",
        ),
        (
            "bpy.context.scene.cursor.rotation_axis_angle[0:2] = (1, 2, 3)",
            "TypeError",
            "RNA_LENGTH",
            "bpy_prop_array[slice] = value: re-sizing bpy_struct arrays isn't supported",
        ),
        (
            "bpy.context.scene.cursor.matrix[4] = (1, 1, 1, 1)",
            "IndexError",
            "RNA_INDEX_RANGE",
            "matrix[attribute] = x: bad row",
        ),
        (
            "bpy.context.scene.cursor.matrix[-5] = (1, 1, 1, 1)",
            "IndexError",
            "RNA_INDEX_RANGE",
            "matrix[attribute] = x: bad row",
        ),
        (
            "bpy.context.scene.cursor.matrix[0] = (1, 2, 3)",
            "ValueError",
            "RNA_LENGTH",
            "matrix[i] = value assignment: sequence length is 3, expected 4",
        ),
        (
            "bpy.context.scene.cursor.matrix[0] = 3",
            "TypeError",
            "RNA_TYPE",
            "matrix[i] = value assignment",
        ),
        (
            "bpy.context.scene.cursor.matrix[0:2] = ((0, 1, 0, 0),)",
            "ValueError",
            "RNA_LENGTH",
            "matrix[slice] = value: sequence size is 1, expected 2",
        ),
        (
            "bpy.context.scene.cursor.matrix[0:2] = (1, 2)",
            "TypeError",
            "RNA_TYPE",
            "matrix[slice] = value",
        ),
        (
            "bpy.context.scene.cursor.matrix[0:2] = ((0, 1, 0), (1, 0, 0, 0))",
            "ValueError",
            "RNA_LENGTH",
            "matrix[slice] = value: sequence length is 3, expected 4",
        ),
        (
            // Python's setattr wording on `None` (no object is in Edit Mode).
            "bpy.context.edit_object.foo = 1",
            "AttributeError",
            "RNA_NO_ATTRIBUTE",
            "'NoneType' object has no attribute 'foo' and no __dict__ for setting new attributes",
        ),
    ];
    for (src, exc, code, msg) in cases {
        assert_eq!(
            err(&mut s, src),
            (exc.into(), code.into(), msg.into()),
            "{src}"
        );
    }
    // Nothing above changed the state.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([0, 0, 0])
    );
}

// ------------------------------------------------------------------ call / print / op_meta

/// A function of a collection whose API struct is not dumped (`Scene.keying_sets`,
/// `KeyingSets.new`) is the collection's `RNA_NOT_IMPLEMENTED`, as its read is, not an
/// `AttributeError` (M5+M6 stabilization round 7). Since M7 W0 dumped `TimelineMarkers`, its
/// unimplemented `new` is a dumped function frap lacks (`NOT_IMPLEMENTED`, until WP 7.6).
#[test]
fn unbound_collection_functions_are_not_implemented() {
    let mut s = session();
    let (exc, code, msg) = err(
        &mut s,
        "bpy.context.scene.timeline_markers.new('F', frame=1)",
    );
    assert_eq!(
        (exc.as_str(), code.as_str()),
        ("NotImplementedError", "NOT_IMPLEMENTED")
    );
    assert_eq!(
        msg,
        "frap: TimelineMarkers.new() exists in Blender 5.2 but is not implemented by frap yet"
    );
    let (exc, code, msg) = err(&mut s, "bpy.context.scene.keying_sets.new()");
    assert_eq!(exc, "NotImplementedError");
    assert_eq!(code, "RNA_NOT_IMPLEMENTED");
    assert_eq!(
        msg,
        "frap: Scene.keying_sets exists in Blender 5.2 but is not bound by frap yet"
    );
}

/// `bpy_struct.driver_add/driver_remove` exist on every struct in Blender; drivers are M8, so
/// they answer `RNA_NOT_IMPLEMENTED` with the plan, not `AttributeError` (M5+M6 stabilization
/// round 8).
#[test]
fn driver_methods_are_not_implemented() {
    let mut s = session();
    for (src, func) in [
        ("bpy.context.object.driver_add('location', 0)", "driver_add"),
        (
            "bpy.context.object.driver_remove('location')",
            "driver_remove",
        ),
        (
            "bpy.context.scene.render.driver_add('resolution_x')",
            "driver_add",
        ),
    ] {
        let (exc, code, msg) = err(&mut s, src);
        assert_eq!(exc, "NotImplementedError", "{src}");
        assert_eq!(code, "RNA_NOT_IMPLEMENTED", "{src}");
        assert_eq!(
            msg,
            format!(
                "frap: bpy_struct.{func}() exists in Blender 5.2 but drivers are not \
                 implemented by frap yet (planned for M8)"
            )
        );
    }
    // Still an AttributeError for a name no struct has.
    let (exc, _, _) = err(&mut s, "bpy.context.object.driver_addx('location')");
    assert_eq!(exc, "AttributeError");
}

#[test]
fn collection_methods() {
    let mut s = session();
    assert_eq!(
        val(&mut s, "bpy.data.objects.get('Cube')"),
        json!({"$ref": "bpy.data.objects['Cube']"})
    );
    assert_eq!(val(&mut s, "bpy.data.objects.get('Nope', 5)"), json!(5));
    assert_eq!(val(&mut s, "bpy.data.objects.get('Nope')"), J::Null);
    assert_eq!(
        val(&mut s, "bpy.data.objects.get(('Cube', None))"),
        json!({"$ref": "bpy.data.objects['Cube']"})
    );
    assert_eq!(val(&mut s, "bpy.data.objects.find('Cube')"), json!(1));
    assert_eq!(val(&mut s, "bpy.data.objects.find('Nope')"), json!(-1));
    assert_eq!(
        val(&mut s, "bpy.data.objects.keys()"),
        json!(["Camera", "Cube", "Light"])
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices.keys()"),
        json!([])
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices.find('a')"),
        json!(-1)
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects.items()")[1],
        json!(["Cube", {"$ref": "bpy.data.objects['Cube']"}])
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects.values()")
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // Calls inside a path: the path continues on the result.
    let r = one(&mut s, "bpy.data.objects.get('Cube').location");
    assert_eq!(r["status"], "FINISHED", "{r}");
    assert_eq!(r["path"], "bpy.data.objects['Cube'].location");
    assert_eq!(r["value"], json!([0, 0, 0]));
    let cases = [
        (
            "bpy.data.objects.get()",
            "TypeError",
            "get() takes at least 1 argument (0 given)",
        ),
        (
            "bpy.data.objects.get('Cube', 1, 2)",
            "TypeError",
            "get() takes at most 2 arguments (3 given)",
        ),
        (
            "bpy.data.objects.find('Cube', 3)",
            "TypeError",
            "bpy_prop_collection.find() takes exactly one argument (2 given)",
        ),
        (
            "bpy.data.objects.keys(3)",
            "TypeError",
            "bpy_prop_collection.keys() takes no arguments (1 given)",
        ),
        (
            "bpy.data.meshes['Cube'].vertices.get('a')",
            "TypeError",
            "bpy_prop_collection.get: Mesh.vertices does not support string lookups",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
}

#[test]
fn struct_methods() {
    let mut s = session();
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['Cube'].vertices[0].path_from_id('co')"
        ),
        json!("vertices[0].co")
    );
    assert_eq!(
        val(&mut s, "bpy.data.meshes['Cube'].vertices[0].path_from_id()"),
        json!("vertices[0]")
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].path_from_id('location')"),
        json!("location")
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.data.objects['Cube'].path_resolve('location[1]')"
        ),
        json!(0)
    );
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].path_resolve('location')"),
        json!([0, 0, 0])
    );
    // The result keeps its mathutils type, so the path continues after the call.
    assert_eq!(
        val(
            &mut s,
            "bpy.data.objects['Cube'].path_resolve('location').x"
        ),
        json!(0)
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.context.scene.cursor.path_resolve('rotation_euler').order"
        ),
        json!("XYZ")
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.context.scene.cursor.path_resolve('rotation_quaternion').w"
        ),
        json!(1)
    );
    assert_eq!(
        val(
            &mut s,
            "bpy.data.meshes['Cube'].vertices[0].path_resolve('co')[0]"
        ),
        json!(1)
    );
    let cases = [
        (
            "bpy.data.objects['Cube'].path_from_id()",
            "ValueError",
            "Object.path_from_id() does not support path creation for this type",
        ),
        (
            "bpy.data.objects['Cube'].path_from_id('foo')",
            "AttributeError",
            "Object.path_from_id(\"foo\") not found",
        ),
        (
            "bpy.data.objects['Cube'].path_resolve('foo')",
            "ValueError",
            "Object.path_resolve(\"foo\") could not be resolved",
        ),
        (
            "bpy.data.objects['Cube'].path_resolve('location[9]')",
            "ValueError",
            "Object.path_resolve(\"location[9]\") could not be resolved",
        ),
        (
            "bpy.data.objects['Cube'].path_resolve('data.vertices[100]')",
            "ValueError",
            "Object.path_resolve(\"data.vertices[100]\") could not be resolved",
        ),
    ];
    for (src, exc, msg) in cases {
        let (e, _, m) = err(&mut s, src);
        assert_eq!((e.as_str(), m.as_str()), (exc, msg), "{src}");
    }
}

#[test]
fn rna_function_binding_errors() {
    let mut s = session();
    let cases = [
        (
            "bpy.data.objects.new()",
            "BlendDataObjects.new(): required parameter \"name\" not specified",
        ),
        (
            "bpy.data.objects.new('a', None, 3)",
            "BlendDataObjects.new(): takes at most 2 arguments, got 3",
        ),
        (
            "bpy.data.objects.new(nam='a', object_data=None)",
            "BlendDataObjects.new(): required parameter \"name\" not specified",
        ),
        (
            "bpy.data.objects.new(1, None)",
            "BlendDataObjects.new(): error with argument 1, \"name\" -  Function.name expected a string type, not int",
        ),
        (
            "bpy.data.objects.new('a', 3)",
            "BlendDataObjects.new(): error with argument 2, \"object_data\" -  Function.object_data expected a ID type, not int",
        ),
        (
            "bpy.data.objects['Cube'].shape_key_add('X')",
            "Object.shape_key_add(): required parameter \"name\" to be a keyword argument!",
        ),
        (
            "bpy.data.objects['Cube'].select_set(True, nope=1)",
            "Object.select_set(): was called with invalid keyword argument(s) (nope), expected (state, view_layer)",
        ),
        (
            "bpy.data.objects['Cube'].is_deform_modified(bpy.context.scene, 'FOO')",
            "Object.is_deform_modified(): error with argument 2, \"settings\" -  enum \"FOO\" not found in ('PREVIEW', 'RENDER')",
        ),
        (
            "bpy.data.objects['Cube'].is_deform_modified(bpy.context.scene)",
            "Object.is_deform_modified(): required parameter \"settings\" not specified",
        ),
        (
            "bpy.data.objects['Cube'].is_deform_modified(bpy.data.objects['Cube'], 'RENDER')",
            "Object.is_deform_modified(): error with argument 1, \"scene\" -  Function.scene expected a Scene type, not Object",
        ),
        (
            "bpy.data.objects['Cube'].keyframe_insert('location', 2, 20)",
            "bpy_struct.keyframe_insert()() takes at most 1 positional argument (3 given)",
        ),
        (
            "bpy.data.objects['Cube'].keyframe_insert()",
            "bpy_struct.keyframe_insert()() missing required argument 'data_path' (pos 1)",
        ),
        (
            "bpy.data.objects['Cube'].keyframe_insert('location', nope=3)",
            "bpy_struct.keyframe_insert()() got an unexpected keyword argument 'nope'",
        ),
    ];
    for (src, msg) in cases {
        let (e, _, m) = err(&mut s, src);
        assert_eq!((e.as_str(), m.as_str()), ("TypeError", msg), "{src}");
    }
    // Dispatch through RNA_FUNCS with bound arguments (keyword or positional).
    let r = one(
        &mut s,
        "bpy.data.objects['Cube'].is_deform_modified(bpy.context.scene, 'RENDER')",
    );
    assert_eq!(r["kind"], "call");
    assert_eq!(r["func"], "bpy.data.objects['Cube'].is_deform_modified");
    assert_eq!(r["value"], true);
    assert_eq!(r["rna"]["type"], "BOOLEAN");
    assert_eq!(r["changes"]["modified"], json!([]));
    let r = one(
        &mut s,
        "data.objects['Cube'].is_deform_modified scene=data.scenes['Scene'] settings=PREVIEW",
    );
    assert_eq!(r["value"], false, "{r}");
    // A well-formed call of a function frap does not implement yet.
    let r = one(&mut s, "bpy.data.objects['Cube'].shape_keys_selected()");
    assert_eq!(r["error"]["code"], "NOT_IMPLEMENTED", "{r}");
    assert_eq!(r["kind"], "call");
}

#[test]
fn print_returns_json_values() {
    let mut s = session();
    let r = one(
        &mut s,
        "print(bpy.data.objects['Cube'].location, 1 + 2, 'a', radians(180), bpy.data.objects['Cube'], 7 / 2)",
    );
    assert_eq!(r["kind"], "print");
    assert_eq!(
        r["value"],
        json!([[0, 0, 0], 3, "a", std::f64::consts::PI, {"$ref": "bpy.data.objects['Cube']"}, 3.5])
    );
    assert_eq!(
        val(
            &mut s,
            "print(Euler((0, 0, 0.5)).to_matrix() @ Vector((1, 0, 0)))"
        ),
        // mathutils is single precision: the shortest f32 strings.
        json!([[0.87758255, 0.47942555, 0]])
    );
    let (e, code, m) = err(&mut s, "print(1 / 0)");
    assert_eq!(
        (e.as_str(), code.as_str(), m.as_str()),
        ("ValueError", "ZERO_DIVISION", "division by zero")
    );
}

#[test]
fn op_meta_queries() {
    let mut s = session();
    let r = one(&mut s, "bpy.ops.mesh.primitive_cube_add.poll()");
    assert_eq!(r["kind"], "op_meta");
    assert_eq!(r["op"], "mesh.primitive_cube_add");
    assert_eq!(r["value"], true);
    assert_eq!(
        val(&mut s, "bpy.ops.mesh.primitive_cube_add.idname()"),
        "MESH_OT_primitive_cube_add"
    );
    assert_eq!(
        val(&mut s, "bpy.ops.mesh.primitive_cube_add.idname_py()"),
        "mesh.primitive_cube_add"
    );
    assert_eq!(val(&mut s, "bpy.ops.mesh.foo.idname()"), "MESH_OT_foo");
    let t = val(&mut s, "bpy.ops.mesh.primitive_cube_add.get_rna_type()");
    assert_eq!(t["$struct"], "MESH_OT_primitive_cube_add");
    assert_eq!(t["properties"][0]["identifier"], "size");
    assert_eq!(t["properties"][0]["default"], 2);
    let (e, code, m) = err(&mut s, "bpy.ops.mesh.foo.poll()");
    assert_eq!(e, "AttributeError");
    assert_eq!(code, "OP_UNKNOWN");
    assert_eq!(
        m,
        "Polling operator \"bpy.ops.mesh.foo\" error, could not be found"
    );
}

// ------------------------------------------------------------------ changes / context / reads

#[test]
fn operator_changes_and_context() {
    let mut s = session();
    let r = one(
        &mut s,
        "bpy.ops.mesh.primitive_cube_add(size=2, location=(0, 0, 1))",
    );
    assert_eq!(
        r["changes"]["added"],
        json!([
            "bpy.data.objects['Cube.001']",
            "bpy.data.meshes['Cube.001']"
        ])
    );
    assert_eq!(
        r["context"],
        json!({"mode": "OBJECT", "active_object": "Cube.001", "selected_objects": ["Cube.001"], "frame_current": 1})
    );
    assert_eq!(
        r["result"]["object"],
        json!({"$ref": "bpy.data.objects['Cube.001']"})
    );
    assert_eq!(r["hints"], json!([]));
    // A pure read has no `changes`.
    let r = one(&mut s, "bpy.data.objects['Cube.001'].location");
    assert!(r.get("changes").is_none(), "{r}");
    assert!(r.get("state").is_none());
}

#[test]
fn reads_leave_the_state_bit_identical() {
    let mut s = session();
    one(&mut s, "bpy.ops.mesh.primitive_cube_add()");
    let before = s.state_dump();
    for src in [
        "bpy.data.objects['Cube'].location",
        "bpy.data.objects",
        "bpy.data.meshes['Cube'].vertices.foreach_get('co')",
        "bpy.data.meshes['Cube'].vertices[0:3]",
        "bpy.data.objects.get('Cube').name",
        "bpy.context.selected_objects",
        "bpy.context.scene.cursor.matrix",
        "print(bpy.data.objects['Cube'].location * 2)",
        "bpy.ops.mesh.primitive_cube_add.poll()",
        "bpy.data.objects['Nope']",
        "bpy.data.objects['Cube'].location[9]",
        "bpy.data.objects['Cube'].path_resolve('location')",
        "import bpy",
    ] {
        let r = one(&mut s, src);
        assert_ne!(r["status"], "CANCELLED", "{src}");
        assert_eq!(s.state_dump(), before, "{src} changed the state");
    }
}

#[test]
fn verbosity_minimal_drops_context() {
    let mut s = session_with(|o| o.verbosity = frap_session::Verbosity::Minimal);
    let r = one(&mut s, "bpy.data.objects");
    assert!(r.get("context").is_none());
}

// ------------------------------------------------------------------ serve framing

#[test]
fn exec_request_echoes_ids_and_skips_after_a_failure() {
    let mut s = session();
    let req: Request = serde_json::from_value(json!({
        "id": 7,
        "src": "bpy.ops.mesh.primitive_cube_add()\nbpy.data.objects['Nope'].location\nbpy.data.objects['Cube'].location = (1, 2, 3)\nbpy.ops.mesh.foo(",
    }))
    .unwrap();
    let rs = s.exec_request(req);
    let j: Vec<J> = rs.iter().map(|r| r.to_json()).collect();
    assert_eq!(j.len(), 4);
    for (i, r) in j.iter().enumerate() {
        assert_eq!(r["id"], 7);
        assert_eq!(r["line"], i as u64 + 1);
        assert_eq!(r["seq"], i as u64 + 1);
    }
    assert_eq!(j[0]["status"], "FINISHED");
    assert_eq!(j[1]["status"], "ERROR");
    assert_eq!(j[2]["status"], "SKIPPED");
    assert_eq!(j[2]["kind"], "set");
    assert_eq!(j[2]["state"], "unchanged");
    assert!(j[2].get("error").is_none());
    assert_eq!(j[3]["status"], "SKIPPED");
    assert_eq!(j[3]["kind"], "invalid");
    // The skipped assignment did not run.
    assert_eq!(
        val(&mut s, "bpy.data.objects['Cube'].location"),
        json!([0, 0, 0])
    );
    let done = DoneRecord::from_responses(json!(7), &rs);
    assert_eq!(
        done.to_json(),
        json!({"id": 7, "event": "done", "ok": false,
               "counts": {"finished": 1, "error": 1, "cancelled": 0, "skipped": 2}})
    );
    // `continue` runs everything.
    let rs = s.exec_request(Request {
        id: json!("b"),
        src: "bpy.data.objects['Nope']\nbpy.data.objects['Cube'].location = (1, 2, 3)".into(),
        on_error: OnError::Continue,
    });
    assert_eq!(rs[0].status, Status::Error);
    assert_eq!(rs[1].status, Status::Finished);
    assert_eq!(rs[1].id, Some(json!("b")));
    assert_eq!(rs[1].seq, 7);
    let done = DoneRecord::from_responses(json!("b"), &rs);
    assert!(!done.ok);
    assert_eq!((done.counts.finished, done.counts.error), (1, 1));
    // All finished; an empty request is ok.
    let rs = s.exec_request(Request {
        id: json!(1),
        src: "bpy.data.objects".into(),
        on_error: OnError::SkipRest,
    });
    assert!(DoneRecord::from_responses(json!(1), &rs).ok);
    let rs = s.exec_request(Request {
        id: json!(2),
        src: String::new(),
        on_error: OnError::SkipRest,
    });
    let d = DoneRecord::from_responses(json!(2), &rs);
    assert!(d.ok);
    assert_eq!(d.counts.finished, rs.len() as u32);
}

#[test]
fn unclosed_bracket_in_a_request_is_an_immediate_syntax_error() {
    let mut s = session();
    let rs = s.exec_request(Request {
        id: json!(1),
        src: "bpy.ops.mesh.primitive_cube_add(size=2".into(),
        on_error: OnError::SkipRest,
    });
    assert_eq!(rs.len(), 1);
    let j = rs[0].to_json();
    assert_eq!(j["kind"], "invalid");
    assert_eq!(j["error"]["message"], "'(' was never closed");
}

// ------------------------------------------------------------------ front-end-free API

#[test]
fn session_api_get_set_call_invoke() {
    let mut s = session();
    let r = s.set(
        "bpy.data.objects['Cube'].location",
        Input::Tuple(vec![Input::Int(1), Input::Float(2.5), Input::Int(3)]),
    );
    assert_eq!(r.status, Status::Finished, "{}", r.to_json());
    assert_eq!(r.value, Some(json!([1, 2.5, 3])));
    let r = s.get("bpy.data.objects['Cube'].location");
    assert_eq!(r.value, Some(json!([1, 2.5, 3])));
    assert_eq!(r.kind, "get");
    let r = s.call(
        "bpy.data.objects.find",
        vec![Input::Str("Cube".into())],
        IndexMap::new(),
    );
    assert_eq!(r.kind, "call");
    assert_eq!(r.value, Some(json!(1)));
    let mut kw = IndexMap::new();
    kw.insert("size".to_owned(), Input::Int(4));
    let r = s.invoke("mesh.primitive_cube_add", kw);
    assert_eq!(r.status, Status::Finished);
    assert_eq!(r.op.as_deref(), Some("mesh.primitive_cube_add"));
    let r = s.get("not a path(");
    assert_eq!(r.status, Status::Error);
    let e: &FrapError = r.error.as_ref().unwrap();
    assert_eq!(e.exception.as_str(), "SyntaxError");
}
