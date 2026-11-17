// SPDX-License-Identifier: GPL-3.0-or-later
//! GLB container writer (glTF 2.0 §4.4): header, JSON chunk padded with spaces, BIN chunk
//! padded with zeros and omitted when empty (as Blender's writer does). M1 moves this to
//! `frap-gltf/src/glb.rs` (format crate, L3).

const MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"

fn padded(mut v: Vec<u8>, pad: u8) -> Vec<u8> {
    while !v.len().is_multiple_of(4) {
        v.push(pad);
    }
    v
}

/// Builds a GLB file.
pub fn write(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let json = padded(json.to_vec(), b' ');
    let bin = padded(bin.to_vec(), 0);
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    out
}
