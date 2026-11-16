// SPDX-License-Identifier: GPL-3.0-or-later
//! GLB container: JSON chunk padded with 0x20, BIN chunk with 0x00, unknown chunks skipped (§17.1). WP 1.5.
//!
//! Layout (glTF 2.0 §4.4, little-endian): a 12-byte header (`glTF`, version 2, total length),
//! then the JSON chunk, then at most one BIN chunk, then chunks of other types, which readers
//! must ignore. Writing follows io_scene_gltf2 (`io/exp/export.py`): header, JSON, BIN, and
//! no BIN chunk when there is no binary data.

use crate::error::GltfError;
use crate::json::Document;

/// `glTF` magic.
pub const MAGIC: u32 = 0x4654_6C67;
/// `JSON` chunk type.
pub const CHUNK_JSON: u32 = 0x4E4F_534A;
/// `BIN\0` chunk type.
pub const CHUNK_BIN: u32 = 0x004E_4942;

/// A parsed GLB (or `.gltf` file, with `bin = None`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Glb {
    /// The JSON document.
    pub document: Document,
    /// The BIN chunk, padding included (buffer 0's `byteLength` may be up to 3 bytes shorter).
    pub bin: Option<Vec<u8>>,
}

/// Output options for [`write_glb`] and [`Document::to_vec`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteOpts {
    /// Pretty-print JSON with this indent and no space after `:`, as io_scene_gltf2 writes
    /// `.gltf` files (indent `"\t"`); compact when `None`.
    pub indent: Option<String>,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Whether `bytes` starts with the GLB magic.
pub fn is_glb(bytes: &[u8]) -> bool {
    u32_at(bytes, 0) == Some(MAGIC)
}

/// Parses a GLB container.
///
/// Rejects a wrong magic or version, a declared length beyond the data, a first chunk that is
/// not JSON, a second BIN chunk and truncated chunks. Unknown chunk types are skipped. The
/// declared length bounds parsing (trailing bytes after it are ignored); chunk lengths that
/// are not multiples of 4 are tolerated.
pub fn read_glb(bytes: &[u8]) -> Result<Glb, GltfError> {
    let (json, bin) = split_glb(bytes)?;
    Ok(Glb {
        document: Document::from_slice(json)?,
        bin,
    })
}

/// The JSON chunk and the BIN chunk of a GLB, checked as [`read_glb`] describes.
pub(crate) fn split_glb(bytes: &[u8]) -> Result<(&[u8], Option<Vec<u8>>), GltfError> {
    let err = |m: String| GltfError::Glb(m);
    if bytes.len() < 12 {
        return Err(err(format!(
            "{} bytes is too short for a GLB header",
            bytes.len()
        )));
    }
    if u32_at(bytes, 0) != Some(MAGIC) {
        return Err(err("bad magic (expected \"glTF\")".into()));
    }
    let version = u32_at(bytes, 4).unwrap_or(0);
    if version != 2 {
        return Err(err(format!("unsupported GLB version {version}")));
    }
    let length = u32_at(bytes, 8).unwrap_or(0) as usize;
    if length > bytes.len() {
        return Err(err(format!(
            "declared length {length} exceeds the {} bytes available",
            bytes.len()
        )));
    }
    let bytes = &bytes[..length];
    let mut at = 12;
    let mut json: Option<&[u8]> = None;
    let mut bin: Option<Vec<u8>> = None;
    let mut index = 0;
    while at < bytes.len() {
        let (Some(len), Some(ty)) = (u32_at(bytes, at), u32_at(bytes, at + 4)) else {
            return Err(err(format!("truncated chunk header at byte {at}")));
        };
        let start = at + 8;
        let end = start
            .checked_add(len as usize)
            .filter(|&e| e <= bytes.len())
            .ok_or_else(|| {
                err(format!(
                    "chunk {index} ({len} bytes) runs past the end of the file"
                ))
            })?;
        let data = &bytes[start..end];
        match (index, ty) {
            (0, CHUNK_JSON) => json = Some(data),
            (0, _) => {
                return Err(err(format!(
                    "first chunk has type {ty:#010x}, expected JSON"
                )));
            }
            (1, CHUNK_BIN) => bin = Some(data.to_vec()),
            (_, CHUNK_BIN) => {
                return Err(err(format!(
                    "BIN chunk at position {index}; it must be the second chunk"
                )));
            }
            (_, CHUNK_JSON) => return Err(err("more than one JSON chunk".into())),
            _ => {} // unknown chunk: ignored (spec)
        }
        at = end;
        index += 1;
    }
    let json = json.ok_or_else(|| err("missing JSON chunk".into()))?;
    Ok((json, bin))
}

/// Reads a GLB, or a `.gltf` JSON document (then `bin` is `None`), by sniffing the magic.
pub fn read(bytes: &[u8]) -> Result<Glb, GltfError> {
    if is_glb(bytes) {
        read_glb(bytes)
    } else {
        Ok(Glb {
            document: Document::from_slice(bytes)?,
            bin: None,
        })
    }
}

fn pad4(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// Writes a GLB: header, JSON chunk (padded with spaces), BIN chunk (padded with zeros; left
/// out when `bin` is `None` or empty, as Blender does).
///
/// # Panics
/// When the file would exceed 4 GiB (the GLB length field is 32-bit).
pub fn write_glb(g: &Glb, opts: &WriteOpts) -> Vec<u8> {
    let json = g.document.to_vec(opts);
    let bin = g.bin.as_deref().filter(|b| !b.is_empty());
    let json_len = pad4(json.len());
    let bin_len = bin.map_or(0, |b| pad4(b.len()));
    let total = 12 + 8 + json_len + if bin.is_some() { 8 + bin_len } else { 0 };
    let le = |n: usize| -> [u8; 4] {
        u32::try_from(n)
            .expect("GLB larger than 4 GiB")
            .to_le_bytes()
    };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&le(total));
    out.extend_from_slice(&le(json_len));
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json);
    out.resize(out.len() + json_len - json.len(), b' ');
    if let Some(b) = bin {
        out.extend_from_slice(&le(bin_len));
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(b);
        out.resize(out.len() + bin_len - b.len(), 0);
    }
    debug_assert_eq!(out.len(), total);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(ty: u32, data: &[u8]) -> Vec<u8> {
        let mut v = (data.len() as u32).to_le_bytes().to_vec();
        v.extend_from_slice(&ty.to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    fn glb(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = chunks.concat();
        let mut v = MAGIC.to_le_bytes().to_vec();
        v.extend_from_slice(&2u32.to_le_bytes());
        v.extend_from_slice(&((12 + body.len()) as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    const JSON: &[u8] = br#"{"asset":{"version":"2.0"}} "#; // 28 bytes

    #[test]
    fn padding_and_layout() {
        let g = Glb {
            document: Document::default(),
            bin: Some(vec![1, 2, 3, 4, 5]),
        };
        let out = write_glb(&g, &WriteOpts::default());
        assert_eq!(&out[0..4], b"glTF");
        assert_eq!(u32_at(&out, 4), Some(2));
        assert_eq!(u32_at(&out, 8), Some(out.len() as u32));
        let json_len = u32_at(&out, 12).unwrap() as usize;
        assert_eq!(json_len % 4, 0);
        assert_eq!(u32_at(&out, 16), Some(CHUNK_JSON));
        let json = &out[20..20 + json_len];
        assert_eq!(json, b"{\"asset\":{\"version\":\"2.0\"}}\x20");
        let b = 20 + json_len;
        assert_eq!(u32_at(&out, b), Some(8));
        assert_eq!(u32_at(&out, b + 4), Some(CHUNK_BIN));
        assert_eq!(&out[b + 8..], &[1, 2, 3, 4, 5, 0, 0, 0]);
        let back = read_glb(&out).unwrap();
        assert_eq!(back.document, g.document);
        assert_eq!(back.bin.as_deref(), Some(&[1, 2, 3, 4, 5, 0, 0, 0][..]));
    }

    #[test]
    fn empty_bin_is_omitted() {
        for bin in [None, Some(vec![])] {
            let out = write_glb(
                &Glb {
                    document: Document::default(),
                    bin,
                },
                &WriteOpts::default(),
            );
            assert_eq!(out.len(), 12 + 8 + 28);
            assert_eq!(read_glb(&out).unwrap().bin, None);
        }
    }

    #[test]
    fn unknown_chunks_are_skipped() {
        let bytes = glb(&[
            chunk(CHUNK_JSON, JSON),
            chunk(CHUNK_BIN, &[9, 9, 9, 9]),
            chunk(0x1234_5678, b"whatever"),
        ]);
        let g = read_glb(&bytes).unwrap();
        assert_eq!(g.bin.as_deref(), Some(&[9u8, 9, 9, 9][..]));
        // Unknown chunk in the BIN position: skipped, and no BIN.
        let bytes = glb(&[chunk(CHUNK_JSON, JSON), chunk(0x1234_5678, b"abcd")]);
        assert_eq!(read_glb(&bytes).unwrap().bin, None);
    }

    #[test]
    fn malformed_containers_are_rejected() {
        let ok = glb(&[chunk(CHUNK_JSON, JSON)]);
        assert!(read_glb(&ok).is_ok());
        let mut bad = ok.clone();
        bad[0] = b'x';
        assert!(read_glb(&bad).unwrap_err().to_string().contains("magic"));
        let mut bad = ok.clone();
        bad[4] = 1;
        assert!(
            read_glb(&bad)
                .unwrap_err()
                .to_string()
                .contains("version 1")
        );
        let mut bad = ok.clone();
        bad[8] = 200;
        assert!(read_glb(&bad).unwrap_err().to_string().contains("exceeds"));
        assert!(read_glb(&ok[..10]).is_err());
        let bad = glb(&[chunk(CHUNK_BIN, &[0; 4]), chunk(CHUNK_JSON, JSON)]);
        assert!(
            read_glb(&bad)
                .unwrap_err()
                .to_string()
                .contains("expected JSON")
        );
        let bad = glb(&[
            chunk(CHUNK_JSON, JSON),
            chunk(0x1234_5678, b"abcd"),
            chunk(CHUNK_BIN, &[0; 4]),
        ]);
        assert!(
            read_glb(&bad)
                .unwrap_err()
                .to_string()
                .contains("second chunk")
        );
        let mut trunc = glb(&[chunk(CHUNK_JSON, JSON)]);
        trunc[12] = 200; // JSON chunk length past the end
        assert!(
            read_glb(&trunc)
                .unwrap_err()
                .to_string()
                .contains("past the end")
        );
        let none = glb(&[]);
        assert!(
            read_glb(&none)
                .unwrap_err()
                .to_string()
                .contains("missing JSON")
        );
    }

    #[test]
    fn read_sniffs_json() {
        let g = read(JSON).unwrap();
        assert_eq!(g.document.asset.version, "2.0");
        assert_eq!(g.bin, None);
        // BOM and NUL padding are tolerated.
        let mut j = b"\xEF\xBB\xBF".to_vec();
        j.extend_from_slice(JSON);
        j.extend_from_slice(&[0, 0]);
        assert!(read(&j).is_ok());
    }

    #[test]
    fn pretty_output_uses_indent() {
        let opts = WriteOpts {
            indent: Some("\t".into()),
        };
        let s = String::from_utf8(Document::default().to_vec(&opts)).unwrap();
        assert_eq!(s, "{\n\t\"asset\":{\n\t\t\"version\":\"2.0\"\n\t}\n}");
    }
}
