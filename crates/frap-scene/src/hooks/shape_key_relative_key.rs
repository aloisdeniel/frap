// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShapeKey.relative_key` (`rna_ShapeKey_relative_key_get/set`, WP 6.3): the block this one
//! is relative to. Assigning a block of the same key stores its index; anything else (a block
//! of another key) keeps the current one (`rna_object_shapekey_index_set` returns `current`).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::id::Key;
use crate::id::paths::internal;
use crate::{Host, HostRead};

#[rna_hook(srna = "ShapeKey", prop = "relative_key", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let (k, i) = Key::block_of(h.main(), this)
            .ok_or_else(|| internal("ShapeKey.relative_key: not a shape key pointer"))?;
        let key = h
            .main()
            .shape_keys
            .get(k)
            .ok_or_else(|| internal("ShapeKey.relative_key: the key was removed"))?;
        let r = key.key_blocks[i].relative_key;
        Ok(Field::Value(match key.key_blocks.get(r) {
            Some(kb) => Value::Ptr(Box::new(Key::block_ptr(k, &kb.name))),
            None => Value::None,
        }))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let (k, i) = Key::block_of(h.main(), this)
            .ok_or_else(|| internal("ShapeKey.relative_key: not a shape key pointer"))?;
        let Value::Ptr(p) = v else {
            return Ok(());
        };
        let Some((k2, j)) = Key::block_of(h.main(), &p) else {
            return Ok(());
        };
        if k2 != k {
            return Ok(());
        }
        let (main, log) = h.main_mut();
        if let Some(key) = main.shape_keys.get_mut(k, log) {
            key.key_blocks[i].relative_key = j;
        }
        Ok(())
    }
}
