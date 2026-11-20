// SPDX-License-Identifier: GPL-3.0-or-later
//! `FCurve` and `Keyframe` (RNA `FCurve`, `Keyframe`, `FCurveKeyframePoints`) with Blender's
//! enum identifiers. M5+M6 W0 contract; WP 6.1 owns the file. Evaluation is in
//! [`crate::eval`], the keyframe-array edits (`insert_vert_fcurve`, `sort_time_fcurve`,
//! `BKE_fcurve_deduplicate_keys`, …) in [`keys`].

use smol_str::SmolStr;

pub mod keys;

/// An enum with Blender's RNA identifiers.
macro_rules! rna_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident = $id:literal),+ $(,)? } default $d:ident) => {
        $(#[$m])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vm])* $v),+
        }
        impl $name {
            /// Every item, in RNA order.
            pub const ALL: &'static [$name] = &[$($name::$v),+];
            /// The RNA identifier.
            pub fn identifier(self) -> &'static str {
                match self { $($name::$v => $id),+ }
            }
            /// From an RNA identifier.
            pub fn from_identifier(id: &str) -> Option<$name> {
                match id { $($id => Some($name::$v),)+ _ => None }
            }
        }
        impl Default for $name {
            fn default() -> Self {
                $name::$d
            }
        }
    };
}

rna_enum! {
    /// `Keyframe.interpolation` (`BEZT_IPO_*`).
    Interpolation {
        /// `CONSTANT`
        Constant = "CONSTANT",
        /// `LINEAR`
        Linear = "LINEAR",
        /// `BEZIER`
        Bezier = "BEZIER",
        /// `SINE`
        Sine = "SINE",
        /// `QUAD`
        Quad = "QUAD",
        /// `CUBIC`
        Cubic = "CUBIC",
        /// `QUART`
        Quart = "QUART",
        /// `QUINT`
        Quint = "QUINT",
        /// `EXPO`
        Expo = "EXPO",
        /// `CIRC`
        Circ = "CIRC",
        /// `BACK`
        Back = "BACK",
        /// `BOUNCE`
        Bounce = "BOUNCE",
        /// `ELASTIC`
        Elastic = "ELASTIC",
    } default Bezier
}

rna_enum! {
    /// `Keyframe.easing` (`BEZT_IPO_EASE_*`).
    Easing {
        /// `AUTO`
        Auto = "AUTO",
        /// `EASE_IN`
        EaseIn = "EASE_IN",
        /// `EASE_OUT`
        EaseOut = "EASE_OUT",
        /// `EASE_IN_OUT`
        EaseInOut = "EASE_IN_OUT",
    } default Auto
}

rna_enum! {
    /// `Keyframe.handle_left_type` / `handle_right_type` (`HD_*`).
    HandleType {
        /// `FREE`
        Free = "FREE",
        /// `ALIGNED`
        Aligned = "ALIGNED",
        /// `VECTOR`
        Vector = "VECTOR",
        /// `AUTO`
        Auto = "AUTO",
        /// `AUTO_CLAMPED`
        AutoClamped = "AUTO_CLAMPED",
    } default AutoClamped
}

impl HandleType {
    /// `HD_AUTO` or `HD_AUTO_ANIM`.
    pub fn is_auto(self) -> bool {
        matches!(self, HandleType::Auto | HandleType::AutoClamped)
    }
}

rna_enum! {
    /// `Keyframe.type` (`BEZT_KEYTYPE_*`).
    KeyframeType {
        /// `KEYFRAME`
        Keyframe = "KEYFRAME",
        /// `BREAKDOWN`
        Breakdown = "BREAKDOWN",
        /// `MOVING_HOLD`
        MovingHold = "MOVING_HOLD",
        /// `EXTREME`
        Extreme = "EXTREME",
        /// `JITTER`
        Jitter = "JITTER",
        /// `GENERATED`
        Generated = "GENERATED",
    } default Keyframe
}

rna_enum! {
    /// `FCurve.extrapolation` (`FCURVE_EXTRAPOLATE_*`).
    Extrapolation {
        /// `CONSTANT`
        Constant = "CONSTANT",
        /// `LINEAR`
        Linear = "LINEAR",
    } default Constant
}

/// `bpy.types.Keyframe` (a `BezTriple`). Defaults are `keyframe_points.insert`'s with the
/// factory preferences (BEZIER, AUTO_CLAMPED, `KEYFRAME`).
#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    /// `co` (frame, value)
    pub co: [f32; 2],
    /// `handle_left`
    pub handle_left: [f32; 2],
    /// `handle_right`
    pub handle_right: [f32; 2],
    /// `handle_left_type`
    pub handle_left_type: HandleType,
    /// `handle_right_type`
    pub handle_right_type: HandleType,
    /// `interpolation`
    pub interpolation: Interpolation,
    /// `easing`
    pub easing: Easing,
    /// `back` (BACK interpolation overshoot)
    pub back: f32,
    /// `amplitude` (ELASTIC)
    pub amplitude: f32,
    /// `period` (ELASTIC)
    pub period: f32,
    /// `type`
    pub r#type: KeyframeType,
    /// `select_control_point`
    pub select_control_point: bool,
    /// `select_left_handle`
    pub select_left_handle: bool,
    /// `select_right_handle`
    pub select_right_handle: bool,
}

impl Keyframe {
    /// A zero-initialised key as `keyframe_points.add(count)` appends it (`ED_keyframes_add`:
    /// `MEM_realloc_zeroed`, then BEZIER, AUTO_CLAMPED and selected; `back`, `amplitude` and
    /// `period` stay 0; verified on 5.2.4).
    pub fn zeroed() -> Keyframe {
        Keyframe {
            co: [0.0; 2],
            handle_left: [0.0; 2],
            handle_right: [0.0; 2],
            back: 0.0,
            amplitude: 0.0,
            period: 0.0,
            ..Keyframe::new(0.0, 0.0)
        }
    }

    /// `BEZT_IS_AUTOH`: both handles AUTO or AUTO_CLAMPED.
    pub fn is_auto_h(&self) -> bool {
        self.handle_left_type.is_auto() && self.handle_right_type.is_auto()
    }

    /// `BEZT_ISSEL_ANY`.
    pub fn is_selected_any(&self) -> bool {
        self.select_control_point || self.select_left_handle || self.select_right_handle
    }

    /// A key at `(frame, value)` with handles on the key (recalculated by
    /// `eval::recalc_handles`).
    pub fn new(frame: f32, value: f32) -> Keyframe {
        Keyframe {
            co: [frame, value],
            handle_left: [frame, value],
            handle_right: [frame, value],
            handle_left_type: HandleType::AutoClamped,
            handle_right_type: HandleType::AutoClamped,
            interpolation: Interpolation::Bezier,
            easing: Easing::Auto,
            back: 1.70158,
            amplitude: 0.8,
            period: 4.1,
            r#type: KeyframeType::Keyframe,
            select_control_point: true,
            select_left_handle: true,
            select_right_handle: true,
        }
    }
}

/// `bpy.types.FCurve`.
#[derive(Clone, Debug, PartialEq)]
pub struct FCurve {
    /// `data_path` (relative to the animated ID, `path_from_id` form)
    pub data_path: SmolStr,
    /// `array_index`
    pub array_index: i32,
    /// `group` (the name of a group of the same channelbag)
    pub group: Option<SmolStr>,
    /// `keyframe_points`, sorted by frame
    pub keyframe_points: Vec<Keyframe>,
    /// `extrapolation`
    pub extrapolation: Extrapolation,
    /// `auto_smoothing` (`NONE`, `CONT_ACCEL`; `fcurves.new` takes the preference
    /// "F-Curve smoothing", `CONT_ACCEL` in the factory settings, verified on 5.2.4)
    pub auto_smoothing: SmolStr,
    /// `mute`
    pub mute: bool,
    /// `hide`
    pub hide: bool,
    /// `lock`
    pub lock: bool,
    /// `select`
    pub select: bool,
    /// `is_valid` (cleared when `data_path` does not resolve)
    pub is_valid: bool,
    /// `color_mode`
    pub color_mode: SmolStr,
    /// `color`
    pub color: [f32; 3],
    /// `FCURVE_DISCRETE_VALUES`: the curve animates a boolean or enum property (constant
    /// interpolation and extrapolation; `update_autoflags`, keying). Not an RNA property.
    pub discrete_values: bool,
    /// `FCURVE_INT_VALUES`: the curve animates an integer, boolean or enum property (evaluation
    /// rounds to whole numbers). Not an RNA property.
    pub int_values: bool,
}

impl FCurve {
    /// An empty F-curve for `data_path[array_index]` (`fcurves.new`:
    /// `create_fcurve_for_channel` with the factory preferences).
    pub fn new(data_path: impl Into<SmolStr>, array_index: i32) -> FCurve {
        FCurve {
            data_path: data_path.into(),
            array_index,
            group: None,
            keyframe_points: Vec::new(),
            extrapolation: Extrapolation::Constant,
            auto_smoothing: "CONT_ACCEL".into(),
            mute: false,
            hide: false,
            lock: false,
            select: true,
            is_valid: true,
            color_mode: "AUTO_RAINBOW".into(),
            color: [0.0, 0.0, 0.0],
            discrete_values: false,
            int_values: false,
        }
    }

    /// `fcu->auto_smoothing != FCURVE_SMOOTH_NONE`.
    pub fn smooths(&self) -> bool {
        self.auto_smoothing != "NONE"
    }

    /// `BKE_fcurve_is_empty` (no keys; frap has no samples, drivers or F-modifiers).
    pub fn is_empty(&self) -> bool {
        self.keyframe_points.is_empty()
    }
}
