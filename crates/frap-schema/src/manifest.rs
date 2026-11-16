// SPDX-License-Identifier: GPL-3.0-or-later
//! `--describe` manifest: operators, structs, context members, modifier and node types, `defaults_that_surprise` (§14). WP 1.18.
//!
//! M1 W0 stub: [`describe_op`] emits a minimal operator fragment so that the per-operator
//! snapshot test (`tests/describe_ops.rs`, §22.1) runs from day one. WP 1.18 replaces the
//! fragment with the full §14 shape and re-blesses every `tests/snapshots/op__<idname>.snap`.

use frap_op::OperatorDef;
use serde_json::{Value, json};

/// The `--describe op:<idname>` fragment of one registered operator.
pub fn describe_op(def: &OperatorDef) -> Value {
    let spec = def.spec;
    let props: Vec<Value> = spec
        .props
        .iter()
        .map(|p| {
            json!({
                "identifier": p.identifier,
                "type": p.type_str(),
                "array_length": p.array_len(),
            })
        })
        .collect();
    let requires: Vec<String> = def.poll.requires.iter().map(|r| format!("{r:?}")).collect();
    json!({
        "idname": spec.idname,
        "bl_idname": spec.bl_idname,
        "label": spec.label,
        "description": spec.description,
        "props": props,
        "frap": {
            "status": "implemented",
            "tier": format!("{:?}", def.tier),
            "requires": requires,
            "notes": def.notes,
        },
    })
}
