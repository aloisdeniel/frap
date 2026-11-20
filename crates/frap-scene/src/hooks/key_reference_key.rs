// SPDX-License-Identifier: GPL-3.0-or-later
//! `Key.reference_key` (WP 6.3): the basis, always the first block (`BKE_key_sort`'s "first
//! key is refkey" rule); `None` for a key without blocks.

use frap_rna::{Field, IdRef, KeyId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;
use crate::id::Key;
use crate::id::paths::{internal, root_id};

#[rna_hook(srna = "Key", prop = "reference_key", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let k: IdRef<KeyId> = root_id(this)?
        .typed()
        .ok_or_else(|| internal("Key.reference_key: not a key"))?;
    let key = h
        .main()
        .shape_keys
        .get(k)
        .ok_or_else(|| internal("Key.reference_key: the key was removed"))?;
    Ok(Field::Value(match key.key_blocks.first() {
        Some(kb) => Value::Ptr(Box::new(Key::block_ptr(k, &kb.name))),
        None => Value::None,
    }))
}
