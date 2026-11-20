// SPDX-License-Identifier: GPL-3.0-or-later
//! `AnimData` (`ID.animation_data`) and the NLA (`NlaTrack`, `NlaStrip`). M5+M6 W0 contract
//! (the M1 placeholder's five fields kept); WP 6.1 owns the file.

use frap_rna::{ActionId, IdRef};
use smol_str::SmolStr;

/// `bpy.types.AnimData`. `Default` = `animation_data_create()`.
#[derive(Clone, Debug, PartialEq)]
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
    /// `animation_data.action_blend_type`
    pub action_blend_type: SmolStr,
    /// `animation_data.action_extrapolation`
    pub action_extrapolation: SmolStr,
    /// `animation_data.use_tweak_mode`
    pub use_tweak_mode: bool,
    /// `animation_data.use_pin`
    pub use_pin: bool,
    /// `animation_data.nla_tracks` (bottom to top)
    pub nla_tracks: Vec<NlaTrack>,
}

impl Default for AnimData {
    fn default() -> Self {
        AnimData {
            action: None,
            action_slot_handle: 0,
            last_slot_identifier: SmolStr::default(),
            use_nla: true,
            action_influence: 1.0,
            action_blend_type: "REPLACE".into(),
            action_extrapolation: "HOLD".into(),
            use_tweak_mode: false,
            use_pin: false,
            nla_tracks: Vec::new(),
        }
    }
}

/// `bpy.types.NlaTrack`.
#[derive(Clone, Debug, PartialEq)]
pub struct NlaTrack {
    /// `name`
    pub name: SmolStr,
    /// `strips`, in time order
    pub strips: Vec<NlaStrip>,
    /// `mute`
    pub mute: bool,
    /// `is_solo`
    pub is_solo: bool,
    /// `lock`
    pub lock: bool,
    /// `select`
    pub select: bool,
    /// `active`
    pub active: bool,
}

/// `bpy.types.NlaStrip` (action strips; transitions and meta strips are P2).
#[derive(Clone, Debug, PartialEq)]
pub struct NlaStrip {
    /// `name`
    pub name: SmolStr,
    /// `type` (`CLIP`, `TRANSITION`, `META`, `SOUND`)
    pub r#type: SmolStr,
    /// `action`
    pub action: Option<IdRef<ActionId>>,
    /// `action_slot_handle`
    pub action_slot_handle: i32,
    /// `frame_start` / `frame_end`
    pub frame_start: f32,
    /// `frame_end`
    pub frame_end: f32,
    /// `action_frame_start`
    pub action_frame_start: f32,
    /// `action_frame_end`
    pub action_frame_end: f32,
    /// `scale`
    pub scale: f32,
    /// `repeat`
    pub repeat: f32,
    /// `blend_type`
    pub blend_type: SmolStr,
    /// `extrapolation` (`HOLD`, `HOLD_FORWARD`, `NOTHING`)
    pub extrapolation: SmolStr,
    /// `blend_in`
    pub blend_in: f32,
    /// `blend_out`
    pub blend_out: f32,
    /// `use_auto_blend`
    pub use_auto_blend: bool,
    /// `influence`
    pub influence: f32,
    /// `use_animated_influence`
    pub use_animated_influence: bool,
    /// `strip_time`
    pub strip_time: f32,
    /// `use_animated_time`
    pub use_animated_time: bool,
    /// `mute`
    pub mute: bool,
    /// `use_reverse`
    pub use_reverse: bool,
    /// `use_sync_length`
    pub use_sync_length: bool,
    /// `select`
    pub select: bool,
    /// `active`
    pub active: bool,
}
