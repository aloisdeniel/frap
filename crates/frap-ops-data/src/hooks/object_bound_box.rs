// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.bound_box` (`rna_Object_boundbox_get`): the eight corners of the evaluated local
//! bounds in `BKE_boundbox_init_from_minmax` order, always fresh (D-04). Verified on 5.2.4:
//! the factory cube reads `(-1,-1,-1), (-1,-1,1), (-1,1,1), (-1,1,-1), (1,-1,-1), (1,-1,1),
//! (1,1,1), (1,1,-1)`; an empty or a camera reads eight zero corners.

use frap_rna::traits::ToRnaValue;
use frap_rna::{Field, MathTag, PropDef, RnaError, RnaPtr, rna_hook};
use frap_scene::HostRead;

use super::_common::{evaluated_read, object_of};

#[rna_hook(srna = "Object", prop = "bound_box")]
fn get<'h>(h: &'h dyn HostRead, this: &RnaPtr, prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let ob = object_of(this)?;
    let eo = evaluated_read(h, ob)?;
    let corners = frap_eval::bound_box(&eo);
    let flat: [f32; 24] = std::array::from_fn(|i| corners[i / 3][i % 3]);
    Ok(Field::Value(flat.to_rna_value(prop, MathTag::None)))
}
