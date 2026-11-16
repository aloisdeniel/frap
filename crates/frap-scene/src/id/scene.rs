// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Scene` (§5.6). The field set is frozen at M1 W0 (decision log W0-21); WP 1.10
//! owns the bindings and behaviour. The glTF variant list arrives with M7 (its W0).

use frap_anim::AnimData;
use frap_rna::*;

use crate::context::ViewLayer;
use crate::generated::plain::{
    ColorManagedDisplaySettings, ColorManagedViewSettings, RenderSettings, SceneDisplay,
    SceneEEVEE, ToolSettings, UnitSettings, View3DCursor,
};
use crate::id::Collection;

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
}

impl Scene {
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
        }
    }
}

crate::id::name_only_rna!(Scene, frap_spec_rna::SCENE, SceneId);

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
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        let _ = (from, to);
        todo!("WP 1.10: unlink / relink in the master collection, view layers, camera, world")
    }
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[EmbeddedSlot::MasterCollection]
    }
}
