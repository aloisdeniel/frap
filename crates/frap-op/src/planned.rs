// SPDX-License-Identifier: GPL-3.0-or-later
//! The milestone column of `docs/OPERATORS.md` (§22.1: "Operator-by-operator milestone
//! assignment"), compiled in so `--describe` can tell an agent when a not-yet-implemented
//! operator, modifier evaluator or shader node is planned (`"M2"`) or that it is out of scope
//! (`"OUT"`). The same column drives codegen (§4.2), so the two cannot disagree.
//!
//! Rule (3) of OPERATORS.md: an operator that is not listed is `OUT`. `M9` is the post-M8
//! backlog ([`POST_M8`]).
//!
//! Moved from `frap-schema` (`manifest/planned.rs`) in M8 W0 (decision log M8W0-4) so the
//! `OP_NOT_IMPLEMENTED` hint of `frap-op`'s registry can name the row; `frap-schema`
//! re-exports it.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// `docs/OPERATORS.md`, as compiled.
const OPERATORS_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/OPERATORS.md"
));

/// The `M` cell of a post-M8 row (OPERATORS.md legend, decision log M8W0-3).
pub const POST_M8: &str = "M9";

/// One table row of OPERATORS.md: the first code-spanned cell (operator idname, modifier type
/// or node `bl_idname`), the second code-spanned cell (modifier struct, if any) and the `M`
/// column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedRow {
    /// `"M2"` or `"OUT"`.
    pub milestone: &'static str,
    /// Second code cell (`"SubsurfModifier"` in the modifier table).
    pub second: Option<&'static str>,
    /// Section (`"A"`, `"B"`, `"C"`, …).
    pub section: char,
}

fn is_milestone(cell: &str) -> bool {
    cell == "OUT"
        || (cell.len() >= 2
            && cell.starts_with('M')
            && cell[1..].chars().all(|c| c.is_ascii_digit()))
}

fn code(cell: &str) -> Option<&str> {
    let c = cell.trim();
    (c.len() > 2 && c.starts_with('`') && c.ends_with('`')).then(|| &c[1..c.len() - 1])
}

fn parse(text: &'static str) -> BTreeMap<&'static str, PlannedRow> {
    let mut out = BTreeMap::new();
    let mut section = ' ';
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            section = rest.chars().next().unwrap_or(' ');
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&'static str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let codes: Vec<&'static str> = cells.iter().filter_map(|c| code(c)).collect();
        let Some(milestone) = cells.iter().copied().find(|c| is_milestone(c)) else {
            continue;
        };
        let Some(first) = codes.first().copied() else {
            continue;
        };
        out.entry(first).or_insert(PlannedRow {
            milestone,
            second: codes.get(1).copied(),
            section,
        });
    }
    out
}

fn table() -> &'static BTreeMap<&'static str, PlannedRow> {
    static T: OnceLock<BTreeMap<&'static str, PlannedRow>> = OnceLock::new();
    T.get_or_init(|| parse(OPERATORS_MD))
}

/// The OPERATORS.md row of an operator idname, modifier type or node `bl_idname`.
pub fn row(key: &str) -> Option<&'static PlannedRow> {
    table().get(key)
}

/// Planned milestone of an operator: its `M` column, or `"OUT"` when it is not listed.
pub fn operator_milestone(idname: &str) -> &'static str {
    row(idname)
        .filter(|r| r.section == 'A')
        .map_or("OUT", |r| r.milestone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_tables() {
        assert_eq!(operator_milestone("mesh.primitive_cube_add"), "M1");
        assert_eq!(operator_milestone("mesh.select_all"), "M2");
        assert_eq!(operator_milestone("sculpt.brush_stroke"), "OUT");
        assert_eq!(operator_milestone("object.track_set"), POST_M8);
        assert_eq!(operator_milestone("scene.gltf2_variant_add"), "M8");
        let sub = row("SUBSURF").expect("modifier table row");
        assert_eq!(
            (sub.milestone, sub.second, sub.section),
            ("M4", Some("SubsurfModifier"), 'B')
        );
        assert_eq!(
            row("ShaderNodeBsdfPrincipled").map(|r| r.milestone),
            Some("M1")
        );
    }

    #[test]
    fn synthetic_rows() {
        let t = parse(
            "## A. Ops\n| ✓ | Operator | M |\n|---|---|---|\n| [ ] | `a.b` | M3 | x |\n| — | `a.c` | OUT | — |\n",
        );
        assert_eq!(t["a.b"].milestone, "M3");
        assert_eq!(t["a.c"].milestone, "OUT");
        assert!(!t.contains_key("Operator"));
    }
}
