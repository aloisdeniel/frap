// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object` subset (§5.6) and its binding (§9.6 step 4, hand-written stand-in for `rna_struct!`).

use frap_probe_mesh::rna::{id_get, id_set};
use frap_probe_rna::*;
use frap_probe_spec as spec;
use smol_str::SmolStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ObjectMode {
    Object,
    Edit,
    Pose,
}

impl ObjectMode {
    pub fn identifier(self) -> &'static str {
        match self {
            ObjectMode::Object => "OBJECT",
            ObjectMode::Edit => "EDIT",
            ObjectMode::Pose => "POSE",
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum SlotLink {
    #[default]
    Data,
    Object,
}

/// Per-object material slot override (`ob->matbits` + `ob->mat`), padded lazily (§5.6).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct SlotOverride {
    pub link: SlotLink,
    pub material: Option<IdRef<MaterialId>>,
}

#[derive(Clone, Debug)]
pub struct Object {
    pub header: IdHeader,
    pub data: Option<IdAny>,
    pub location: [f32; 3],
    pub rotation_euler: [f32; 3],
    pub scale: [f32; 3],
    pub mode: ObjectMode,
    pub material_slot_overrides: Vec<SlotOverride>,
    pub active_material_index: i32,
}

impl Object {
    pub fn new(name: &str, data: Option<IdAny>) -> Object {
        Object {
            header: IdHeader::named(name),
            data,
            location: [0.0; 3],
            rotation_euler: [0.0; 3],
            scale: [1.0; 3],
            mode: ObjectMode::Object,
            material_slot_overrides: vec![],
            active_material_index: 0,
        }
    }
    pub fn object_type(&self) -> &'static str {
        match self.data.map(|d| d.code) {
            None => "EMPTY",
            Some(IdCode::ME) => "MESH",
            Some(IdCode::CA) => "CAMERA",
            Some(IdCode::LA) => "LIGHT",
            Some(IdCode::AR) => "ARMATURE",
            Some(_) => "UNKNOWN",
        }
    }
}

impl IdType for Object {
    type Kind = ObjectId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(d) = self.data {
            f(d);
        }
        for o in &self.material_slot_overrides {
            if let Some(m) = o.material {
                f(m.any());
            }
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        if self.data == Some(from) {
            self.data = to;
        }
    }
}

fn arr3(v: [f32; 3], prop: &'static PropDef) -> Field<'static> {
    Field::Value(Value::float3(v, math_tag(prop)))
}

impl Rna for Object {
    fn srna(&self) -> &'static StructDef {
        &spec::OBJECT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "location" => Ok(arr3(self.location, prop)),
            "rotation_euler" => Ok(arr3(self.rotation_euler, prop)),
            "scale" => Ok(arr3(self.scale, prop)),
            "data" => Ok(Field::Id(self.data)),
            "type" => Ok(Field::Value(Value::Enum(SmolStr::from(self.object_type())))),
            "mode" => Ok(Field::Value(Value::Enum(SmolStr::from(self.mode.identifier())))),
            "active_material_index" => Ok(Field::Value(Value::Int(self.active_material_index as i64))),
            // `material_slots => hook_coll` (§9.6): derived from data.materials + overrides.
            "material_slots" => Ok(Field::NeedsHook),
            _ => id_get(self.srna(), &self.header, prop),
        }
    }
}

impl RnaMut for Object {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "location" => {
                self.location = f32_array::<3>(&v)?;
                Ok(SetOutcome::Done)
            }
            "rotation_euler" => {
                self.rotation_euler = f32_array::<3>(&v)?;
                Ok(SetOutcome::Done)
            }
            "scale" => {
                self.scale = f32_array::<3>(&v)?;
                Ok(SetOutcome::Done)
            }
            "active_material_index" => match v {
                Value::Int(i) => {
                    self.active_material_index = i as i32;
                    Ok(SetOutcome::Done)
                }
                _ => Err(bad_value("active_material_index")),
            },
            _ => {
                let srna = self.srna();
                id_set(srna, &mut self.header, prop, v)
            }
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        // material_slots: not reachable mutably from the Object alone (writes may go to the Mesh).
        Err(unbound(self.srna(), prop.identifier))
    }
}
