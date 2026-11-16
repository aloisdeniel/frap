// SPDX-License-Identifier: GPL-3.0-or-later
//! `IdRef` / `IdAny` serde (§15, W0-18): the `{"$id": code, "name": …, "library"?: …}` form,
//! round-trips through a scoped namer and resolver, `Option` fields, nesting, and errors.

use std::collections::HashMap;

use frap_rna::serde_id::{IdNameTable, IdRefResolver, with_namer, with_resolver};
use frap_rna::{IdAny, IdCode, IdKey, IdRef, KeyId, MaterialId, MeshId, ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Shaped like WP 1.4's `Mesh` reference fields.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct MeshRefs {
    materials: Vec<Option<IdRef<MaterialId>>>,
    shape_keys: Option<IdRef<KeyId>>,
    any: IdAny,
}

fn any<K: frap_rna::IdKind>() -> IdAny {
    IdRef::<K>::from_key(IdKey::next()).any()
}

#[test]
fn round_trip_through_namer_and_resolver() {
    let local = any::<MaterialId>();
    let linked = any::<MaterialId>();
    let key = any::<KeyId>();
    let mesh = any::<MeshId>();
    let mut table = IdNameTable::new();
    table.insert(local, "Material", None);
    // Same name, linked from a library: distinguished by the library name.
    table.insert(linked, "Material", Some("lib.blend.001"));
    table.insert(key, "Key", None);
    table.insert(mesh, "Cube", None);

    let v = MeshRefs {
        materials: vec![
            local.typed(),
            None,
            linked.typed(),
            IdRef::<MaterialId>::from_any(local),
        ],
        shape_keys: key.typed(),
        any: mesh,
    };
    let (json, table) = with_namer(table, || serde_json::to_value(&v).unwrap());
    assert_eq!(
        json,
        json!({
            "materials": [
                {"$id": "MA", "name": "Material"},
                null,
                {"$id": "MA", "name": "Material", "library": "lib.blend.001"},
                {"$id": "MA", "name": "Material"},
            ],
            "shape_keys": {"$id": "KE", "name": "Key"},
            "any": {"$id": "ME", "name": "Cube"},
        })
    );
    // Key order is fixed (`$id`, `name`, `library`): diffs stay stable (§15 determinism).
    let (text, table) = with_namer(table, || serde_json::to_string(&v.materials[2]).unwrap());
    assert_eq!(
        text,
        r#"{"$id":"MA","name":"Material","library":"lib.blend.001"}"#
    );

    let (back, _) = with_resolver(table, || {
        serde_json::from_value::<MeshRefs>(json.clone()).unwrap()
    });
    assert_eq!(back, v);
}

/// Like frap-project's loader: keys allocated on first sight, so forward references resolve
/// before the referenced ID is inserted.
#[derive(Default)]
struct Allocating(HashMap<(IdCode, String, Option<String>), IdKey>);
impl IdRefResolver for Allocating {
    fn resolve(&mut self, code: IdCode, name: &str, library: Option<&str>) -> Option<IdKey> {
        Some(
            *self
                .0
                .entry((code, name.to_owned(), library.map(str::to_owned)))
                .or_insert_with(IdKey::next),
        )
    }
}

#[test]
fn allocating_resolver_reuses_keys() {
    let j = json!([
        {"$id": "OB", "name": "Cube"},
        {"$id": "OB", "name": "Cube"},
        {"$id": "OB", "name": "Cube", "library": "lib"},
    ]);
    let (refs, res) = with_resolver(Allocating::default(), || {
        serde_json::from_value::<Vec<IdRef<ObjectId>>>(j).unwrap()
    });
    assert_eq!(refs[0], refs[1]);
    assert_ne!(refs[0], refs[2]);
    assert_eq!(res.0.len(), 2);
}

#[test]
fn nested_scopes_restore_the_outer_one() {
    let a = any::<ObjectId>();
    let mut outer = IdNameTable::new();
    outer.insert(a, "Outer", None);
    let mut inner = IdNameTable::new();
    inner.insert(a, "Inner", None);
    let ((i, o), _) = with_namer(outer, || {
        let (i, _) = with_namer(inner, || serde_json::to_value(a).unwrap());
        (i, serde_json::to_value(a).unwrap())
    });
    assert_eq!(i, json!({"$id": "OB", "name": "Inner"}));
    assert_eq!(o, json!({"$id": "OB", "name": "Outer"}));
    // Nothing installed after the scope ends.
    assert!(serde_json::to_value(a).is_err());
}

#[test]
fn errors() {
    let ob = any::<ObjectId>();
    // No namer / resolver installed.
    let e = serde_json::to_value(ob).unwrap_err().to_string();
    assert!(e.contains("no IdRefNamer installed"), "{e}");
    let e = serde_json::from_value::<IdAny>(json!({"$id": "OB", "name": "Cube"}))
        .unwrap_err()
        .to_string();
    assert!(e.contains("no IdRefResolver installed"), "{e}");

    // Dangling reference on save.
    let (r, _) = with_namer(IdNameTable::new(), || serde_json::to_value(ob));
    let e = r.unwrap_err().to_string();
    assert!(e.starts_with("dangling reference to a OB"), "{e}");

    let mut table = IdNameTable::new();
    table.insert(ob, "Cube", None);
    let load = |j: serde_json::Value, t: IdNameTable| {
        with_resolver(t, || {
            serde_json::from_value::<IdRef<ObjectId>>(j).map_err(|e| e.to_string())
        })
    };
    let (r, t) = load(json!({"$id": "OB", "name": "Cube"}), table);
    assert_eq!(r.unwrap().any(), ob);
    // Wrong kind for a typed reference.
    let (r, t) = load(json!({"$id": "ME", "name": "Cube"}), t);
    assert_eq!(
        r.unwrap_err(),
        "expected a reference to a OB (objects), got \"$id\": \"ME\""
    );
    // Unknown code, unknown field, unresolved names.
    let (r, t) = load(json!({"$id": "ZZ", "name": "Cube"}), t);
    assert_eq!(r.unwrap_err(), "unknown ID code \"ZZ\" in \"$id\"");
    let (r, t) = load(json!({"$id": "OB", "name": "Cube", "extra": 1}), t);
    assert!(r.unwrap_err().contains("unknown field `extra`"));
    let (r, t) = load(json!({"$id": "OB", "name": "Nope"}), t);
    assert_eq!(r.unwrap_err(), "unresolved reference: OB \"Nope\"");
    let (r, _) = load(json!({"$id": "OB", "name": "Cube", "library": "lib"}), t);
    assert_eq!(
        r.unwrap_err(),
        "unresolved reference: OB \"Cube\" in library \"lib\""
    );
}

#[test]
fn id_code_parse() {
    for c in IdCode::ALL {
        assert_eq!(IdCode::from_code(c.as_str()), Some(c));
    }
    assert_eq!(IdCode::from_code("ob"), None);
}
