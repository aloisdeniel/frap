// SPDX-License-Identifier: GPL-3.0-or-later
//! `Action` datablock and `AnimData` (placeholders until M6, see the crate docs).

use frap_rna::*;
use smol_str::SmolStr;

/// `bpy.types.Action` (M6: slots, layers, strips, channelbags, F-curves).
#[derive(Clone, Debug)]
pub struct Action {
    /// ID header.
    pub header: IdHeader,
}

impl Action {
    /// Empty action.
    pub fn new(name: impl Into<SmolStr>) -> Action {
        Action {
            header: IdHeader::new(name),
        }
    }
}

/// `bpy.types.AnimData` (M6: NLA tracks, blend type, extrapolation).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimData {
    /// `animation_data.action`
    pub action: Option<IdRef<ActionId>>,
    /// `animation_data.action_slot_handle`
    pub action_slot_handle: i32,
    /// `animation_data.last_slot_identifier`
    pub last_slot_identifier: SmolStr,
    /// `animation_data.use_nla`
    pub use_nla: bool,
    /// `animation_data.action_influence`
    pub action_influence: f32,
}

impl Rna for Action {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::ACTION
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let _ = prop;
        todo!("M6: Action RNA bindings")
    }
}

impl RnaMut for Action {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let _ = (prop, v);
        todo!("M6: Action RNA bindings")
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        let _ = prop;
        todo!("M6: Action RNA bindings")
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        let _ = prop;
        todo!("M6: Action RNA bindings")
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
    fn for_each_ref(&self, _f: &mut dyn FnMut(IdAny)) {}
    fn remap_ref(&mut self, _from: IdAny, _to: Option<IdAny>) {}
}
