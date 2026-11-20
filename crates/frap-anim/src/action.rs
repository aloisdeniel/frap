// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Action` datablock: slots, layers, keyframe strips and their channelbags (Blender 5.x
//! "slotted actions", `blender::animrig::Action`; RNA `Action`, `ActionSlot`, `ActionLayer`,
//! `ActionStrip`/`ActionKeyframeStrip`, `ActionChannelbag`, `ActionGroup`). M5+M6 W0 contract;
//! WP 6.1 owns the file.
//!
//! Keying (WP 6.4) creates `<IDName>Action` with one slot (`OB<name>`, `KE<name>`,
//! `NT<name>`), one layer and one keyframe strip whose channelbag for that slot holds the
//! F-curves, grouped (`Object Transforms`, the bone name).

use frap_rna::*;
use smol_str::SmolStr;

use crate::fcurve::FCurve;

/// `bpy.types.Action`.
#[derive(Clone, Debug)]
pub struct Action {
    /// ID header.
    pub header: IdHeader,
    /// `Action.slots`
    pub slots: Vec<ActionSlot>,
    /// `Action.layers`
    pub layers: Vec<ActionLayer>,
    /// The handle the next new slot gets (`Action::last_slot_handle` + 1; handles are never
    /// reused).
    pub last_slot_handle: i32,
    /// `Action.use_frame_range`
    pub use_frame_range: bool,
    /// `Action.frame_start` / `frame_end` (manual range)
    pub frame_range: [f32; 2],
    /// `Action.use_cyclic`
    pub use_cyclic: bool,
}

impl Action {
    /// An empty action (`bpy.data.actions.new(name)`: no slots, no layers).
    pub fn new(name: impl Into<SmolStr>) -> Action {
        Action {
            header: IdHeader::new(name),
            slots: Vec::new(),
            layers: Vec::new(),
            last_slot_handle: 0,
            use_frame_range: false,
            frame_range: [1.0, 1.0],
            use_cyclic: false,
        }
    }

    /// The slot with `handle`.
    pub fn slot(&self, handle: i32) -> Option<&ActionSlot> {
        self.slots.iter().find(|s| s.handle == handle)
    }

    /// The channelbag of slot `handle` in the first keyframe strip of the first layer (the only
    /// place Blender 5.2 keys into; `Action.layers[0].strips[0].channelbag(slot)`).
    pub fn channelbag(&self, handle: i32) -> Option<&Channelbag> {
        let ActionStrip::Keyframe(s) = self.layers.first()?.strips.first()?;
        s.channelbags.iter().find(|c| c.slot_handle == handle)
    }
}

/// `bpy.types.ActionSlot`: what an action animates for one ID.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionSlot {
    /// `handle` (stable within the action).
    pub handle: i32,
    /// `identifier`: the two-letter ID type prefix plus the name (`OBCube`, `KEKey`,
    /// `NTShader Nodetree`); `XX` while unbound.
    pub identifier: SmolStr,
    /// `target_id_type` (`OBJECT`, `KEY`, `NODETREE`, …; `UNSPECIFIED` while unbound).
    pub target_id_type: SmolStr,
    /// `select`
    pub select: bool,
    /// `show_expanded`
    pub show_expanded: bool,
    /// `active`
    pub active: bool,
}

/// `bpy.types.ActionLayer`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionLayer {
    /// `name`
    pub name: SmolStr,
    /// `influence`
    pub influence: f32,
    /// `mix_mode` (`REPLACE`, `OFFSET`, `ADD`, `SUBTRACT`, `MULTIPLY`)
    pub mix_mode: SmolStr,
    /// `strips`
    pub strips: Vec<ActionStrip>,
}

/// `bpy.types.ActionStrip` (5.2 has only `type == 'KEYFRAME'`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ActionStrip {
    /// `ActionKeyframeStrip`
    Keyframe(KeyframeStrip),
}

/// `bpy.types.ActionKeyframeStrip`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeyframeStrip {
    /// `channelbags`
    pub channelbags: Vec<Channelbag>,
}

/// `bpy.types.ActionChannelbag`: the F-curves of one slot in one strip.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Channelbag {
    /// `slot_handle`
    pub slot_handle: i32,
    /// `groups`
    pub groups: Vec<ActionGroup>,
    /// `fcurves` (grouped F-curves first, in group order, as Blender keeps them).
    pub fcurves: Vec<FCurve>,
}

/// `bpy.types.ActionGroup`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActionGroup {
    /// `name`
    pub name: SmolStr,
    /// `color_set`
    pub color_set: SmolStr,
    /// `show_expanded`
    pub show_expanded: bool,
    /// `mute`
    pub mute: bool,
    /// `lock`
    pub lock: bool,
    /// `select`
    pub select: bool,
}

impl Rna for Action {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::ACTION
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        crate::rna::action_get(self, prop)
    }
}

impl RnaMut for Action {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        crate::rna::action_set(self, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        crate::rna::action_struct_mut(self, prop)
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        crate::rna::action_coll_mut(self, prop)
    }
}

impl IdType for Action {
    type Kind = ActionId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    // Slots name their targets by identifier, not by pointer: an action holds no ID pointers.
    fn for_each_ref(&self, _f: &mut dyn FnMut(IdAny)) {}
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
}
