// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.dimensions` (`rna_Object_dimensions_get/set`, `BKE_object_dimensions_get/set`).
//!
//! Get: the evaluated local bounds scaled by the lengths of `matrix_world`'s axes, always
//! fresh (D-04). Objects without geometry (empty, camera, light) read `(0, 0, 0)`.
//! Set: per axis `value / extent` with the sign of the current scale; an axis with zero extent
//! keeps its scale (`BKE_object_dimensions_set` with no axis mask).
//!
//! Verified on 5.2.4: a cube scaled `(2, -1, 3)` and rotated reads `(4, 2, 6)`; writing
//! `(4, 5, 0)` gives scale `(2, -2.5, 0)`; a parent's scale is included.

use frap_rna::traits::FromRnaValue;
use frap_rna::traits::ToRnaValue;
use frap_rna::{MathTag, PropDef, RnaError, RnaPtr, Value, rna_hook};
use frap_scene::{EvalMode, Host, HostRead};

use super::_common::{evaluated_read, internal, object_of};

#[rna_hook(srna = "Object", prop = "dimensions")]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        prop: &'static PropDef,
    ) -> Result<frap_rna::Field<'h>, RnaError> {
        let ob = object_of(this)?;
        let eo = evaluated_read(h, ob)?;
        Ok(frap_rna::Field::Value(
            frap_eval::dimensions(&eo).to_rna_value(prop, MathTag::Vector),
        ))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let value: [f32; 3] = FromRnaValue::from_rna_value(prop, v)?;
        let (host, ev) = h.eval();
        let eo = ev
            .object(host, ob, EvalMode::Viewport)
            .map_err(|e| internal(e.message))?;
        let (main, log) = h.main_mut();
        let Some(o) = main.objects.get_mut(ob, log) else {
            return Err(internal("Object.dimensions: the object was removed"));
        };
        let scale = frap_eval::scale_for_dimensions(&eo, value, o.scale);
        o.scale = scale;
        Ok(())
    }
}
