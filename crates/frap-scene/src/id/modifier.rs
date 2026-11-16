// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Modifier` data (ARCHITECTURE §7.1). Every Blender modifier type exists as data;
//! evaluators arrive with M4 (`frap-modifiers`), whose W0 freezes `ModifierSettings`.

use smol_str::SmolStr;

pub use crate::generated::modifier_settings::ModifierSettings;

/// One entry of `Object.modifiers`.
#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    /// `name` (≤63 bytes, unique per object)
    pub name: SmolStr,
    /// `show_viewport`
    pub show_viewport: bool,
    /// `show_render`
    pub show_render: bool,
    /// `show_in_editmode`
    pub show_in_editmode: bool,
    /// `show_on_cage`
    pub show_on_cage: bool,
    /// `show_expanded`
    pub show_expanded: bool,
    /// `is_active`
    pub is_active: bool,
    /// `use_pin_to_last`
    pub use_pin_to_last: bool,
    /// `persistent_uid`
    pub persistent_uid: u32,
    /// Type and settings (GENERATED: `Subsurf(SubsurfSettings) | Bevel(..) | Nodes(..) | …`).
    pub settings: ModifierSettings,
}
