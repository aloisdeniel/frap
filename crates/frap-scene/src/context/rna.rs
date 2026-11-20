// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA of the context side of the scene (§8, §9.4): `bpy.context` (a computed struct), the
//! `Window` stub, `ViewLayer` and `LayerCollection`.
//!
//! Context members that point *into* a datablock re-root at their canonical pointer (F-TR-2,
//! verified reprs): `bpy.context.view_layer` is
//! `bpy.data.scenes['Scene'].view_layers["ViewLayer"]`, `bpy.context.tool_settings` is
//! `bpy.data.scenes['Scene'].tool_settings`, `bpy.context.collection` is
//! `bpy.data.collections['Collection']` (or `bpy.data.scenes['Scene'].collection` for the
//! master collection). `bpy.context` itself is read-only (`AttributeError: bpy_struct: Context
//! property "object" is read-only`, raised by the resolver).

use frap_rna::traits::{index_out_of_range, no_attribute};
use frap_rna::{
    CollRef, EmbeddedSlot, Field, IdRef, Item, ObjectId, PropDef, PyExc, Rna, RnaColl, RnaError,
    RnaErrorKind, RnaPtr, Root, SceneId, Seg, StructDef, StructRef, Value,
};
use smol_str::SmolStr;

use crate::context::{Context, LayerCollection, ViewLayer, WindowState};
use crate::data::Main;
use crate::id::idlist::IdList;
use crate::id::paths::{layer_collection_name, layer_collection_ptr, view_layer_ptr};

frap_rna::rna_struct! { WindowState as frap_spec_rna::WINDOW {
    scene => hook,
    view_layer => hook,
    x => readonly(|_w| 0i32),
    y => readonly(|_w| 0i32),
    width => readonly(|_w| 0i32),
    height => readonly(|_w| 0i32),
    support_hdr_color => readonly(|_w| false),
}}

frap_rna::rna_struct! { ViewLayer as frap_spec_rna::VIEW_LAYER {
    name => name (set = hook),
    objects => hook_coll,
    layer_collection => struct(layer_collection),
    active_layer_collection => hook,
}}

frap_rna::rna_struct! { LayerCollection as frap_spec_rna::LAYER_COLLECTION {
    collection => hook,
    name => hook,
    children => hook_coll,
    exclude => exclude (set = hook),
    holdout => holdout (set = hook),
    indirect_only => indirect_only (set = hook),
    hide_viewport => hide_viewport (set = hook),
    is_visible => hook,
}}

/// `LayerCollection.children` (keys are collection names, which live in `bpy.data`).
pub struct LayerCollectionChildren<'h> {
    /// `bpy.data`
    pub main: &'h Main,
    /// The scene owning the view layer.
    pub scene: IdRef<SceneId>,
    /// The children.
    pub items: &'h [LayerCollection],
}

impl RnaColl for LayerCollectionChildren<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::LAYER_COLLECTION
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        let c = self.items.get(i)?;
        Some(layer_collection_name(self.main, self.scene, c.collection).into())
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.items.len()).find(|i| self.key_of(*i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|lc| Item::Struct(StructRef::Borrowed(lc)))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

fn ids(v: Vec<IdRef<ObjectId>>) -> Field<'static> {
    Field::Coll(CollRef::Owned(Box::new(IdList::objects(v))))
}

fn not_implemented(member: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::NotImplementedError,
        format!("frap: bpy.context.{member} is not implemented (headless; no UI)"),
    )
}

impl Rna for Context<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::CONTEXT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let scene = self.win.scene;
        let ptr = |path: &[Seg], srna: &'static StructDef| {
            let mut p = RnaPtr::id(scene.any(), &frap_spec_rna::SCENE);
            for s in path {
                p = p.child(s.clone(), srna);
            }
            Field::Ptr(p)
        };
        let id = |o: Option<IdRef<ObjectId>>| Field::Id(o.map(IdRef::any));
        Ok(match prop.identifier {
            "window" => Field::Struct(StructRef::Borrowed(self.win)),
            "blend_data" => Field::Ptr(RnaPtr::new(Root::Data, &frap_spec_rna::BLEND_DATA)),
            "scene" => Field::Id(Some(scene.any())),
            "view_layer" => Field::Ptr(view_layer_ptr(scene, &self.view_layer().name)),
            "engine" => Field::Value(Value::Str(self.scene().render.engine.clone())),
            "collection" => match self.collection() {
                Some(c) => Field::Id(Some(c.any())),
                None => Field::Ptr(RnaPtr::new(
                    Root::Embedded {
                        owner: scene.any(),
                        slot: EmbeddedSlot::MasterCollection,
                    },
                    &frap_spec_rna::COLLECTION,
                )),
            },
            "layer_collection" => {
                let vl = self.view_layer();
                Field::Ptr(layer_collection_ptr(
                    self.main,
                    scene,
                    vl,
                    &vl.active_layer_collection,
                ))
            }
            "tool_settings" => ptr(&[Seg::Prop("tool_settings")], &frap_spec_rna::TOOL_SETTINGS),
            "mode" => Field::Value(Value::Enum(self.mode().as_str().into())),
            "object" => id(self.object()),
            "active_object" => id(self.active_object()),
            "edit_object" => id(self.edit_object()),
            "pose_object" => id(self.pose_object()),
            "selected_objects" => ids(self.selected_objects()),
            "selected_editable_objects" => ids(self.selected_editable_objects()),
            "visible_objects" => ids(self.visible_objects()),
            "selectable_objects" => ids(self.selectable_objects()),
            "editable_objects" => ids(self.editable_objects()),
            "objects_in_mode" => ids(self.objects_in_mode()),
            "objects_in_mode_unique_data" => ids(self.objects_in_mode_unique_data()),
            // The bone members (WP 6.2). A `Context` does not borrow the edit state, so the
            // edit-bone members answer `None` here; the session reads them from the host
            // (`edit_armature::ctx::bone_member`), which answers the active edit bone too.
            "active_bone" if self.mode() != crate::context::ContextMode::EditArmature => match self
                .active_bone()
            {
                Some(b) => match self
                    .main
                    .objects
                    .get(b.object)
                    .and_then(|o| o.data)
                    .and_then(|d| d.typed::<frap_rna::ArmatureId>())
                {
                    Some(arm) => Field::Ptr(crate::edit_armature::paths::bone_ptr(arm, &b.bone)),
                    None => Field::Value(Value::None),
                },
                None => Field::Value(Value::None),
            },
            "active_pose_bone" => match self.active_pose_bone() {
                Some(b) => Field::Ptr(crate::edit_armature::paths::pose_bone_ptr(
                    b.object, &b.bone,
                )),
                None => Field::Value(Value::None),
            },
            "selected_pose_bones" | "selected_pose_bones_from_active_object" => {
                if self.pose_object().is_none() {
                    Field::Value(Value::None)
                } else {
                    Field::Value(Value::Tuple(
                        self.selected_pose_bones()
                            .into_iter()
                            .filter(|b| {
                                prop.identifier == "selected_pose_bones"
                                    || Some(b.object) == self.active_object()
                            })
                            .map(|b| {
                                Value::Ptr(Box::new(crate::edit_armature::paths::pose_bone_ptr(
                                    b.object, &b.bone,
                                )))
                            })
                            .collect(),
                    ))
                }
            }
            "active_bone"
            | "selected_bones"
            | "selected_editable_bones"
            | "editable_bones"
            | "visible_bones" => Field::Value(Value::None),
            // The animation members (WP 6.4, M56W0-12). Blender answers them from an animation
            // editor's channel list (`screen_ctx_active_action` and `…_editable_fcurve` need a
            // Dope Sheet / Graph Editor, `…_nla_track` / `…_nla_strip` an NLA editor through
            // `ANIM_nla_context_*_ptr`); frap has no editors, so they are `None`, as in the
            // 3D Viewport every frap statement runs in (verified on 5.2.4).
            "active_action" => Field::Id(None),
            "active_editable_fcurve" | "active_nla_strip" | "active_nla_track" => {
                Field::Value(Value::None)
            }
            "active_operator" => Field::Id(None),
            other => {
                return Err(if self.srna().find_prop(other).is_some() {
                    not_implemented(other)
                } else {
                    no_attribute(self.srna(), other)
                });
            }
        })
    }
}
