// SPDX-License-Identifier: GPL-3.0-or-later
//! Overlay audits run by every `cargo xtask codegen [--check]` (and its tests):
//! * the frap-option audit of ARCHITECTURE §4.2: a `frap_*` operator option must not duplicate a
//!   feature the Blender operator already has a property for (`frap_instancing` duplicated
//!   `export_gpu_instances`, §17.2);
//! * every removed-4.x member of `compat_4x.toml` is really absent from the 5.2 dumps.

use super::model::Spec;

/// frap-option audit; one message per violation.
pub fn frap_options(s: &Spec) -> Vec<String> {
    let mut errors = Vec::new();
    for o in &s.frap_options {
        let ctx = format!("frap_options.toml: {}.{}", o.op, o.identifier);
        if !o.identifier.starts_with("frap_") {
            errors.push(format!("{ctx}: frap options are named frap_*"));
        }
        if !s.frap_features.contains(&o.feature) {
            errors.push(format!("{ctx}: unknown feature `{}`", o.feature));
        }
        let Some(op) = s.ops.iter().find(|x| x.idname == o.op) else {
            errors.push(format!("{ctx}: operator not dumped"));
            continue;
        };
        let blender: Vec<&str> = op
            .props
            .iter()
            .filter(|p| !p.identifier.starts_with("frap_"))
            .map(|p| p.identifier.as_str())
            .collect();
        if blender.contains(&o.identifier.as_str()) {
            errors.push(format!("{ctx}: collides with a Blender property"));
        }
        if let Some(prop) = s.frap_covered.get(&o.op).and_then(|m| m.get(&o.feature))
            && blender.contains(&prop.as_str())
        {
            errors.push(format!(
                "{ctx}: feature `{}` is already covered by Blender's `{prop}`",
                o.feature
            ));
        }
        // A frap option whose suffix names an existing Blender prop is a duplicate too.
        let suffix = o.identifier.trim_start_matches("frap_");
        if let Some(b) = blender
            .iter()
            .find(|b| **b == suffix || **b == format!("export_{suffix}"))
        {
            errors.push(format!("{ctx}: duplicates Blender's `{b}`"));
        }
    }
    for (op, m) in &s.frap_covered {
        let Some(o) = s.ops.iter().find(|x| &x.idname == op) else {
            errors.push(format!(
                "frap_options.toml: [covered] {op}: operator not dumped"
            ));
            continue;
        };
        for (feature, prop) in m {
            if !s.frap_features.contains(feature) {
                errors.push(format!(
                    "frap_options.toml: [covered] {op}: unknown feature {feature}"
                ));
            }
            if !o.props.iter().any(|p| &p.identifier == prop) {
                errors.push(format!(
                    "frap_options.toml: [covered] {op}.{prop}: not a dumped property"
                ));
            }
        }
    }
    errors
}

/// `compat_4x.toml` entries that exist in 5.2 (or name an undumped struct / bad kind).
pub fn compat_4x(s: &Spec) -> Vec<String> {
    let mut errors = Vec::new();
    for c in &s.compat {
        let ctx = format!("compat_4x.toml: {}.{}", c.struct_, c.identifier);
        if !s.structs.contains_key(&c.struct_) {
            errors.push(format!("{ctx}: struct not dumped"));
            continue;
        }
        if c.kind != "prop" && c.kind != "func" {
            errors.push(format!("{ctx}: kind must be prop or func"));
        }
        let found = s.chain(&c.struct_).iter().any(|x| {
            x.props.iter().any(|p| p.identifier == c.identifier)
                || x.funcs.iter().any(|f| f.identifier == c.identifier)
        });
        if found {
            errors.push(format!("{ctx}: exists in Blender 5.2"));
        }
    }
    errors
}
