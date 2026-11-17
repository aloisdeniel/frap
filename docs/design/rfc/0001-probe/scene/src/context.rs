// SPDX-License-Identifier: GPL-3.0-or-later
//! §8 Context, selection and modes (`frap-scene/src/context.rs`, CONTRACT).

use std::collections::BTreeMap;
use std::sync::Arc;

use frap_probe_rna::*;
use frap_probe_spec as spec;
use smallvec::smallvec;
use smol_str::SmolStr;

use crate::{Main, Object, ObjectMode, Scene, ToolSettings, ViewLayer};

#[derive(Clone, Debug)]
pub struct WindowState {
    pub scene: IdRef<SceneId>,
    pub view_layer: SmolStr,
}

/// Stand-in for `frap_bmesh::BMesh` (L5) — only what the `split_edit` test needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BMesh {
    pub verts: Vec<[f32; 3]>,
    pub select_mode: [bool; 3],
}

#[derive(Clone, Debug)]
pub struct EditMesh {
    pub bm: Arc<BMesh>,
    pub bm_gen: u64,
}

#[derive(Clone, Debug, Default)]
pub struct EditState {
    pub meshes: BTreeMap<IdKey, EditMesh>,
}

#[derive(Clone, Debug, Default)]
pub struct Preferences {
    pub use_duplicate_mesh: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContextMode {
    Object,
    EditMesh,
    EditArmature,
    Pose,
}

impl ContextMode {
    pub fn identifier(self) -> &'static str {
        match self {
            ContextMode::Object => "OBJECT",
            ContextMode::EditMesh => "EDIT_MESH",
            ContextMode::EditArmature => "EDIT_ARMATURE",
            ContextMode::Pose => "POSE",
        }
    }
}

/// No EditState borrow: the mode is on Object.mode (§8).
#[derive(Copy, Clone)]
pub struct Context<'a> {
    pub main: &'a Main,
    pub win: &'a WindowState,
    pub prefs: &'a Preferences,
}

impl<'a> Context<'a> {
    pub fn scene(&self) -> &'a Scene {
        self.main.scenes.get(self.win.scene).expect("invariant: window scene exists")
    }
    pub fn view_layer(&self) -> &'a ViewLayer {
        let s = self.scene();
        s.view_layers
            .iter()
            .find(|v| v.name == self.win.view_layer)
            .unwrap_or(&s.view_layers[0])
    }
    pub fn active_object(&self) -> Option<IdRef<ObjectId>> {
        self.view_layer().active
    }
    pub fn object(&self) -> Option<IdRef<ObjectId>> {
        self.active_object()
    }
    pub fn selected_objects(&self) -> Vec<IdRef<ObjectId>> {
        self.view_layer().bases.iter().filter(|b| b.selected).map(|b| b.object).collect()
    }
    pub fn visible_objects(&self) -> Vec<IdRef<ObjectId>> {
        self.view_layer().bases.iter().map(|b| b.object).collect()
    }
    fn active(&self) -> Option<&'a Object> {
        self.active_object().and_then(|r| self.main.objects.get(r))
    }
    pub fn mode(&self) -> ContextMode {
        match self.active().map(|o| (o.mode, o.object_type())) {
            Some((ObjectMode::Edit, "MESH")) => ContextMode::EditMesh,
            Some((ObjectMode::Edit, "ARMATURE")) => ContextMode::EditArmature,
            Some((ObjectMode::Pose, _)) => ContextMode::Pose,
            _ => ContextMode::Object,
        }
    }
    pub fn edit_object(&self) -> Option<IdRef<ObjectId>> {
        self.active().filter(|o| o.mode == ObjectMode::Edit).and(self.active_object())
    }
    pub fn objects_in_mode(&self) -> Vec<IdRef<ObjectId>> {
        let Some(mode) = self.active().map(|o| o.mode) else { return vec![] };
        if mode == ObjectMode::Object {
            return vec![];
        }
        self.view_layer()
            .bases
            .iter()
            .map(|b| b.object)
            .filter(|r| self.main.objects.get(*r).is_some_and(|o| o.mode == mode))
            .collect()
    }
    pub fn tool_settings(&self) -> &'a ToolSettings {
        &self.scene().tool_settings
    }
    pub fn cursor(&self) -> [f32; 3] {
        self.scene().cursor_location
    }
}

/// `bpy.context.selected_objects` etc.: a Python list of IDs. Items are IDs (F-TR-4).
pub struct IdList {
    pub ids: Vec<IdAny>,
    pub item_type: &'static StructDef,
}
impl RnaColl for IdList {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, _i: usize) -> Result<StructRef<'_>, RnaError> {
        Err(RnaError::new(
            PyExc::RuntimeError,
            codes::INTERNAL,
            "contract gap: an ID item is not a StructRef (RFC 0001 F-TR-4)",
        ))
    }
    fn item_any(&self, i: usize) -> Result<Item<'_>, RnaError> {
        Ok(Item::Id(Some(self.ids[i])))
    }
    fn item_seg(&self, i: usize) -> Seg {
        Seg::Index(i as i64)
    }
}

fn ids(v: Vec<IdRef<ObjectId>>) -> Field<'static> {
    Field::Coll(CollRef::Owned(Box::new(IdList {
        ids: v.into_iter().map(IdRef::any).collect(),
        item_type: &spec::OBJECT,
    })))
}

impl Rna for Context<'_> {
    fn srna(&self) -> &'static StructDef {
        &spec::CONTEXT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let scene = self.win.scene.any();
        match prop.identifier {
            "scene" => Ok(Field::Id(Some(scene))),
            "mode" => Ok(Field::Value(Value::Enum(self.mode().identifier().into()))),
            "object" | "active_object" => Ok(Field::Id(self.active_object().map(IdRef::any))),
            "edit_object" => Ok(Field::Id(self.edit_object().map(IdRef::any))),
            "selected_objects" | "selected_editable_objects" => Ok(ids(self.selected_objects())),
            "visible_objects" => Ok(ids(self.visible_objects())),
            "objects_in_mode" => Ok(ids(self.objects_in_mode())),
            // Pointers *into* an ID: Blender's repr is the canonical one
            // (`bpy.data.scenes['Scene'].tool_settings`, verified) -> F-TR-2.
            "tool_settings" => Ok(Field::Ptr(RnaPtr {
                root: Root::Id(scene),
                path: smallvec![Seg::Prop("tool_settings")],
                srna: &spec::TOOL_SETTINGS,
            })),
            "view_layer" => Ok(Field::Ptr(RnaPtr {
                root: Root::Id(scene),
                path: smallvec![Seg::Prop("view_layers"), Seg::Key(self.view_layer().name.clone())],
                srna: &spec::VIEW_LAYER,
            })),
            id => Err(unbound(self.srna(), id)),
        }
    }
}
