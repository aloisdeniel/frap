// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.data` (`rna_Object_data_set` / `rna_Object_data_poll`, verified on 5.2.4):
//! the new data must be of the object's type (`TypeError: … Object.data expected a Mesh type,
//! not Camera`; an empty's data type is `Image`), `None` is ignored, and an object never
//! changes type. Material slots follow the new data (they are derived, §5.6).

use frap_rna::{Field, IdCode, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook};

use crate::id::paths::{internal, object_of};
use crate::{Host, HostRead};

/// The RNA struct name of an object type's data (`BKE_object_obdata_to_type` inverse).
fn data_struct(code: Option<IdCode>) -> &'static str {
    match code {
        Some(IdCode::ME) => "Mesh",
        Some(IdCode::CA) => "Camera",
        Some(IdCode::LA) => "Light",
        Some(IdCode::AR) => "Armature",
        Some(IdCode::PT) => "PointCloud",
        _ => "Image",
    }
}

fn struct_of(code: IdCode) -> &'static str {
    match code {
        IdCode::OB => "Object",
        IdCode::ME => "Mesh",
        IdCode::MA => "Material",
        IdCode::CA => "Camera",
        IdCode::LA => "Light",
        IdCode::WO => "World",
        IdCode::IM => "Image",
        IdCode::AR => "Armature",
        IdCode::AC => "Action",
        IdCode::SC => "Scene",
        IdCode::GR => "Collection",
        IdCode::KE => "Key",
        IdCode::NT => "NodeTree",
        IdCode::TE => "Texture",
        IdCode::LI => "Library",
        IdCode::PT => "PointCloud",
        _ => "ID",
    }
}

#[rna_hook(srna = "Object", prop = "data", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let ob = object_of(this)?;
        let o = h
            .main()
            .objects
            .get(ob)
            .ok_or_else(|| internal("Object.data: the object was removed"))?;
        Ok(Field::Id(o.data))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Value::Id(Some(new)) = v else {
            return Ok(()); // `ob.data = None` is ignored (verified)
        };
        let ob = object_of(this)?;
        let (main, log) = h.main_mut();
        let cur = main
            .objects
            .get(ob)
            .ok_or_else(|| internal("Object.data: the object was removed"))?
            .data
            .map(|d| d.code);
        if cur != Some(new.code) {
            return Err(RnaError::new(
                RnaErrorKind::Type,
                PyExc::TypeError,
                format!(
                    "Object.data expected a {} type, not {}",
                    data_struct(cur),
                    struct_of(new.code)
                ),
            ));
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.data = Some(new);
        }
        Ok(())
    }
}
