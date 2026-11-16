// SPDX-License-Identifier: GPL-3.0-or-later
//! Removed 4.x members (ARCHITECTURE §9.3 step 6, `spec/overlay/compat_4x.toml`): accessing one
//! answers Blender's `AttributeError: '<Struct>' object has no attribute '<member>'` with code
//! `RNA_REMOVED_IN_5X` and the 5.x replacement as `hint` (frap extension).

use std::sync::OnceLock;

use frap_rna::{FrapError, PyExc, StructDef, codes};
use serde::Deserialize;

/// The overlay, embedded at build time (the file is checked against the 5.2.4 dumps by xtask).
const COMPAT_4X: &str = include_str!("../../../../spec/overlay/compat_4x.toml");

/// One removed member.
#[derive(Clone, Debug, Deserialize)]
pub struct Removed {
    /// Struct identifier (`Action`).
    #[serde(rename = "struct")]
    pub srna: String,
    /// Member identifier (`fcurves`).
    pub identifier: String,
    /// `prop` or `func`.
    pub kind: String,
    /// Blender version that removed it (`5.0`).
    pub removed_in: String,
    /// The 5.x replacement.
    pub hint: String,
}

#[derive(Deserialize)]
struct File {
    member: Vec<Removed>,
}

/// Every removed member of the overlay.
pub fn table() -> &'static [Removed] {
    static T: OnceLock<Vec<Removed>> = OnceLock::new();
    T.get_or_init(|| {
        toml::from_str::<File>(COMPAT_4X)
            .expect("invariant: spec/overlay/compat_4x.toml parses")
            .member
    })
}

/// The removed member `attr` of `srna` (or one of its bases), if the overlay lists it.
pub fn lookup(srna: &'static StructDef, attr: &str) -> Option<&'static Removed> {
    let mut s = Some(srna);
    while let Some(d) = s {
        if let Some(r) = table()
            .iter()
            .find(|r| r.srna == d.identifier && r.identifier == attr)
        {
            return Some(r);
        }
        s = d.base;
    }
    None
}

/// Blender's AttributeError for a removed member, with the replacement as hint.
pub fn error(owner: &str, r: &Removed) -> FrapError {
    FrapError::new(
        PyExc::AttributeError,
        codes::RNA_REMOVED_IN_5X,
        format!("'{owner}' object has no attribute '{}'", r.identifier),
    )
    .with_detail("removed_in", r.removed_in.as_str())
    .with_hint(r.hint.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_parses_and_resolves_through_bases() {
        assert!(table().len() >= 10);
        let r = lookup(&frap_spec_rna::ACTION, "fcurves").expect("listed");
        assert_eq!(r.removed_in, "5.0");
        assert!(r.hint.contains("channelbag"));
        // Sub-types inherit the entries of their base (`SunLight` is a `Light`).
        assert!(lookup(&frap_spec_rna::SUN_LIGHT, "shadow_buffer_bias").is_some());
        assert!(lookup(&frap_spec_rna::OBJECT, "location").is_none());
        let e = error("Action", r);
        assert_eq!(e.message, "'Action' object has no attribute 'fcurves'");
        assert_eq!(e.code, codes::RNA_REMOVED_IN_5X);
    }

    #[test]
    fn no_overlay_member_exists_in_the_5_2_dumps() {
        for r in table() {
            let s = frap_spec_rna::find_struct(&r.srna)
                .unwrap_or_else(|| panic!("{} is not a 5.2 struct", r.srna));
            match r.kind.as_str() {
                "prop" => assert!(s.find_prop(&r.identifier).is_none(), "{r:?}"),
                "func" => assert!(s.find_func(&r.identifier).is_none(), "{r:?}"),
                k => panic!("unknown kind {k}"),
            }
        }
    }
}
