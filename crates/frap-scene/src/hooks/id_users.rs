// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.users` (§5.5): computed on demand from the user-counting references of every ID, plus
//! one for `use_fake_user`, plus the window's user of its scene (verified on 5.2.4 factory:
//! every datablock has 1 user; an unlinked `objects.new` object has 0; a fresh mesh 0, 1 once
//! an object uses it, 2 with a fake user). Embedded IDs report 0 (they are owned, not used).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Root, Value, rna_hook};

use crate::HostRead;

#[rna_hook(srna = "ID", prop = "users", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let n = match this.root {
        Root::Id(a) => {
            let window = u32::from(h.win().scene.any() == a);
            h.main().users(a) + window
        }
        _ => 0,
    };
    Ok(Field::Value(Value::Int(i64::from(n))))
}
