// SPDX-License-Identifier: GPL-3.0-or-later
//! Context, window state, view layers, edit state (ARCHITECTURE §8, CONTRACT frozen at M1 W0).
//!
//! `Context` borrows no `EditState`: the mode lives on `Object.mode`, so an operator can hold a
//! `Context` and a `&mut EditState` at the same time (`Host::split_edit`, RFC 0001 F-OP-5).

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use frap_bmesh::BMesh;
use frap_mesh::Mesh;
use frap_rna::{CollectionId, IdKey, IdRef, ObjectId, SceneId, Value};
use glam::Mat4;
use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::data::Main;
use crate::generated::plain::{ToolSettings, View3DCursor, View3DShading};
use crate::id::Scene;

/// Part of the snapshot; RNA: `bpy.context.window` (a `Window` stub).
#[derive(Clone, Debug)]
pub struct WindowState {
    /// `window.scene = …` switches the context scene (verified in `-b`).
    pub scene: IdRef<SceneId>,
    /// `window.view_layer`
    pub view_layer: SmolStr,
    /// The virtual Layout 3D View (§8).
    pub view3d: VirtualView3D,
}

/// The one VIEW_3D area frap behaves as if it had: the factory Layout viewport (§8), so
/// `align='VIEW'` matches Blender and `V`-poll operators pass.
#[derive(Clone, Debug)]
pub struct VirtualView3D {
    /// `space_data.region_3d.view_matrix` (factory Layout value from `startup.json`).
    pub view_matrix: Mat4,
    /// `space_data.shading`
    pub shading: View3DShading,
}

bitflags::bitflags! {
    /// `Base` flags (selection and hiding are per view layer, §8).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct BaseFlag: u8 {
        /// `select_get()`
        const SELECTED = 1 << 0;
        /// `hide_get()`
        const HIDDEN = 1 << 1;
    }
}

/// An object's base in a view layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Base {
    /// The object.
    pub object: IdRef<ObjectId>,
    /// Selection / hiding.
    pub flag: BaseFlag,
}

impl Base {
    /// `select_get()`
    pub fn selected(&self) -> bool {
        self.flag.contains(BaseFlag::SELECTED)
    }
}

/// `LayerCollection` tree node (exclude / hide_viewport / holdout per view layer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerCollection {
    /// The collection; `None` = the scene's master collection (embedded, `Scene.collection`).
    pub collection: Option<IdRef<CollectionId>>,
    /// `exclude`
    pub exclude: bool,
    /// `hide_viewport`
    pub hide_viewport: bool,
    /// `holdout`
    pub holdout: bool,
    /// `indirect_only`
    pub indirect_only: bool,
    /// Child layer collections, in `children` order.
    pub children: Vec<LayerCollection>,
}

impl LayerCollection {
    /// Layer collection of a collection (master when `None`), all flags off.
    pub fn new(collection: Option<IdRef<CollectionId>>) -> Self {
        LayerCollection {
            collection,
            exclude: false,
            hide_viewport: false,
            holdout: false,
            indirect_only: false,
            children: Vec::new(),
        }
    }
    /// Follows a path of child indices.
    pub fn at(&self, path: &LayerCollectionPath) -> Option<&LayerCollection> {
        let mut lc = self;
        for &i in &path.0 {
            lc = lc.children.get(i as usize)?;
        }
        Some(lc)
    }
}

/// Path of child indices from the view layer's root layer collection (empty = master).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct LayerCollectionPath(pub SmallVec<[u16; 4]>);

/// A view layer (inside `Scene`).
#[derive(Clone, Debug)]
pub struct ViewLayer {
    /// Name ("ViewLayer").
    pub name: SmolStr,
    /// Base order = layer-collection sync order.
    pub bases: Vec<Base>,
    /// `view_layer.objects.active` (may be unselected, verified).
    pub active: Option<IdRef<ObjectId>>,
    /// exclude / hide_viewport / holdout tree.
    pub layer_collection: LayerCollection,
    /// `view_layer.active_layer_collection`
    pub active_layer_collection: LayerCollectionPath,
}

/// `Object.mode` (stored per object).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ObjectMode {
    /// OBJECT
    #[default]
    Object,
    /// EDIT
    Edit,
    /// POSE
    Pose,
}

impl ObjectMode {
    /// Blender identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            ObjectMode::Object => "OBJECT",
            ObjectMode::Edit => "EDIT",
            ObjectMode::Pose => "POSE",
        }
    }
}

/// `context.mode` (derived from the active object's mode and type).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ContextMode {
    /// OBJECT
    Object,
    /// EDIT_MESH
    EditMesh,
    /// EDIT_ARMATURE
    EditArmature,
    /// POSE
    Pose,
}

impl ContextMode {
    /// Blender identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            ContextMode::Object => "OBJECT",
            ContextMode::EditMesh => "EDIT_MESH",
            ContextMode::EditArmature => "EDIT_ARMATURE",
            ContextMode::Pose => "POSE",
        }
    }
}

/// A bone of an armature object (`context.active_bone`, `selected_pose_bones`; RFC 0001 F-EVAL-1).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoneRef {
    /// The armature object.
    pub object: IdRef<ObjectId>,
    /// Bone name.
    pub bone: SmolStr,
}

/// Read-only, cached view of an edit BMesh as a `Mesh` (§6.3), keyed by `EditMesh::bm_gen`.
/// Filled lazily from `&self`; reset whenever the BMesh changes.
#[derive(Clone, Debug, Default)]
pub struct ViewCache {
    /// `(bm_gen, mesh)` of the last conversion.
    pub cell: OnceLock<(u64, Arc<Mesh>)>,
}

/// One mesh in Edit Mode.
#[derive(Clone, Debug)]
pub struct EditMesh {
    /// The edit BMesh (COW through `Arc::make_mut`).
    pub bm: Arc<BMesh>,
    /// Bumped by every `OpCtx::bmesh_mut`.
    pub bm_gen: u64,
    /// Cached read-only `to_mesh` view.
    pub view: ViewCache,
}

/// Armature in Edit Mode (edit bones; M6).
#[derive(Clone, Debug, Default)]
pub struct EditArmature {
    _private: (),
}

/// Edit-mode state; part of the snapshot.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    /// Meshes in Edit Mode, keyed by the Mesh's `IdKey`.
    pub meshes: BTreeMap<IdKey, EditMesh>,
    /// Armatures in Edit Mode, keyed by the Armature's `IdKey`.
    pub armatures: BTreeMap<IdKey, Arc<EditArmature>>,
}

/// `bpy.context.preferences` (read-only except the glTF add-on preferences, §8).
#[derive(Clone, Debug, Default)]
pub struct Preferences {
    /// `preferences.addons[<module>].preferences` values by RNA identifier
    /// (`io_scene_gltf2`: `KHR_materials_variants_ui`, …; saved in `.frap`).
    pub addons: IndexMap<SmolStr, IndexMap<SmolStr, Value>>,
}

/// Read-only view of the context (`bpy.context`).
#[derive(Copy, Clone)]
pub struct Context<'a> {
    /// `bpy.data`
    pub main: &'a Main,
    /// Window state.
    pub win: &'a WindowState,
    /// `bpy.context.preferences`
    pub prefs: &'a Preferences,
}

impl<'a> Context<'a> {
    /// `context.scene` (= `window.scene`).
    pub fn scene(&self) -> &'a Scene {
        self.main
            .scenes
            .get(self.win.scene)
            .expect("invariant: window scene exists")
    }
    /// `context.view_layer`
    pub fn view_layer(&self) -> &'a ViewLayer {
        let s = self.scene();
        s.view_layers
            .iter()
            .find(|v| v.name == self.win.view_layer)
            .unwrap_or(&s.view_layers[0])
    }
    /// `context.active_object`
    pub fn active_object(&self) -> Option<IdRef<ObjectId>> {
        self.view_layer().active
    }
    /// `context.object` (the active object in the 3D View).
    pub fn object(&self) -> Option<IdRef<ObjectId>> {
        self.active_object()
    }
    /// `context.selected_objects`, in base order (not click order, verified).
    pub fn selected_objects(&self) -> Vec<IdRef<ObjectId>> {
        self.view_layer()
            .bases
            .iter()
            .filter(|b| b.selected())
            .map(|b| b.object)
            .collect()
    }
    /// `context.selected_editable_objects`
    pub fn selected_editable_objects(&self) -> Vec<IdRef<ObjectId>> {
        todo!("WP 1.10: selected and not linked")
    }
    /// `context.visible_objects`
    pub fn visible_objects(&self) -> Vec<IdRef<ObjectId>> {
        todo!("WP 1.10: base and layer-collection visibility")
    }
    /// `context.mode`, derived from the active object's mode and type.
    pub fn mode(&self) -> ContextMode {
        let mode = self
            .active_object()
            .and_then(|ob| self.main.objects.get(ob))
            .map(|o| o.mode)
            .unwrap_or_default();
        match mode {
            ObjectMode::Object => ContextMode::Object,
            ObjectMode::Edit => ContextMode::EditMesh,
            ObjectMode::Pose => ContextMode::Pose,
        }
    }
    /// `context.edit_object`
    pub fn edit_object(&self) -> Option<IdRef<ObjectId>> {
        todo!("WP 1.10 (M2 for edit mode)")
    }
    /// `context.objects_in_mode`
    pub fn objects_in_mode(&self) -> Vec<IdRef<ObjectId>> {
        todo!("WP 1.10 (M2 for edit mode)")
    }
    /// `context.objects_in_mode_unique_data`
    pub fn objects_in_mode_unique_data(&self) -> Vec<IdRef<ObjectId>> {
        todo!("WP 1.10 (M2 for edit mode)")
    }
    /// `context.collection`: where `*_add` links new objects (the active layer collection's
    /// collection). `None` = the scene's master collection, which is embedded and has no
    /// `IdRef` (Decision log, M1 W0: the §8 draft returned a bare `IdRef<CollectionId>`).
    pub fn collection(&self) -> Option<IdRef<CollectionId>> {
        let vl = self.view_layer();
        vl.layer_collection
            .at(&vl.active_layer_collection)
            .and_then(|lc| lc.collection)
    }
    /// `context.tool_settings` (= `scene.tool_settings`).
    pub fn tool_settings(&self) -> &'a ToolSettings {
        &self.scene().tool_settings
    }
    /// `scene.cursor`
    pub fn cursor(&self) -> &'a View3DCursor {
        &self.scene().cursor
    }
    /// `context.active_bone` (M6).
    pub fn active_bone(&self) -> Option<BoneRef> {
        todo!("M6")
    }
    /// `context.active_pose_bone` (M6).
    pub fn active_pose_bone(&self) -> Option<BoneRef> {
        todo!("M6")
    }
    /// `context.selected_pose_bones` (M6).
    pub fn selected_pose_bones(&self) -> Vec<BoneRef> {
        todo!("M6")
    }
    /// `context.pose_object` (M6).
    pub fn pose_object(&self) -> Option<IdRef<ObjectId>> {
        todo!("M6")
    }
}
