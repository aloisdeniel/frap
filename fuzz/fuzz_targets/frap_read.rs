// SPDX-License-Identifier: GPL-3.0-or-later
//! Fuzz target: the `.frap` reader on arbitrary bytes: `frap_project::container::read` (zip,
//! manifest, migrations, buffer and image hashes) and `scene_map` (`scene.json` → `Main`),
//! strict and lenient: never panics, never allocates without bound, refuses with
//! `ProjectError` / `MapError` (ARCHITECTURE §20.1, nightly from M8; WP 8.7).
//!
//! Random bytes rarely make a zip, so the first byte picks a mode:
//! - even: the rest is the archive itself (seeded with saved `.frap` files);
//! - odd: the rest is `scene.json` followed by up to four buffers, separated by `\0\0\0\0`.
//!   Every `$bN$` in the JSON is replaced by the content hash of buffer `N` (as `{"$buf": …}`
//!   references spell it), and the parts are written as a valid archive with
//!   `container::write`, so the fuzzer mutates the scene mapping and the buffer decoders with
//!   text it can edit.
#![no_main]

use frap_project::buffers::{BufferStore, ContentHash, ImageStore};
use frap_project::container::{self, ReadOptions};
use frap_project::scene_map;
use libfuzzer_sys::fuzz_target;

const SEP: &[u8] = b"\0\0\0\0";

fn split_parts(data: &[u8]) -> Vec<&[u8]> {
    let mut parts = Vec::new();
    let mut rest = data;
    while parts.len() < 4 {
        match rest.windows(SEP.len()).position(|w| w == SEP) {
            Some(i) => {
                parts.push(&rest[..i]);
                rest = &rest[i + SEP.len()..];
            }
            None => break,
        }
    }
    parts.push(rest);
    parts
}

fn archive(data: &[u8]) {
    for lenient in [false, true] {
        let _ = container::read(data, ReadOptions { lenient });
        let _ = scene_map::open(data, lenient);
    }
}

fn scene_with_buffers(data: &[u8]) {
    let parts = split_parts(data);
    let (scene, bufs) = parts.split_first().expect("invariant: at least one part");
    let mut buffers = BufferStore::new();
    let hashes: Vec<ContentHash> = bufs.iter().map(|b| buffers.insert(b.to_vec())).collect();
    let Ok(mut text) = String::from_utf8(scene.to_vec()) else {
        return;
    };
    for (i, h) in hashes.iter().enumerate() {
        text = text.replace(&format!("$b{i}$"), &h.to_hex());
    }
    let images = ImageStore::new();
    let Ok(bytes) = container::write(text.as_bytes(), &buffers, &images) else {
        return;
    };
    archive(&bytes);
}

fuzz_target!(|data: &[u8]| {
    match data.split_first() {
        Some((m, rest)) if m & 1 == 1 => scene_with_buffers(rest),
        Some((_, rest)) => archive(rest),
        None => archive(data),
    }
});
