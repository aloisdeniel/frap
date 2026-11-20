// SPDX-License-Identifier: GPL-3.0-or-later
//! `view_settings.look` items per view transform and the look remap of the `view_transform`
//! setter, against Blender 5.2.4 (`tests/data/look_items_5_2_4.json`, dumped with
//! `-b --factory-startup`, display sRGB).

use frap_color::ViewTransform;
use frap_color::looks::{looks_for, ui_name, validate_for_view};

fn table() -> serde_json::Value {
    serde_json::from_str(include_str!("data/look_items_5_2_4.json")).unwrap()
}

#[test]
fn look_items_per_view_equal_blender() {
    let t = table();
    for v in ViewTransform::ALL {
        let items = t["items"][v.name()].as_array().unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i[0].as_str().unwrap()).collect();
        assert_eq!(looks_for(v), ids, "{}", v.name());
        for i in items {
            assert_eq!(ui_name(i[0].as_str().unwrap()), i[1].as_str().unwrap());
        }
    }
}

#[test]
fn view_transform_setter_remaps_the_look_as_blender() {
    let t = table();
    let mut n = 0;
    for (key, by_view) in t["remap"].as_object().unwrap() {
        let (_, look) = key.split_once('|').unwrap();
        for (view, expected) in by_view.as_object().unwrap() {
            let v = ViewTransform::from_name(view).unwrap();
            assert_eq!(
                validate_for_view(v, look),
                expected.as_str().unwrap(),
                "{key} -> {view}"
            );
            n += 1;
        }
    }
    assert_eq!(n, 64 * 9);
    // An unknown name is kept (`get_look_by_name` finds nothing).
    assert_eq!(validate_for_view(ViewTransform::AgX, "Nope"), "Nope");
}
