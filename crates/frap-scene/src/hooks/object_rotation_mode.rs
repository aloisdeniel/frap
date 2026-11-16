// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.rotation_mode` assignment (`rna_Object_rotation_mode_set` →
//! `BKE_rotMode_change_values`, verified on 5.2.4): the stored rotation is converted into the
//! channel of the new mode so the orientation is preserved. Euler → Euler of another order
//! converts nothing; leaving `QUATERNION` normalises the stored quaternion in place.

use frap_math::matrix::RotationMode;
use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::object_of;

#[rna_hook(srna = "Object", prop = "rotation_mode", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let Some(mode) = v.as_str().and_then(RotationMode::from_blender) else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        let mut t = o.transform();
        t.set_rotation_mode(mode);
        o.set_transform(&t);
    }
    Ok(())
}
