// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.matrix_world`. Reading is always fresh (deviation D-04): the parent chain composed
//! with `matrix_parent_inverse` and `matrix_basis`, bone parents (`parent_type = 'BONE'`) with
//! the posed bone of the original pose channels (WP 6.5, `world_matrix_in`). Writing decomposes the matrix back into
//! location, rotation (in the current `rotation_mode`, deltas removed) and scale relative to
//! the parent (`BKE_object_apply_mat4(ob, m, false, true)`, verified on 5.2.4 for every
//! rotation mode, with a parent, with delta transforms, negative scale and shear).

use frap_rna::traits::ToRnaValue;
use frap_rna::{Field, MathTag, PropDef, RnaError, RnaPtr, Value, rna_hook};
use glam::Mat4;

use crate::id::paths::{internal, object_of};
use crate::id::transform::{parent_world_matrix_in, world_matrix_in};
use crate::{Host, HostRead};

#[rna_hook(srna = "Object", prop = "matrix_world", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let ob = object_of(this)?;
        if h.main().objects.get(ob).is_none() {
            return Err(internal("Object.matrix_world: the object was removed"));
        }
        let m = world_matrix_in(h.main(), ob);
        Ok(Field::Value(m.to_rna_value(prop, MathTag::Matrix)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let m: Mat4 = frap_rna::traits::FromRnaValue::from_rna_value(prop, v)?;
        let (main, log) = h.main_mut();
        let pw = parent_world_matrix_in(main, ob);
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.set_matrix_world(&m, pw.as_ref());
        }
        Ok(())
    }
}
