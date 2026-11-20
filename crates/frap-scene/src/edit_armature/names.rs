// SPDX-License-Identifier: GPL-3.0-or-later
//! Bone names (WP 6.2): the byte limit (`MAXBONENAME - 1`) and `BLI_string_flip_side_name`,
//! which mirror editing, `armature.symmetrize` and `armature.duplicate(do_flip_names=True)`
//! use to pair `.L`/`.R`, `L_`/`R_`, `Left`/`Right` names.
//!
//! Ported from blender/source/blender/blenlib/intern/string_utils.cc
//! (`BLI_string_flip_side_name`), Blender 5.2.4, GPL-2.0-or-later.

/// Bytes a bone name may hold (`MAXBONENAME` is 64 with the terminator).
pub const MAX_BONE_NAME: usize = 63;

fn is_char_sep(c: u8) -> bool {
    matches!(c, b'.' | b' ' | b'-' | b'_')
}

/// Case-insensitive ASCII search (`BLI_strcasestr`), byte offset of the first match.
fn strcasestr(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

fn side_letter(c: u8) -> Option<&'static [u8]> {
    match c {
        b'l' => Some(b"r"),
        b'r' => Some(b"l"),
        b'L' => Some(b"R"),
        b'R' => Some(b"L"),
        _ => None,
    }
}

/// `BLI_string_flip_side_name(name, strip_number)`.
pub fn flip_side_name(name: &str, strip_number: bool) -> String {
    let src = frap_rna::naming::truncate_utf8(name, MAX_BONE_NAME);
    let mut bytes = src.as_bytes().to_vec();
    if bytes.len() < 3 {
        return src.to_owned();
    }
    let mut number: Vec<u8> = Vec::new();
    if bytes.last().is_some_and(|c| c.is_ascii_digit())
        && let Some(index) = bytes.iter().rposition(|&c| c == b'.')
        && bytes.get(index + 1).is_some_and(|c| c.is_ascii_digit())
    {
        if !strip_number {
            number = bytes[index..].to_vec();
        }
        bytes.truncate(index);
    }
    let len = bytes.len();
    let mut prefix = bytes.clone();
    let mut suffix: Vec<u8> = Vec::new();
    let mut replace: Option<&[u8]> = None;
    let mut is_set = false;
    if len > 1
        && is_char_sep(bytes[len - 2])
        && let Some(r) = side_letter(bytes[len - 1])
    {
        is_set = true;
        prefix.truncate(len - 1);
        replace = Some(r);
    }
    if !is_set
        && len > 1
        && is_char_sep(bytes[1])
        && let Some(r) = side_letter(bytes[0])
    {
        is_set = true;
        replace = Some(r);
        suffix = bytes[1..].to_vec();
        prefix.clear();
    }
    if !is_set && len > 5 {
        if let Some(i) = strcasestr(&prefix, b"right").filter(|&i| i == 0 || i == len - 5) {
            replace = Some(if prefix[i] == b'r' {
                b"left"
            } else if prefix[i + 1] == b'I' {
                b"LEFT"
            } else {
                b"Left"
            });
            suffix = prefix[i + 5..].to_vec();
            prefix.truncate(i);
        } else if let Some(i) = strcasestr(&prefix, b"left").filter(|&i| i == 0 || i == len - 4) {
            replace = Some(if prefix[i] == b'l' {
                b"right"
            } else if prefix[i + 1] == b'E' {
                b"RIGHT"
            } else {
                b"Right"
            });
            suffix = prefix[i + 4..].to_vec();
            prefix.truncate(i);
        }
    }
    let mut out = prefix;
    out.extend_from_slice(replace.unwrap_or(b""));
    out.extend_from_slice(&suffix);
    out.extend_from_slice(&number);
    let s = String::from_utf8_lossy(&out).into_owned();
    frap_rna::naming::truncate_utf8(&s, MAX_BONE_NAME).to_owned()
}

#[cfg(test)]
mod tests {
    use super::flip_side_name;

    #[test]
    fn flips_like_blender() {
        assert_eq!(flip_side_name("arm.L", false), "arm.R");
        assert_eq!(flip_side_name("arm_r", false), "arm_l");
        assert_eq!(flip_side_name("L_arm", false), "R_arm");
        assert_eq!(flip_side_name("arm.L.001", false), "arm.R.001");
        assert_eq!(flip_side_name("arm.L.001", true), "arm.R");
        assert_eq!(flip_side_name("LeftArm", false), "RightArm");
        assert_eq!(flip_side_name("armRIGHT", false), "armLEFT");
        assert_eq!(flip_side_name("Bone", false), "Bone");
        assert_eq!(flip_side_name(".L", false), ".L");
    }
}
