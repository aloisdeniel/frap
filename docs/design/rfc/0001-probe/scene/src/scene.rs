// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene`, `ViewLayer`, `Base`, `ToolSettings` subset (§8).

use frap_probe_mesh::rna::{id_get, id_set};
use frap_probe_rna::*;
use frap_probe_spec as spec;
use smol_str::SmolStr;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Base {
    pub object: IdRef<ObjectId>,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct ViewLayer {
    pub name: SmolStr,
    pub bases: Vec<Base>,
    pub active: Option<IdRef<ObjectId>>,
}

#[derive(Clone, Debug)]
pub struct ToolSettings {
    pub mesh_select_mode: [bool; 3],
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub header: IdHeader,
    pub view_layers: Vec<ViewLayer>,
    pub tool_settings: ToolSettings,
    pub cursor_location: [f32; 3],
    pub frame_current: i32,
    /// Probe stand-in for `scene.world` (a constant background colour for the render test).
    pub world_color: [f32; 3],
}

impl Scene {
    pub fn new(name: &str) -> Scene {
        Scene {
            header: IdHeader::named(name),
            view_layers: vec![ViewLayer {
                name: "ViewLayer".into(),
                bases: vec![],
                active: None,
            }],
            tool_settings: ToolSettings {
                mesh_select_mode: [true, false, false],
            },
            cursor_location: [0.0; 3],
            frame_current: 1,
            world_color: [0.05, 0.05, 0.05],
        }
    }
}

impl IdType for Scene {
    type Kind = SceneId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        for vl in &self.view_layers {
            for b in &vl.bases {
                f(b.object.any());
            }
        }
    }
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
}

impl Rna for Scene {
    fn srna(&self) -> &'static StructDef {
        &spec::SCENE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "frame_current" => Ok(Field::Value(Value::Int(self.frame_current as i64))),
            "tool_settings" => Ok(Field::Struct(StructRef::Borrowed(&self.tool_settings))),
            "view_layers" => Ok(Field::Coll(CollRef::Owned(Box::new(ViewLayers(&self.view_layers))))),
            _ => id_get(self.srna(), &self.header, prop),
        }
    }
}
impl RnaMut for Scene {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            // §7.3: the frame_current setter only marks time dirty -> hook.
            "frame_current" => Ok(SetOutcome::NeedsHook),
            _ => {
                let srna = self.srna();
                id_set(srna, &mut self.header, prop, v)
            }
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        match prop.identifier {
            "tool_settings" => Ok(Box::new(&mut self.tool_settings)),
            id => Err(unbound(self.srna(), id)),
        }
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
}

pub struct ViewLayers<'a>(pub &'a [ViewLayer]);
impl RnaColl for ViewLayers<'_> {
    fn item_type(&self) -> &'static StructDef {
        &spec::VIEW_LAYER
    }
    fn len(&self) -> usize {
        self.0.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        Some(self.0[i].name.clone())
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.0.iter().position(|v| v.name == key)
    }
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError> {
        Ok(StructRef::Borrowed(&self.0[i]))
    }
}

impl Rna for ViewLayer {
    fn srna(&self) -> &'static StructDef {
        &spec::VIEW_LAYER
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(self.name.clone()))),
            id => Err(unbound(self.srna(), id)),
        }
    }
}

impl Rna for ToolSettings {
    fn srna(&self) -> &'static StructDef {
        &spec::TOOL_SETTINGS
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "mesh_select_mode" => Ok(Field::Value(Value::BoolArray(self.mesh_select_mode.iter().copied().collect()))),
            id => Err(unbound(self.srna(), id)),
        }
    }
}
impl RnaMut for ToolSettings {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match (prop.identifier, v) {
            ("mesh_select_mode", Value::BoolArray(b)) if b.len() == 3 => {
                self.mesh_select_mode = [b[0], b[1], b[2]];
                Ok(SetOutcome::Done)
            }
            (id, _) => Err(unbound(self.srna(), id)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(unbound(self.srna(), prop.identifier))
    }
}
