// SPDX-License-Identifier: GPL-3.0-or-later
//! Derived `MaterialSlot` (§5.6): slot count = `len(ob.data.materials)`, overrides padded lazily.
//! `Object.material_slots` is `=> hook_coll`; `MaterialSlot.link` / `.material` are set hooks.
//!
//! Behaviour verified on Blender 5.2.4 (RFC 0001 appendix): `slot.material = M` with
//! `link == 'DATA'` writes `mesh.materials[i]` (another ID!); with `'OBJECT'` it writes the
//! object override; `material_slots['Name']` looks up by material name but reprs as
//! `material_slots[0]`; a `None` material gives `slot.name == ''`.

use frap_probe_rna::*;
use frap_probe_spec as spec;
use smol_str::SmolStr;

use crate::{Host, HostRead, Main, Object, RNA_HOOKS, RnaHookReg, SlotLink, SlotOverride};

fn data_materials<'h>(main: &'h Main, ob: &Object) -> &'h [Option<IdRef<MaterialId>>] {
    match ob.data.and_then(IdRef::<MeshId>::from_any).and_then(|m| main.meshes.get(m)) {
        Some(me) => &me.materials,
        None => &[],
    }
}

fn effective(main: &Main, ob: &Object, i: usize) -> (SlotLink, Option<IdRef<MaterialId>>) {
    let ov = ob.material_slot_overrides.get(i).copied().unwrap_or_default();
    match ov.link {
        SlotLink::Object => (SlotLink::Object, ov.material),
        SlotLink::Data => (SlotLink::Data, data_materials(main, ob).get(i).copied().flatten()),
    }
}

pub struct MaterialSlots<'h> {
    pub main: &'h Main,
    pub ob: &'h Object,
}

impl RnaColl for MaterialSlots<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::MATERIAL_SLOT
    }
    fn len(&self) -> usize {
        data_materials(self.main, self.ob).len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        let (_, m) = effective(self.main, self.ob, i);
        Some(
            m.and_then(|m| self.main.materials.get(m))
                .map(|m| m.header.name.clone())
                .unwrap_or_default(),
        )
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Owned(Box::new(MaterialSlotRef {
            main: self.main,
            ob: self.ob,
            i,
        })))
    }
    /// Verified: `material_slots['Material']` reprs as `material_slots[0]`.
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

pub struct MaterialSlotRef<'h> {
    pub main: &'h Main,
    pub ob: &'h Object,
    pub i: usize,
}

impl Rna for MaterialSlotRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &spec::MATERIAL_SLOT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let (link, m) = effective(self.main, self.ob, self.i);
        match prop.identifier {
            "link" => Ok(Field::Value(Value::Enum(
                if link == SlotLink::Object { "OBJECT" } else { "DATA" }.into(),
            ))),
            "material" => Ok(Field::Id(m.map(IdRef::any))),
            "slot_index" => Ok(Field::Value(Value::Int(self.i as i64))),
            "name" => Ok(Field::Value(Value::Str(
                m.and_then(|m| self.main.materials.get(m))
                    .map(|m| m.header.name.clone())
                    .unwrap_or_default(),
            ))),
            id => Err(unbound(self.srna(), id)),
        }
    }
}

fn this_object(this: &RnaPtr) -> Result<IdRef<ObjectId>, RnaError> {
    match this.root {
        Root::Id(a) => IdRef::from_any(a).ok_or_else(|| RnaError::new(PyExc::RuntimeError, codes::INTERNAL, "hook: root is not an Object")),
        _ => Err(RnaError::new(PyExc::RuntimeError, codes::INTERNAL, "hook: root is not an ID")),
    }
}

fn slot_index(this: &RnaPtr) -> Result<usize, RnaError> {
    match this.path.as_slice() {
        [Seg::Prop("material_slots"), Seg::Index(i)] => Ok(*i as usize),
        _ => Err(RnaError::new(
            PyExc::RuntimeError,
            codes::INTERNAL,
            "hook: expected canonical material_slots[i]",
        )),
    }
}

fn get_material_slots<'h>(host: &'h dyn HostRead, this: &RnaPtr, _prop: &'static PropDef) -> Result<Field<'h>, RnaError> {
    let main = host.main();
    let ob = main
        .objects
        .get(this_object(this)?)
        .ok_or_else(|| RnaError::new(PyExc::RuntimeError, codes::INTERNAL, "dangling"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(MaterialSlots { main, ob }))))
}

fn grow(ob: &mut Object, i: usize) -> &mut SlotOverride {
    if ob.material_slot_overrides.len() <= i {
        ob.material_slot_overrides.resize(i + 1, SlotOverride::default());
    }
    &mut ob.material_slot_overrides[i]
}

fn set_link(host: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (ob_ref, i) = (this_object(this)?, slot_index(this)?);
    let link = match v {
        Value::Enum(s) if s == "OBJECT" => SlotLink::Object,
        _ => SlotLink::Data,
    };
    let (main, log) = host.main_mut();
    let ob = main.objects.get_mut(ob_ref, log).expect("invariant: resolved object");
    grow(ob, i).link = link;
    Ok(())
}

fn set_material(host: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (ob_ref, i) = (this_object(this)?, slot_index(this)?);
    let mat = match v {
        Value::Ptr(RnaPtr { root: Root::Id(a), .. }) => IdRef::<MaterialId>::from_any(a),
        _ => None,
    };
    let (main, log) = host.main_mut();
    let ob = main.objects.get(ob_ref).expect("invariant: resolved object");
    let link = ob.material_slot_overrides.get(i).map(|o| o.link).unwrap_or_default();
    match link {
        SlotLink::Object => grow(main.objects.get_mut(ob_ref, log).expect("invariant"), i).material = mat,
        SlotLink::Data => {
            // Writes ANOTHER ID than the one the path is rooted at.
            let me = ob.data.and_then(IdRef::<MeshId>::from_any).expect("probe: mesh objects only");
            main.meshes.get_mut(me, log).expect("invariant").materials[i] = mat;
        }
    }
    Ok(())
}

#[allow(unsafe_code)] // RFC 0001 F-LINK-1: linkme entries use #[link_section]
#[linkme::distributed_slice(RNA_HOOKS)]
static OBJECT_MATERIAL_SLOTS: RnaHookReg = RnaHookReg {
    srna: "Object",
    prop: "material_slots",
    get: Some(get_material_slots),
    set: None,
};
#[allow(unsafe_code)] // RFC 0001 F-LINK-1: linkme entries use #[link_section]
#[linkme::distributed_slice(RNA_HOOKS)]
static MATERIAL_SLOT_LINK: RnaHookReg = RnaHookReg {
    srna: "MaterialSlot",
    prop: "link",
    get: None,
    set: Some(set_link),
};
#[allow(unsafe_code)] // RFC 0001 F-LINK-1: linkme entries use #[link_section]
#[linkme::distributed_slice(RNA_HOOKS)]
static MATERIAL_SLOT_MATERIAL: RnaHookReg = RnaHookReg {
    srna: "MaterialSlot",
    prop: "material",
    get: None,
    set: Some(set_material),
};
