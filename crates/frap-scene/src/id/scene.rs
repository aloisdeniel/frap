// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Scene` (§5.6). The field set is frozen at M1 W0 (decision log W0-21); WP 1.10
//! owns the bindings and behaviour. M7 W0 added the timeline markers (WP 7.6) and the glTF
//! add-on's variant list (WP 7.5), unbound until those WPs.

use frap_anim::AnimData;
use frap_rna::*;

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
}

impl Scene {
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
        // `IDWALK_CB_NOP` (no user). `remap_ref` still clears them; WP 7.6 makes `Main::remap`
        // reach a scene that references an object only through a marker.
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
