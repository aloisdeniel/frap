// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Scene` (§5.6). The field set is frozen at M1 W0 (decision log W0-21); WP 1.10
//! owns the bindings and behaviour. M7 W0 added the timeline markers (WP 7.6) and the glTF
//! add-on's variant list (WP 7.5), unbound until those WPs.

use frap_anim::AnimData;
use frap_rna::*;
use smol_str::SmolStr;

use crate::context::ViewLayer;
use crate::generated::plain::{
    ColorManagedDisplaySettings, ColorManagedViewSettings, RenderSettings, SceneDisplay,
    SceneEEVEE, ToolSettings, UnitSettings, View3DCursor,
};
use crate::id::{Collection, SceneGltfVariants, TimelineMarker};

/// A scene.
#[derive(Clone, Debug)]
pub struct Scene {
    /// ID header.
    pub header: IdHeader,
    /// `scene.collection`: the embedded master collection ("Scene Collection"), addressed as
    /// `Root::Embedded { owner, slot: MasterCollection }` and mutated through the scene.
    pub collection: Collection,
    /// `scene.view_layers`
    pub view_layers: Vec<ViewLayer>,
    /// `scene.camera`
    pub camera: Option<IdRef<ObjectId>>,
    /// `scene.world`
    pub world: Option<IdRef<WorldId>>,
    /// `scene.frame_start`
    pub frame_start: i32,
    /// `scene.frame_end`
    pub frame_end: i32,
    /// `scene.frame_current` (its setter marks time dirty only, §7.3)
    pub frame_current: i32,
    /// `scene.frame_step`
    pub frame_step: i32,
    /// `scene.render`
    pub render: RenderSettings,
    /// `scene.view_settings`
    pub view_settings: ColorManagedViewSettings,
    /// `scene.display_settings`
    pub display_settings: ColorManagedDisplaySettings,
    /// `scene.display` (Workbench render settings, incl. `shading`)
    pub display: SceneDisplay,
    /// `scene.eevee`
    pub eevee: SceneEEVEE,
    /// `scene.cursor`
    pub cursor: View3DCursor,
    /// `scene.tool_settings`
    pub tool_settings: ToolSettings,
    /// `scene.unit_settings`
    pub unit_settings: UnitSettings,
    /// `scene.animation_data`
    pub animation_data: Option<AnimData>,
    /// `scene.timeline_markers` (M7 W0; WP 7.6).
    pub timeline_markers: Vec<TimelineMarker>,
    /// The glTF add-on's variant list and active variant (M7 W0; WP 7.5).
    pub gltf2_variants: SceneGltfVariants,
    /// `scene->transform_spaces`: the custom transform orientations, in creation order
    /// (WP 8.3.5; `transform.create_orientation`). Not bound in RNA: the
    /// `TransformOrientation` struct is not in the RNA dump.
    pub transform_orientations: Vec<TransformOrientation>,
    /// `scene->orientation_slots` (`SCE_ORIENT_DEFAULT`, `_TRANSLATE`, `_ROTATE`, `_SCALE`;
    /// WP 8.3.5). Not bound in RNA (`TransformOrientationSlot` is not in the dump either).
    pub orientation_slots: [TransformOrientationSlot; 4],
}

/// `TransformOrientation`: a named custom orientation (`scene.transform_orientation_slots[0]
/// .custom_orientation`).
#[derive(Clone, Debug, PartialEq)]
pub struct TransformOrientation {
    /// `name` (unique among the scene's orientations, at most 63 bytes).
    pub name: SmolStr,
    /// `matrix`: columns are the X, Y and Z axes (`ts->mat[i]` is axis `i`).
    pub matrix: glam::Mat3,
}

/// `TransformOrientationSlot`: `type` (a `V3D_ORIENT_*` value) and, for
/// [`Scene::ORIENT_CUSTOM`], the index of the custom orientation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TransformOrientationSlot {
    /// `orient_slot->type`: [`Scene::ORIENT_GLOBAL`] … [`Scene::ORIENT_PARENT`] or
    /// [`Scene::ORIENT_CUSTOM`].
    pub kind: i32,
    /// `orient_slot->index_custom` (-1 unless `kind` is custom).
    pub index_custom: i32,
}

impl Default for TransformOrientationSlot {
    fn default() -> Self {
        TransformOrientationSlot {
            kind: Scene::ORIENT_GLOBAL,
            index_custom: -1,
        }
    }
}

impl Scene {
    /// `V3D_ORIENT_GLOBAL`
    pub const ORIENT_GLOBAL: i32 = 0;
    /// `V3D_ORIENT_PARENT` (the last built-in orientation: GLOBAL 0, LOCAL 1, NORMAL 2, VIEW 3,
    /// GIMBAL 4, CURSOR 5, PARENT 6).
    pub const ORIENT_PARENT: i32 = 6;
    /// `V3D_ORIENT_CUSTOM`: custom orientation `i` has the enum value `ORIENT_CUSTOM + i`.
    pub const ORIENT_CUSTOM: i32 = 1024;
    /// `SCE_ORIENT_DEFAULT`, the slot transforms and the orientation operators use.
    pub const ORIENT_SLOT_DEFAULT: usize = 0;

    /// `BKE_scene_orientation_slot_get_index`: the orientation value of a slot (custom
    /// orientations as `ORIENT_CUSTOM + index`).
    pub fn orientation_slot_index(&self, slot: usize) -> i32 {
        let s = &self.orientation_slots[slot];
        if s.kind == Scene::ORIENT_CUSTOM {
            s.kind + s.index_custom
        } else {
            s.kind
        }
    }

    /// `BKE_scene_orientation_slot_set_index`.
    pub fn orientation_slot_set_index(&mut self, slot: usize, orientation: i32) {
        let is_custom = orientation >= Scene::ORIENT_CUSTOM;
        let s = &mut self.orientation_slots[slot];
        s.kind = if is_custom {
            Scene::ORIENT_CUSTOM
        } else {
            orientation
        };
        s.index_custom = if is_custom {
            orientation - Scene::ORIENT_CUSTOM
        } else {
            -1
        };
    }

    /// `BKE_scene_transform_orientation_remove`: removes custom orientation `index`; slots that
    /// used it fall back to GLOBAL, slots using a later one follow it down.
    pub fn transform_orientation_remove(&mut self, index: usize) {
        if index >= self.transform_orientations.len() {
            return;
        }
        let index = index as i32;
        for s in &mut self.orientation_slots {
            if s.index_custom == index {
                s.kind = Scene::ORIENT_GLOBAL;
                s.index_custom = -1;
            } else if s.index_custom > index {
                s.index_custom -= 1;
            }
        }
        self.transform_orientations.remove(index as usize);
    }

    /// `addMatrixSpace` (`transform_orientations.cc`): with `overwrite`, the orientation named
    /// `name` takes the matrix (created under that name when there is none); otherwise a new
    /// orientation gets a unique name (`name`, `name.001`, …; `uniqueOrientationName`).
    /// Returns its index.
    pub fn transform_orientation_add(
        &mut self,
        name: &str,
        matrix: glam::Mat3,
        overwrite: bool,
    ) -> usize {
        // `sizeof(TransformOrientation::name)` is 64 bytes.
        const MAX_NAME_BYTES: usize = 63;
        let found = overwrite
            .then(|| {
                self.transform_orientations
                    .iter()
                    .position(|t| t.name == name)
            })
            .flatten();
        let index = match found {
            Some(i) => i,
            None => {
                let name = if overwrite {
                    frap_rna::naming::truncate_utf8(name, MAX_NAME_BYTES).into()
                } else {
                    let taken = |n: &str| self.transform_orientations.iter().any(|t| t.name == n);
                    frap_rna::naming::unique_subname(&taken, name, MAX_NAME_BYTES, "Space")
                };
                self.transform_orientations.push(TransformOrientation {
                    name,
                    matrix: glam::Mat3::IDENTITY,
                });
                self.transform_orientations.len() - 1
            }
        };
        self.transform_orientations[index].matrix = matrix;
        index
    }
    /// Whether a timeline marker of this scene binds `id` as its camera (a reference that is
    /// no user, so not in `for_each_ref`, but remapped and cleared like one; WP 7.6).
    pub fn marker_refs(&self, id: IdAny) -> bool {
        self.timeline_markers
            .iter()
            .any(|m| m.camera.map(|c| c.any()) == Some(id))
    }

    /// `(struct, property)` pairs whose setter applies its own range: a caller coerces these
    /// without the generic hard-range clamp and passes the raw number to the setter
    /// (`Scene.frame_current`: `= 1000000000` reads back 1000000000 in Blender 5.2.4;
    /// `VertexGroups.active_index`: `= -3` reads back -3; `ShapeKey.value`: clamped to the
    /// block's `slider_min..slider_max`, the dumped `0..1` being the range at default sliders,
    /// WP 6.3).
    pub const UNCLAMPED: &'static [(&'static str, &'static str)] = &[
        ("Scene", "frame_current"),
        ("VertexGroups", "active_index"),
        ("ShapeKey", "value"),
    ];

    /// `(struct, property)` enums whose items depend on the data (`runtime` in
    /// `spec/overlay/dynamic_enums.toml`) and whose setter validates against the live items:
    /// a caller passes an unknown string identifier on to the setter instead of failing with
    /// the RNA union, so the `TypeError` lists the items Blender lists (M5+M6 stabilization).
    pub const LIVE_ENUMS: &'static [(&'static str, &'static str)] = &[
        ("ColorManagedViewSettings", "look"),
        ("ColorManagedViewSettings", "view_transform"),
        ("View3DShading", "studio_light"),
        ("ImageFormatSettings", "file_format"),
        ("ImageFormatSettings", "color_mode"),
        ("ImageFormatSettings", "color_depth"),
    ];

    /// Blender's `frame_current` setter on a raw Python int (`rna_Scene_frame_current_set`
    /// with "Allow Negative Frames" off: negative frames become 0, large ones are kept).
    pub fn frame_from_input(v: i64) -> i32 {
        v.clamp(0, i64::from(i32::MAX)) as i32
    }

    /// A scene with an empty master collection, the given view layers and the factory values
    /// of every setting (`bpy.data.scenes.new` refines them in WP 1.10 / M3 WP 3.10).
    pub fn new(name: &str, view_layers: Vec<ViewLayer>) -> Scene {
        Scene {
            header: IdHeader::new(name),
            collection: Collection::master(),
            view_layers,
            camera: None,
            world: None,
            frame_start: 1,
            frame_end: 250,
            frame_current: 1,
            frame_step: 1,
            render: RenderSettings::default(),
            view_settings: ColorManagedViewSettings::default(),
            display_settings: ColorManagedDisplaySettings::default(),
            display: SceneDisplay::default(),
            eevee: SceneEEVEE::default(),
            cursor: View3DCursor::default(),
            tool_settings: ToolSettings::default(),
            unit_settings: UnitSettings::default(),
            animation_data: None,
            timeline_markers: Vec::new(),
            gltf2_variants: SceneGltfVariants::default(),
            transform_orientations: Vec::new(),
            orientation_slots: [TransformOrientationSlot::default(); 4],
        }
    }
}

crate::id::id_rna! { Scene as frap_spec_rna::SCENE, "SCENE" {
    camera => id(camera),
    world => id(world),
    objects => hook_coll,
    frame_current => frame_current (set = hook),
    frame_start => frame_start (set = hook),
    frame_end => frame_end (set = hook),
    frame_step => frame_step,
    frame_current_final => readonly(|s| s.frame_current as f32),
    collection => embedded(collection, MasterCollection),
    view_layers => coll(view_layers, api = "ViewLayers"),
    render => struct(render),
    view_settings => struct(view_settings),
    display_settings => struct(display_settings),
    display => struct(display),
    eevee => struct(eevee),
    cursor => struct(cursor),
    tool_settings => struct(tool_settings),
    unit_settings => struct(unit_settings),
    timeline_markers => hook_coll,
}}

impl IdType for Scene {
    type Kind = SceneId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        // The embedded master collection's references count as the scene's (§5.1).
        self.collection.for_each_ref(f);
        if let Some(c) = self.camera {
            f(c.any());
        }
        if let Some(w) = self.world {
            f(w.any());
        }
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
        // Marker cameras are not listed: `BKE_scene_foreach_id` walks them with
        // `IDWALK_CB_NOP` (no user). `remap_ref` still clears them: `Main::remap` also visits
        // the scenes whose markers reference the ID (WP 7.6, `Scene::marker_refs`).
    }
    /// Unlink / relink in the master collection, `camera`, `world` and the action; view
    /// layers drop bases of removed objects at the next sync (`Main::remap` resyncs).
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        self.collection.remap_ref(from, to);
        if self.camera.map(|c| c.any()) == Some(from) {
            self.camera = to.and_then(IdRef::from_any);
        }
        if self.world.map(|w| w.any()) == Some(from) {
            self.world = to.and_then(IdRef::from_any);
        }
        for vl in &mut self.view_layers {
            if vl.active.map(|a| a.any()) == Some(from) {
                vl.active = to.and_then(IdRef::from_any);
            }
            match to.and_then(IdRef::<ObjectId>::from_any) {
                Some(t) => vl
                    .bases
                    .iter_mut()
                    .filter(|b| b.object.any() == from)
                    .for_each(|b| b.object = t),
                None => vl.bases.retain(|b| b.object.any() != from),
            }
        }
        crate::id::remap_action(&mut self.animation_data, from, to);
        for m in &mut self.timeline_markers {
            if m.camera.map(|c| c.any()) == Some(from) {
                m.camera = to.and_then(IdRef::from_any);
            }
        }
    }
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[EmbeddedSlot::MasterCollection]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat3;

    #[test]
    fn custom_orientations_are_named_selected_and_removed_as_blender() {
        let mut s = Scene::new("Scene", Vec::new());
        assert_eq!(
            s.transform_orientation_add("Cube", Mat3::IDENTITY, false),
            0
        );
        assert_eq!(
            s.transform_orientation_add("Cube", Mat3::IDENTITY, false),
            1
        );
        assert_eq!(s.transform_orientations[1].name, "Cube.001");
        // `overwrite` reuses the orientation of that name.
        let m = Mat3::from_rotation_z(0.5);
        assert_eq!(s.transform_orientation_add("Cube", m, true), 0);
        assert_eq!(s.transform_orientations[0].matrix, m);
        assert_eq!(s.transform_orientation_add("Face", m, true), 2);
        s.orientation_slot_set_index(Scene::ORIENT_SLOT_DEFAULT, Scene::ORIENT_CUSTOM + 2);
        assert_eq!(s.orientation_slot_index(0), Scene::ORIENT_CUSTOM + 2);
        // Removing an earlier orientation shifts the slot; removing its own resets it.
        s.transform_orientation_remove(0);
        assert_eq!(s.orientation_slot_index(0), Scene::ORIENT_CUSTOM + 1);
        s.transform_orientation_remove(1);
        assert_eq!(s.orientation_slot_index(0), Scene::ORIENT_GLOBAL);
        assert_eq!(s.transform_orientations.len(), 1);
        s.orientation_slot_set_index(0, 3);
        assert_eq!(s.orientation_slots[0].index_custom, -1);
        assert_eq!(s.orientation_slot_index(0), 3);
    }
}
