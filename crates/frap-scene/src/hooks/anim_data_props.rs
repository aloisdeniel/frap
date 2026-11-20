// SPDX-License-Identifier: GPL-3.0-or-later
//! Writes below `animation_data` (WP 6.1): `AnimData`, `NlaTrack` and `NlaStrip` properties.
//! The ID bindings do not bind `animation_data` (its `ID.animation_data` get hook serves the
//! struct), so the resolver's write walk cannot reach it from the ID; every writable property
//! gets a set hook that writes through `frap_anim::rna::anim_data` (with Blender's setter side
//! effects: solo exclusivity, strip frame clamping, scale/repeat bounds, blend in/out limits).
//! The action and slot assignments have their own hooks (`anim_data_action.rs`).

use frap_rna::{PropDef, RnaError, RnaPtr, Value};

use super::id_animation_data::write;
use crate::Host;

fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    write(h, this, prop, v)
}

/// One set hook and its declaration per `(struct, property)`.
macro_rules! write_hooks {
    ($($srna:literal: [$($prop:literal),* $(,)?]),* $(,)?) => {$($(
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        const _: () = {
            #[allow(unsafe_code)]
            #[frap_rna::linkme::distributed_slice(crate::RNA_HOOKS)]
            #[linkme(crate = frap_rna::linkme)]
            static HOOK: crate::RnaHookReg = crate::RnaHookReg {
                srna: $srna,
                prop: $prop,
                get: None,
                set: Some(set),
            };
            #[allow(unsafe_code)]
            #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
            #[linkme(crate = frap_rna::linkme)]
            static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
                srna: $srna,
                prop: $prop,
                coll: false,
            };
        };
    )*)*};
}

write_hooks! {
    "AnimData": [
        "action_extrapolation", "action_blend_type", "action_influence", "use_nla",
        "use_tweak_mode", "use_pin", "last_slot_identifier",
    ],
    "NlaTrack": ["name", "is_solo", "select", "mute", "lock"],
    "NlaStrip": [
        "name", "extrapolation", "blend_type", "frame_start", "frame_end", "frame_start_raw",
        "frame_end_raw", "blend_in", "blend_out", "use_auto_blend", "last_slot_identifier",
        "action_frame_start", "action_frame_end", "repeat", "scale", "influence", "strip_time",
        "use_animated_influence", "use_animated_time", "use_animated_time_cyclic", "select",
        "mute", "use_reverse", "use_sync_length",
    ],
}

#[cfg(test)]
mod tests {
    /// Every writable property of the bindings has a hook here or in `anim_data_action.rs`.
    #[test]
    fn every_writable_property_has_a_hook() {
        for (srna, props) in frap_anim::rna::anim_data::WRITABLE {
            for p in *props {
                assert!(
                    crate::RNA_HOOKS
                        .iter()
                        .any(|h| h.srna == *srna && h.prop == *p && h.set.is_some()),
                    "{srna}.{p} has no set hook"
                );
            }
        }
    }
}
