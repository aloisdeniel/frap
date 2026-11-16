// SPDX-License-Identifier: GPL-3.0-or-later
//! Derived material slots (§5.6). Blender keeps `len(ob.material_slots) ==
//! len(ob.data.materials)` for every object sharing the data (`BKE_object_materials_test`);
//! frap stores only the per-object overrides (`link`, object material) and pads them lazily, so
//! the slot count always follows `data.materials`. Overrides beyond the data's slot count are
//! kept (as Blender keeps `ob->mat` entries) but are not visible.
//!
//! Verified on 5.2.4 (RFC 0001): `slot.material = M` writes `mesh.materials[i]` when
//! `link == 'DATA'` and the object override when `'OBJECT'`; `material_slots['Name']` looks up
//! by material name but reprs as `material_slots[0]`; a `None` material gives `slot.name == ''`.

use frap_rna::traits::index_out_of_range;
use frap_rna::{
    Field, IdCode, IdRef, Item, MaterialId, MeshId, ObjectId, PropDef, Rna, RnaColl, RnaError, Seg,
    StructDef, StructRef, Value,
};
use smol_str::SmolStr;

use crate::data::{ChangeLog, Main};
use crate::id::{Object, SlotLink, SlotOverride};

/// The material list of an object's data (`data.materials`; empty for data without one).
pub fn data_materials<'m>(main: &'m Main, ob: &Object) -> &'m [Option<IdRef<MaterialId>>] {
    match ob.data {
        Some(d) if d.code == IdCode::ME => d
            .typed::<MeshId>()
            .and_then(|m| main.meshes.get(m))
            .map_or(&[], |me| me.materials.as_slice()),
        _ => &[],
    }
}

/// Number of material slots of an object (`len(data.materials)`).
pub fn slot_count(main: &Main, ob: &Object) -> usize {
    data_materials(main, ob).len()
}

/// The effective `(link, material)` of slot `i`.
pub fn slot(main: &Main, ob: &Object, i: usize) -> (SlotLink, Option<IdRef<MaterialId>>) {
    let ov = ob
        .material_slot_overrides
        .get(i)
        .cloned()
        .unwrap_or_default();
    match ov.link {
        SlotLink::Object => (SlotLink::Object, ov.material),
        SlotLink::Data => (
            SlotLink::Data,
            data_materials(main, ob).get(i).copied().flatten(),
        ),
    }
}

/// The material of every slot, in slot order (what `Object.active_material`, Workbench and the
/// glTF exporter use).
pub fn slot_materials(main: &Main, ob: &Object) -> Vec<Option<IdRef<MaterialId>>> {
    (0..slot_count(main, ob))
        .map(|i| slot(main, ob, i).1)
        .collect()
}

/// The override of slot `i`, padded lazily (§5.6).
pub fn override_mut(ob: &mut Object, i: usize) -> &mut SlotOverride {
    if ob.material_slot_overrides.len() <= i {
        ob.material_slot_overrides
            .resize(i + 1, SlotOverride::default());
    }
    &mut ob.material_slot_overrides[i]
}

fn material_name(main: &Main, m: Option<IdRef<MaterialId>>) -> SmolStr {
    m.and_then(|m| main.materials.get(m))
        .map(|m| m.header.name.clone())
        .unwrap_or_default()
}

/// `Object.material_slots` (a proxy over the object and its data).
pub struct MaterialSlots<'h> {
    /// `bpy.data`
    pub main: &'h Main,
    /// The object.
    pub ob: &'h Object,
}

impl RnaColl for MaterialSlots<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::MATERIAL_SLOT
    }
    fn len(&self) -> usize {
        slot_count(self.main, self.ob)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        (i < self.len()).then(|| material_name(self.main, slot(self.main, self.ob, i).1))
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.len()).find(|&i| self.key_of(i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        if i >= self.len() {
            return Err(index_out_of_range(i, self.len()));
        }
        Ok(Item::Struct(StructRef::Owned(Box::new(MaterialSlotRef {
            main: self.main,
            ob: self.ob,
            i,
        }))))
    }
    /// Verified: `material_slots['Material']` reprs as `material_slots[0]`.
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

/// One `MaterialSlot` (read proxy; writes go through the `MaterialSlot.link` / `.material`
/// set hooks).
pub struct MaterialSlotRef<'h> {
    /// `bpy.data`
    pub main: &'h Main,
    /// The object.
    pub ob: &'h Object,
    /// Slot index.
    pub i: usize,
}

impl Rna for MaterialSlotRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::MATERIAL_SLOT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let (link, m) = slot(self.main, self.ob, self.i);
        Ok(match prop.identifier {
            "link" => Field::Value(Value::Enum(
                match link {
                    SlotLink::Object => "OBJECT",
                    SlotLink::Data => "DATA",
                }
                .into(),
            )),
            "material" => Field::Id(m.map(IdRef::any)),
            "slot_index" => Field::Value(Value::Int(self.i as i64)),
            "name" => Field::Value(Value::Str(material_name(self.main, m))),
            _ => return Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        })
    }
}

/// `material_slots[i].material = m` (and `Object.active_material`): a `DATA` slot writes
/// the data's material list (`mesh.materials[i]`, so every user of the mesh sees it), an
/// `OBJECT` slot the object's override. Out-of-range slots are ignored.
pub fn set_slot_material(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    i: usize,
    m: Option<IdRef<MaterialId>>,
    log: &mut ChangeLog,
) {
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    if i >= slot_count(main, o) {
        return;
    }
    let link = o
        .material_slot_overrides
        .get(i)
        .map_or(SlotLink::Data, |s| s.link);
    match (link, o.data.and_then(|d| d.typed::<MeshId>())) {
        (SlotLink::Object, _) => {
            if let Some(o) = main.objects.get_mut(ob, log) {
                override_mut(o, i).material = m;
            }
        }
        (SlotLink::Data, Some(me)) => {
            if let Some(me) = main.meshes.get_mut(me, log)
                && let Some(slot) = me.materials.get_mut(i)
            {
                *slot = m;
            }
        }
        (SlotLink::Data, None) => {}
    }
}

/// `material_slots[i].link = 'OBJECT' | 'DATA'` (the override is padded lazily).
pub fn set_slot_link(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    i: usize,
    link: SlotLink,
    log: &mut ChangeLog,
) {
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    if i >= slot_count(main, o) {
        return;
    }
    if let Some(o) = main.objects.get_mut(ob, log) {
        override_mut(o, i).link = link;
    }
}

/// `MaterialSlot.link` / `.material` are set through hooks (the slot is a proxy).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static SLOT_LINK_DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "MaterialSlot",
    prop: "link",
    coll: false,
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static SLOT_MATERIAL_DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "MaterialSlot",
    prop: "material",
    coll: false,
};

/// Slot index of a `MaterialSlot` pointer (`…material_slots[i]`).
pub(crate) fn slot_index(this: &frap_rna::RnaPtr) -> Result<usize, RnaError> {
    match this.path.as_slice() {
        [.., Seg::Prop("material_slots"), Seg::Index(i)] if *i >= 0 => Ok(*i as usize),
        _ => Err(crate::id::paths::internal(
            "not a material slot pointer (material_slots[i])",
        )),
    }
}
