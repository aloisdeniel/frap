// SPDX-License-Identifier: GPL-3.0-or-later
//! `CorrectiveSmoothModifier.is_bind` (WP 8.3.12): whether bind coordinates are stored
//! (`rna_CorrectiveSmoothModifier_is_bind_get`: `bind_coords != nullptr`). frap stores no
//! bind data (`object.correctivesmooth_bind` is OUT, ARCHITECTURE §22.10), so it is `False`.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_modifier.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;

/// Declares a `=> hook` binding (the registry test pairs every `RNA_HOOKS` entry with one).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static IS_BIND_DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "CorrectiveSmoothModifier",
    prop: "is_bind",
    coll: false,
};

#[rna_hook(srna = "CorrectiveSmoothModifier", prop = "is_bind", krate = crate)]
fn get<'h>(
    _h: &'h dyn HostRead,
    _this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    Ok(Field::Value(Value::Bool(false)))
}
