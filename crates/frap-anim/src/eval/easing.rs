// SPDX-License-Identifier: GPL-3.0-or-later
//! Robert Penner's easing equations as Blender uses them for keyframe interpolation.
//!
//! Ported from blender/source/blender/blenlib/intern/easing.cc (Blender 5.2.4; the equations
//! are © 2001 Robert Penner, BSD-3-Clause, as noted in that file). Every function takes
//! `(time, begin, change, duration)` like the C originals, in `f32` with the same operation
//! order; transcendentals go through `libm` (rule 2.2.12).

use core::f32::consts::{FRAC_PI_2, PI};

/// `BLI_easing_back_ease_in`
pub fn back_ease_in(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let time = time / duration;
    change * time * time * ((overshoot + 1.0) * time - overshoot) + begin
}

/// `BLI_easing_back_ease_out`
pub fn back_ease_out(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (time * time * ((overshoot + 1.0) * time + overshoot) + 1.0) + begin
}

/// `BLI_easing_back_ease_in_out`
pub fn back_ease_in_out(time: f32, begin: f32, change: f32, duration: f32, overshoot: f32) -> f32 {
    let overshoot = overshoot * 1.525;
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * (time * time * ((overshoot + 1.0) * time - overshoot)) + begin;
    }
    time -= 2.0;
    change / 2.0 * (time * time * ((overshoot + 1.0) * time + overshoot) + 2.0) + begin
}

/// `BLI_easing_bounce_ease_out`
pub fn bounce_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / duration;
    if time < (1.0 / 2.75) {
        return change * (7.5625 * time * time) + begin;
    }
    if time < (2.0 / 2.75) {
        time -= 1.5 / 2.75;
        return change * ((7.5625 * time) * time + 0.75) + begin;
    }
    if time < (2.5 / 2.75) {
        time -= 2.25 / 2.75;
        return change * ((7.5625 * time) * time + 0.9375) + begin;
    }
    time -= 2.625 / 2.75;
    change * ((7.5625 * time) * time + 0.984375) + begin
}

/// `BLI_easing_bounce_ease_in`
pub fn bounce_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    change - bounce_ease_out(duration - time, 0.0, change, duration) + begin
}

/// `BLI_easing_bounce_ease_in_out`
pub fn bounce_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time < duration / 2.0 {
        return bounce_ease_in(time * 2.0, 0.0, change, duration) * 0.5 + begin;
    }
    bounce_ease_out(time * 2.0 - duration, 0.0, change, duration) * 0.5 + change * 0.5 + begin
}

/// `BLI_easing_circ_ease_in`
pub fn circ_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    -change * ((1.0 - time * time).sqrt() - 1.0) + begin
}

/// `BLI_easing_circ_ease_out`
pub fn circ_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (1.0 - time * time).sqrt() + begin
}

/// `BLI_easing_circ_ease_in_out`
pub fn circ_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return -change / 2.0 * ((1.0 - time * time).sqrt() - 1.0) + begin;
    }
    time -= 2.0;
    change / 2.0 * ((1.0 - time * time).sqrt() + 1.0) + begin
}

/// `BLI_easing_cubic_ease_in`
pub fn cubic_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    change * time * time * time + begin
}

/// `BLI_easing_cubic_ease_out`
pub fn cubic_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (time * time * time + 1.0) + begin
}

/// `BLI_easing_cubic_ease_in_out`
pub fn cubic_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * time * time * time + begin;
    }
    time -= 2.0;
    change / 2.0 * (time * time * time + 2.0) + begin
}

/// `elastic_blend` (`USE_ELASTIC_BLEND`): when the amplitude is less than the change, blend
/// near the crossing point instead of falling off sharply.
fn elastic_blend(time: f32, change: f32, duration: f32, amplitude: f32, s: f32, mut f: f32) -> f32 {
    if change != 0.0 {
        let t = s.abs();
        if amplitude != 0.0 {
            f *= amplitude / change.abs();
        } else {
            f = 0.0;
        }
        if (time * duration).abs() < t {
            let l = (time * duration).abs() / t;
            f = (f * l) + (1.0 - l);
        }
    }
    f
}

/// `BLI_easing_elastic_ease_in`
pub fn elastic_ease_in(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    let mut f = 1.0;
    if time == 0.0 {
        return begin;
    }
    let mut time = time / duration;
    if time == 1.0 {
        return begin + change;
    }
    time -= 1.0;
    let period = if period == 0.0 {
        duration * 0.3
    } else {
        period
    };
    let (s, amplitude) = if amplitude == 0.0 || amplitude < change.abs() {
        let s = period / 4.0;
        f = elastic_blend(time, change, duration, amplitude, s, f);
        (s, change)
    } else {
        (
            period / (2.0 * PI) * libm::asinf(change / amplitude),
            amplitude,
        )
    };
    (-f * (amplitude
        * libm::powf(2.0, 10.0 * time)
        * libm::sinf((time * duration - s) * (2.0 * PI) / period)))
        + begin
}

/// `BLI_easing_elastic_ease_out`
pub fn elastic_ease_out(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    let mut f = 1.0;
    if time == 0.0 {
        return begin;
    }
    let mut time = time / duration;
    if time == 1.0 {
        return begin + change;
    }
    time = -time;
    let period = if period == 0.0 {
        duration * 0.3
    } else {
        period
    };
    let (s, amplitude) = if amplitude == 0.0 || amplitude < change.abs() {
        let s = period / 4.0;
        f = elastic_blend(time, change, duration, amplitude, s, f);
        (s, change)
    } else {
        (
            period / (2.0 * PI) * libm::asinf(change / amplitude),
            amplitude,
        )
    };
    (f * (amplitude
        * libm::powf(2.0, 10.0 * time)
        * libm::sinf((time * duration - s) * (2.0 * PI) / period)))
        + change
        + begin
}

/// `BLI_easing_elastic_ease_in_out`
pub fn elastic_ease_in_out(
    time: f32,
    begin: f32,
    change: f32,
    duration: f32,
    amplitude: f32,
    period: f32,
) -> f32 {
    let mut f = 1.0;
    if time == 0.0 {
        return begin;
    }
    let mut time = time / (duration / 2.0);
    if time == 2.0 {
        return begin + change;
    }
    time -= 1.0;
    let period = if period == 0.0 {
        duration * (0.3 * 1.5)
    } else {
        period
    };
    let (s, amplitude) = if amplitude == 0.0 || amplitude < change.abs() {
        let s = period / 4.0;
        f = elastic_blend(time, change, duration, amplitude, s, f);
        (s, change)
    } else {
        (
            period / (2.0 * PI) * libm::asinf(change / amplitude),
            amplitude,
        )
    };
    if time < 0.0 {
        f *= -0.5;
        return (f
            * (amplitude
                * libm::powf(2.0, 10.0 * time)
                * libm::sinf((time * duration - s) * (2.0 * PI) / period)))
            + begin;
    }
    time = -time;
    f *= 0.5;
    (f * (amplitude
        * libm::powf(2.0, 10.0 * time)
        * libm::sinf((time * duration - s) * (2.0 * PI) / period)))
        + change
        + begin
}

/// `pow_min` = 2^-10.
const POW_MIN: f32 = 0.000_976_562_5;
/// `pow_scale` = 1 / (1 - 2^-10).
const POW_SCALE: f32 = 1.0 / (1.0 - 0.000_976_562_5);

/// `BLI_easing_expo_ease_in`
pub fn expo_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time == 0.0 {
        return begin;
    }
    change * (libm::powf(2.0, 10.0 * (time / duration - 1.0)) - POW_MIN) * POW_SCALE + begin
}

/// `BLI_easing_expo_ease_out`
pub fn expo_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    if time == 0.0 {
        return begin;
    }
    change * (1.0 - (libm::powf(2.0, -10.0 * time / duration) - POW_MIN) * POW_SCALE) + begin
}

/// `BLI_easing_expo_ease_in_out`
pub fn expo_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let duration_half = duration / 2.0;
    let change_half = change / 2.0;
    if time <= duration_half {
        return expo_ease_in(time, begin, change_half, duration_half);
    }
    expo_ease_out(
        time - duration_half,
        begin + change_half,
        change_half,
        duration_half,
    )
}

/// `BLI_easing_linear_ease`
pub fn linear_ease(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    change * time / duration + begin
}

/// `BLI_easing_quad_ease_in`
pub fn quad_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    change * time * time + begin
}

/// `BLI_easing_quad_ease_out`
pub fn quad_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    -change * time * (time - 2.0) + begin
}

/// `BLI_easing_quad_ease_in_out`
pub fn quad_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * time * time + begin;
    }
    time -= 1.0;
    -change / 2.0 * (time * (time - 2.0) - 1.0) + begin
}

/// `BLI_easing_quart_ease_in`
pub fn quart_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    change * time * time * time * time + begin
}

/// `BLI_easing_quart_ease_out`
pub fn quart_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration - 1.0;
    -change * (time * time * time * time - 1.0) + begin
}

/// `BLI_easing_quart_ease_in_out`
pub fn quart_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * time * time * time * time + begin;
    }
    time -= 2.0;
    -change / 2.0 * (time * time * time * time - 2.0) + begin
}

/// `BLI_easing_quint_ease_in`
pub fn quint_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration;
    change * time * time * time * time * time + begin
}

/// `BLI_easing_quint_ease_out`
pub fn quint_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let time = time / duration - 1.0;
    change * (time * time * time * time * time + 1.0) + begin
}

/// `BLI_easing_quint_ease_in_out`
pub fn quint_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    let mut time = time / (duration / 2.0);
    if time < 1.0 {
        return change / 2.0 * time * time * time * time * time + begin;
    }
    time -= 2.0;
    change / 2.0 * (time * time * time * time * time + 2.0) + begin
}

/// `BLI_easing_sine_ease_in`
pub fn sine_ease_in(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    -change * libm::cosf(time / duration * FRAC_PI_2) + change + begin
}

/// `BLI_easing_sine_ease_out`
pub fn sine_ease_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    change * libm::sinf(time / duration * FRAC_PI_2) + begin
}

/// `BLI_easing_sine_ease_in_out`
pub fn sine_ease_in_out(time: f32, begin: f32, change: f32, duration: f32) -> f32 {
    -change / 2.0 * (libm::cosf(PI * time / duration) - 1.0) + begin
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every equation starts at `begin` and ends at `begin + change` (elastic and back included).
    #[test]
    fn endpoints() {
        type E = fn(f32, f32, f32, f32) -> f32;
        let eqs: &[E] = &[
            bounce_ease_in,
            bounce_ease_out,
            bounce_ease_in_out,
            circ_ease_in,
            circ_ease_out,
            circ_ease_in_out,
            cubic_ease_in,
            cubic_ease_out,
            cubic_ease_in_out,
            expo_ease_in,
            expo_ease_out,
            expo_ease_in_out,
            linear_ease,
            quad_ease_in,
            quad_ease_out,
            quad_ease_in_out,
            quart_ease_in,
            quart_ease_out,
            quart_ease_in_out,
            quint_ease_in,
            quint_ease_out,
            quint_ease_in_out,
            sine_ease_in,
            sine_ease_out,
            sine_ease_in_out,
        ];
        for e in eqs {
            assert!((e(0.0, 2.0, 3.0, 10.0) - 2.0).abs() < 1e-5);
            assert!((e(10.0, 2.0, 3.0, 10.0) - 5.0).abs() < 1e-5);
        }
        assert_eq!(elastic_ease_in(10.0, 2.0, 3.0, 10.0, 0.8, 4.1), 5.0);
        assert_eq!(elastic_ease_out(0.0, 2.0, 3.0, 10.0, 0.8, 4.1), 2.0);
        assert!((back_ease_in_out(10.0, 2.0, 3.0, 10.0, 1.70158) - 5.0).abs() < 1e-5);
    }
}
