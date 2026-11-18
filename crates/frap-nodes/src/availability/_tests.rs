// SPDX-License-Identifier: GPL-3.0-or-later
//! WP 3.1 acceptance: the socket `enabled` sets of every node type equal Blender 5.2.4's for
//! every combination of its mode properties (`_tests/availability.blender.json`, written by
//! `_tests/dump_availability.py`). Not a concept file (leading underscore).
#![cfg(test)]

use frap_rna::Value;
use serde_json::Value as J;
use smol_str::SmolStr;

use crate::Node;
use crate::generated::node_type;

const DUMP: &str = include_str!("_tests/availability.blender.json");

fn bits(sockets: &[crate::Socket]) -> String {
    sockets
        .iter()
        .map(|s| if s.enabled { '1' } else { '0' })
        .collect()
}

fn ids(sockets: &[crate::Socket]) -> Vec<&str> {
    sockets.iter().map(|s| s.identifier.as_str()).collect()
}

fn strs(v: &J) -> Vec<&str> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|x| x.as_str().expect("string"))
        .collect()
}

fn as_value(v: &J) -> Value {
    match v {
        J::Bool(b) => Value::Bool(*b),
        J::String(s) => Value::Enum(SmolStr::new(s)),
        other => panic!("unexpected mode value {other}"),
    }
}

fn fresh(bl_idname: &str) -> Node {
    let def = node_type(bl_idname).unwrap_or_else(|| panic!("{bl_idname} not generated"));
    Node::from_type(def, 0, SmolStr::new(def.name))
}

#[test]
fn enabled_sets_equal_blender_for_every_mode_combination() {
    let dump: J = serde_json::from_str(DUMP).expect("valid dump");
    assert_eq!(dump["blender"].as_str(), Some("5.2.4 LTS"));
    let types = dump["types"].as_object().expect("types");
    assert!(types.len() > 100, "every shader node type is dumped");
    let mut cases = 0;
    let mut failures = Vec::new();
    for (bl_idname, t) in types {
        let mut node = fresh(bl_idname);
        assert_eq!(ids(&node.inputs), strs(&t["inputs"]), "{bl_idname} inputs");
        assert_eq!(ids(&node.outputs), strs(&t["outputs"]), "{bl_idname} outputs");
        let default = strs(&t["default"]);
        if [bits(&node.inputs), bits(&node.outputs)] != [default[0], default[1]] {
            failures.push(format!("{bl_idname} fresh: {default:?}"));
        }
        let relevant = strs(&t["relevant"]);
        for case in t["cases"].as_array().expect("cases") {
            let values = case[0].as_array().expect("values");
            // Sequential writes on one node (state carried over, as in a script) and the same
            // writes on a fresh node must agree with Blender.
            let mut once = fresh(bl_idname);
            for (prop, v) in relevant.iter().zip(values) {
                assert!(node.set_prop(prop, as_value(v)), "{bl_idname}.{prop}");
                assert!(once.set_prop(prop, as_value(v)));
            }
            let want = [case[1].as_str().unwrap(), case[2].as_str().unwrap()];
            for (n, how) in [(&node, "sequential"), (&once, "fresh")] {
                if [bits(&n.inputs), bits(&n.outputs)] != want {
                    failures.push(format!(
                        "{bl_idname} {relevant:?}={values:?} ({how}): frap {:?} {:?}, Blender {want:?}",
                        bits(&n.inputs),
                        bits(&n.outputs),
                    ));
                }
            }
            cases += 1;
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(cases > 250, "{cases} cases");
}

/// Every node type whose sockets depend on a mode property has a rule, and no rule exists for a
/// type whose sockets never change (a rule there could only diverge from Blender).
#[test]
fn rules_exist_exactly_for_mode_dependent_types() {
    let dump: J = serde_json::from_str(DUMP).expect("valid dump");
    for (bl_idname, t) in dump["types"].as_object().expect("types") {
        let dependent = !t["relevant"].as_array().expect("relevant").is_empty();
        assert_eq!(
            crate::availability_of(bl_idname).is_some(),
            dependent,
            "{bl_idname}: rule present vs mode-dependent sockets"
        );
    }
}
