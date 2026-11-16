// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.children_recursive` (Python-only): every descendant, depth first, each object
//! followed by its own descendants, siblings in `bpy.data.objects` order (verified:
//! `['A_empty', 'c_empty', 'b_empty', 'Camera', 'Light']`, a `list`). frap returns a tuple value:
//! `Value` has no list variant, and both encode as a JSON array.

use frap_rna::{Field, IdRef, ObjectId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::HostRead;
use crate::hooks::object_children::children;
use crate::id::paths::object_of;
use crate::id::transform::MAX_PARENT_DEPTH;

#[rna_hook(srna = "Object", prop = "children_recursive", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    fn walk(
        objects: &crate::data::IdMap<crate::id::Object>,
        ob: IdRef<ObjectId>,
        depth: usize,
        out: &mut Vec<IdRef<ObjectId>>,
    ) {
        if depth > MAX_PARENT_DEPTH {
            return;
        }
        for c in children(objects, ob) {
            if out.contains(&c) {
                continue;
            }
            out.push(c);
            walk(objects, c, depth + 1, out);
        }
    }
    let ob = object_of(this)?;
    let mut out = Vec::new();
    walk(&h.main().objects, ob, 0, &mut out);
    Ok(Field::Value(Value::Tuple(
        out.into_iter().map(Value::from).collect(),
    )))
}
