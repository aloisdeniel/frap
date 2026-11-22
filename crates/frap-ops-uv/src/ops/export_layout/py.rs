// SPDX-License-Identifier: GPL-3.0-or-later
//! The bits of CPython the UV layout writers depend on: `%g` / `format(x, '.Ng')`, `round()`,
//! and the iteration order of a `set` of float triples (the EPS writer names its colours in
//! `set` order).
//!
//! Ported from CPython 3.13 (Blender 5.2.4's Python): `Python/pyhash.c` (`_Py_HashDouble`),
//! `Objects/tupleobject.c` (`tuplehash`), `Objects/setobject.c` (`set_add_entry`,
//! `set_table_resize`, `set_insert_clean`) and `Python/pystrtod.c` (`'g'` formatting),
//! PSF-2.0, compatible with GPL-3.0-or-later.

/// `_PyHASH_BITS` (64-bit build).
const HASH_BITS: u32 = 61;
/// `_PyHASH_MODULUS`.
const HASH_MODULUS: u64 = (1u64 << HASH_BITS) - 1;
/// `_PyHASH_INF`.
const HASH_INF: i64 = 314_159;

/// `_Py_HashDouble` (NaN is hashed by identity in CPython; frap hashes it as 0).
pub(crate) fn hash_double(v: f64) -> i64 {
    if !v.is_finite() {
        if v.is_infinite() {
            return if v > 0.0 { HASH_INF } else { -HASH_INF };
        }
        return 0;
    }
    let (mut m, mut e) = frexp(v);
    let mut sign = 1i64;
    if m < 0.0 {
        sign = -1;
        m = -m;
    }
    let mut x: u64 = 0;
    while m != 0.0 {
        x = ((x << 28) & HASH_MODULUS) | (x >> (HASH_BITS - 28));
        m *= 268_435_456.0;
        e -= 28;
        let y = m as u64;
        m -= y as f64;
        x += y;
        if x >= HASH_MODULUS {
            x -= HASH_MODULUS;
        }
    }
    let e = if e >= 0 {
        (e as u32) % HASH_BITS
    } else {
        HASH_BITS - 1 - (((-1 - e) as u32) % HASH_BITS)
    };
    x = ((x << e) & HASH_MODULUS) | (x >> (HASH_BITS - e));
    let mut r = (x as i64).wrapping_mul(sign);
    if r == -1 {
        r = -2;
    }
    r
}

/// C `frexp`: `v = m * 2^e` with `0.5 <= |m| < 1` (`v` finite and non-zero, else `(v, 0)`).
fn frexp(v: f64) -> (f64, i32) {
    if v == 0.0 || !v.is_finite() {
        return (v, 0);
    }
    let bits = v.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0 {
        // Subnormal: scale up first.
        let (m, e) = frexp(v * f64::from_bits(0x4350_0000_0000_0000)); // 2^54
        return (m, e - 54);
    }
    let e = exp - 1022;
    let m = f64::from_bits((bits & !(0x7ffu64 << 52)) | (1022u64 << 52));
    (m, e)
}

/// `tuplehash` of a tuple of floats.
pub(crate) fn hash_tuple(items: &[f64]) -> i64 {
    const P1: u64 = 11_400_714_785_074_694_791;
    const P2: u64 = 14_029_467_366_897_019_727;
    const P5: u64 = 2_870_177_450_012_600_261;
    let mut acc: u64 = P5;
    for &it in items {
        let lane = hash_double(it) as u64;
        acc = acc.wrapping_add(lane.wrapping_mul(P2));
        acc = acc.rotate_left(31);
        acc = acc.wrapping_mul(P1);
    }
    acc = acc.wrapping_add((items.len() as u64) ^ (P5 ^ 3_527_539));
    if acc == u64::MAX {
        return 1_546_275_796;
    }
    acc as i64
}

/// `PySet_MINSIZE`
const SET_MINSIZE: usize = 8;
/// `LINEAR_PROBES`
const LINEAR_PROBES: usize = 9;
/// `PERTURB_SHIFT`
const PERTURB_SHIFT: u32 = 5;

/// The iteration order of `set(keys)` for float triples: the index of the first occurrence of
/// each distinct key, in table order.
pub(crate) fn set_order(keys: &[[f64; 3]]) -> Vec<usize> {
    // Slots hold `(hash, index of the key)`.
    let mut table: Vec<Option<(i64, usize)>> = vec![None; SET_MINSIZE];
    let mut used = 0usize;
    for (k, key) in keys.iter().enumerate() {
        let hash = hash_tuple(key);
        let mask = table.len() - 1;
        let mut i = (hash as u64 as usize) & mask;
        let mut perturb = hash as u64;
        let mut found = false;
        let slot = 'probe: loop {
            let probes = if i + LINEAR_PROBES <= mask {
                LINEAR_PROBES
            } else {
                0
            };
            for j in 0..=probes {
                match table[i + j] {
                    None => break 'probe i + j,
                    Some((h, idx)) if h == hash && keys[idx] == *key => {
                        found = true;
                        break 'probe i + j;
                    }
                    Some(_) => {}
                }
            }
            perturb >>= PERTURB_SHIFT;
            i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
        };
        if found {
            continue;
        }
        table[slot] = Some((hash, k));
        used += 1;
        // `fill * 5 < mask * 3` (no dummies: fill == used).
        if used * 5 >= mask * 3 {
            let minused = if used > 50_000 { used * 2 } else { used * 4 };
            let mut newsize = SET_MINSIZE;
            while newsize <= minused {
                newsize <<= 1;
            }
            let old = std::mem::replace(&mut table, vec![None; newsize]);
            for (h, idx) in old.into_iter().flatten() {
                insert_clean(&mut table, h, idx);
            }
        }
    }
    table.into_iter().flatten().map(|(_, i)| i).collect()
}

/// `set_insert_clean`.
fn insert_clean(table: &mut [Option<(i64, usize)>], hash: i64, idx: usize) {
    let mask = table.len() - 1;
    let mut perturb = hash as u64;
    let mut i = (hash as u64 as usize) & mask;
    loop {
        if table[i].is_none() {
            table[i] = Some((hash, idx));
            return;
        }
        if i + LINEAR_PROBES <= mask {
            for j in 1..=LINEAR_PROBES {
                if table[i + j].is_none() {
                    table[i + j] = Some((hash, idx));
                    return;
                }
            }
        }
        perturb >>= PERTURB_SHIFT;
        i = (i.wrapping_mul(5).wrapping_add(1).wrapping_add(perturb as usize)) & mask;
    }
}

/// Python `round(x)` to an integer (half to even).
pub(crate) fn round_half_even(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 {
        (2.0 * (x / 2.0).round()) as i64
    } else {
        r as i64
    }
}

/// `'%.<prec>g' % v` / `format(v, '.<prec>g')`.
pub(crate) fn fmt_g(v: f64, prec: usize) -> String {
    let prec = prec.max(1);
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.into();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let e = format!("{:.*e}", prec - 1, v);
    let (mant, exp) = e.split_once('e').expect("exponent format");
    let exp: i32 = exp.parse().expect("exponent");
    if exp >= -4 && exp < prec as i32 {
        let decimals = (prec as i32 - 1 - exp).max(0) as usize;
        strip_zeros(format!("{v:.decimals$}"))
    } else {
        let mant = strip_zeros(mant.to_owned());
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{mant}e{sign}{:02}", exp.abs())
    }
}

/// Removes trailing zeros after a decimal point (and the point itself).
fn strip_zeros(s: String) -> String {
    if !s.contains('.') {
        return s;
    }
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_cpython() {
        // `hash(x)` in CPython 3.13 (64-bit).
        assert_eq!(hash_double(1.0), 1);
        assert_eq!(hash_double(0.5), 1_152_921_504_606_846_976);
        assert_eq!(hash_double(0.8), 1_844_674_407_370_955_264);
        assert_eq!(hash_double(-1.0), -2);
        assert_eq!(hash_double(f64::from(0.8f32)), 1_844_674_434_858_745_856);
        assert_eq!(hash_tuple(&[0.8, 0.8, 0.8]), -3_431_768_586_012_071_185);
        assert_eq!(hash_tuple(&[1.0, 0.0, 0.0]), 7_839_147_235_533_974_352);
    }

    #[test]
    fn set_order_matches_cpython() {
        // `list(set([(1.0, 0.0, 0.0), (0.8, 0.8, 0.8), (0.0, 1.0, 0.0), (0.8, 0.8, 0.8),
        //  (0.0, 0.0, 1.0)]))`.
        let keys = [
            [1.0, 0.0, 0.0],
            [0.8, 0.8, 0.8],
            [0.0, 1.0, 0.0],
            [0.8, 0.8, 0.8],
            [0.0, 0.0, 1.0],
        ];
        let order: Vec<[f64; 3]> = set_order(&keys).into_iter().map(|i| keys[i]).collect();
        assert_eq!(order, EXPECTED_SET_ORDER.to_vec());
    }

    /// The order CPython 3.13 (Blender 5.2.4) prints.
    const EXPECTED_SET_ORDER: [[f64; 3]; 4] = [
        [1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0],
        [0.8, 0.8, 0.8],
    ];

    #[test]
    fn g_format_and_round() {
        assert_eq!(fmt_g(0.25, 2), "0.25");
        assert_eq!(fmt_g(1.0, 2), "1");
        assert_eq!(fmt_g(0.0, 2), "0");
        assert_eq!(fmt_g(f64::from(0.8f32), 3), "0.8");
        assert_eq!(fmt_g(0.00001, 3), "1e-05");
        assert_eq!(fmt_g(123456.0, 3), "1.23e+05");
        assert_eq!(fmt_g(0.0001234, 3), "0.000123");
        assert_eq!(fmt_g(99.96, 3), "100");
        assert_eq!(round_half_even(0.5), 0);
        assert_eq!(round_half_even(1.5), 2);
        assert_eq!(round_half_even(203.99), 204);
        assert_eq!(round_half_even(-2.5), -2);
    }
}
