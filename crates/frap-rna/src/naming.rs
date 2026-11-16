// SPDX-License-Identifier: GPL-3.0-or-later
//! Unique names (ARCHITECTURE §5.4; Blender `BLI_uniquename` / `BKE_id_new_name_validate`).

use smol_str::SmolStr;

fn truncate_utf8(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Splits `"Foo.001"` into `("Foo", Some(1))`; a suffix counts only if it is all digits.
pub fn split_suffix(name: &str) -> (&str, Option<u32>) {
    if let Some(dot) = name.rfind('.') {
        let digits = &name[dot + 1..];
        if !digits.is_empty()
            && digits.bytes().all(|b| b.is_ascii_digit())
            && let Ok(n) = digits.parse::<u32>()
        {
            return (&name[..dot], Some(n));
        }
    }
    (name, None)
}

/// Returns `requested` if free, else `base.NNN` with the lowest free suffix ≥ 1 (zero-padded
/// to 3 digits). Empty → `empty_default`. Truncated to `max_bytes` on a UTF-8 boundary.
pub fn unique_name(
    taken: &dyn Fn(&str) -> bool,
    requested: &str,
    max_bytes: usize,
    empty_default: &str,
) -> SmolStr {
    let requested = if requested.is_empty() {
        empty_default
    } else {
        requested
    };
    let requested = truncate_utf8(requested, max_bytes);
    if !taken(requested) {
        return requested.into();
    }
    let (base, _) = split_suffix(requested);
    let mut n: u32 = 1;
    loop {
        let suffix = format!(".{n:03}");
        let b = truncate_utf8(base, max_bytes.saturating_sub(suffix.len()));
        let candidate = format!("{b}{suffix}");
        if !taken(&candidate) {
            return candidate.into();
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(taken: &[&str], req: &str) -> String {
        unique_name(&|n| taken.contains(&n), req, 255, "Object").to_string()
    }

    #[test]
    fn blender_rules() {
        assert_eq!(run(&[], "Cube"), "Cube");
        assert_eq!(run(&["Cube"], "Cube"), "Cube.001");
        assert_eq!(run(&["Cube", "Cube.001"], "Cube"), "Cube.002");
        assert_eq!(run(&["Foo.001", "Foo.002"], "Foo.001"), "Foo.003");
        assert_eq!(run(&["Bar.5"], "Bar.5"), "Bar.001");
        assert_eq!(run(&[], ""), "Object");
    }
}
