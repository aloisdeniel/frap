// SPDX-License-Identifier: GPL-3.0-or-later
//! The type registry over [`crate::MODIFIER_TYPES`] (WP 4.1 owns it after W0): lookup by
//! type, the per-type `--describe` support list, and the duplicate-registration check.

use crate::{MODIFIER_TYPES, ModifierType, ModifierTypeInfo};

/// The evaluator registered for `ty`, if any.
pub fn find(ty: ModifierType) -> Option<&'static ModifierTypeInfo> {
    MODIFIER_TYPES.iter().find(|i| i.ty == ty)
}

/// Types registered more than once (must be empty; the registry test checks it).
pub fn duplicates() -> Vec<ModifierType> {
    let mut seen = Vec::new();
    let mut dup = Vec::new();
    for i in MODIFIER_TYPES.iter() {
        if seen.contains(&i.ty) {
            if !dup.contains(&i.ty) {
                dup.push(i.ty);
            }
        } else {
            seen.push(i.ty);
        }
    }
    dup
}

/// Every type with an evaluator, in `ModifierType` order (`--describe`).
pub fn evaluated_types() -> Vec<ModifierType> {
    let mut v: Vec<ModifierType> = MODIFIER_TYPES.iter().map(|i| i.ty).collect();
    v.sort();
    v.dedup();
    v
}
