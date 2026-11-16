// SPDX-License-Identifier: GPL-3.0-or-later
//! Shortest round-trip f32 formatting for JSON output (§13.3: `0.1`, not
//! `0.10000000149011612`).
//!
//! Rust's `Display` for `f32` prints the shortest decimal string that parses back to the same
//! f32 and never uses an exponent; parsing that string as an `f64` gives the f64 whose own
//! shortest representation is the same digits, which is what a JSON writer working in f64
//! (`serde_json`) needs to print the f32's shortest form. Both are exact, platform-independent
//! operations, so the output is byte-stable across OSes (§21.4).

/// Shortest decimal string that round-trips `x` as an f32 (`0.1`, `1`, `-0`, `1e-7` is
/// written `0.0000001`). `NaN`, `inf` and `-inf` for non-finite values.
pub fn shortest(x: f32) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.to_owned();
    }
    format!("{x}")
}

/// The f64 whose shortest representation equals [`shortest`]`(x)` (`0.1_f32` → `0.1_f64`),
/// for JSON writers that format f64. Non-finite values are returned unchanged.
pub fn shortest_f64(x: f32) -> f64 {
    if !x.is_finite() {
        return f64::from(x);
    }
    format!("{x}")
        .parse()
        .expect("invariant: f32 Display output parses as f64")
}

/// True when `x` is integral and small enough to be written as a JSON integer without loss
/// (`|x| < 2^53`, so any integral f32 below it is exact as i64).
pub fn is_json_integer(x: f32) -> bool {
    x.is_finite() && x.fract() == 0.0 && x.abs() < 9.007_199e15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_strings() {
        assert_eq!(shortest(0.1), "0.1");
        assert_eq!(shortest(1.0), "1");
        assert_eq!(shortest(-0.0), "-0");
        assert_eq!(shortest(0.3), "0.3");
        assert_eq!(shortest(1.0e-7), "0.0000001");
        assert_eq!(
            shortest(3.402_823_5e38),
            "340282350000000000000000000000000000000"
        );
        assert_eq!(shortest(f32::NAN), "NaN");
        assert_eq!(shortest(f32::NEG_INFINITY), "-inf");
    }

    #[test]
    fn shortest_f64_prints_the_same_digits() {
        for x in [
            0.1f32,
            0.7,
            1.0 / 3.0,
            123.456,
            -2.5e-6,
            1.0e20,
            f32::MIN_POSITIVE,
        ] {
            let d = shortest_f64(x);
            assert_eq!(d as f32, x);
            let a: f64 = shortest(x).parse().unwrap();
            assert_eq!(a, d);
        }
        assert_eq!(format!("{}", shortest_f64(0.1)), "0.1");
        assert!(shortest_f64(f32::INFINITY).is_infinite());
        assert!(is_json_integer(3.0) && !is_json_integer(0.5) && !is_json_integer(f32::NAN));
    }
}
