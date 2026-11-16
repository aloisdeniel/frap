// SPDX-License-Identifier: GPL-3.0-or-later
//! `gltf_canonical` (§20.4): parse both files into a canonical JSON form and compare it with
//! the float tolerance.
//!
//! The canonical form is independent of everything an exporter may legitimately reorder:
//! - buffers, buffer views and accessors disappear: accessor data is decoded (normalised
//!   integers become floats, sparse accessors are applied) and inlined;
//! - nodes are compared by name and hierarchy (children sorted by name), scenes by name;
//! - meshes are inlined into their nodes; each primitive becomes a **multiset** of triangles
//!   (lines, points) whose corners are full attribute tuples (morph target deltas included),
//!   each triangle rotated so its smallest corner comes first (winding kept), the multiset
//!   sorted; the primitives of a mesh are sorted too;
//! - materials are inlined with glTF defaults filled in, texture references resolved to
//!   sampler (defaults filled) + image (PNG: size and a hash of the decoded RGBA8 pixels;
//!   other formats: a hash of the bytes);
//! - animations by name, channels sorted by (target node name, path), sampler input/output
//!   decoded;
//! - `extensionsUsed` / `extensionsRequired` as sorted sets; `asset.generator` is ignored
//!   (GLTF §7.2); `extras` and unknown extensions are kept verbatim.
//!
//! This module carries its own minimal glTF reader on purpose: the judge must not share code
//! with the implementation it judges (`frap-gltf`).

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value as Json, json};

use super::rna_values::json_close;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "gltf_canonical";

/// Ordering key of a canonical element (quantised corners).
type SortKey = Vec<Vec<i64>>;
/// The corners of a canonical element (full attribute tuples).
type Corners = Vec<Vec<f64>>;

/// Quantisation used only to **order** multisets (comparison uses `tol`).
const ORDER_Q: f64 = 1e4;

/// A loaded glTF: its JSON and its buffers.
#[derive(Clone, Debug)]
pub struct GltfDoc {
    /// The glTF JSON.
    pub json: Json,
    /// Buffer contents by index (GLB `BIN` chunk, data URIs, external files).
    pub buffers: Vec<Vec<u8>>,
    base_dir: Option<std::path::PathBuf>,
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// Decodes standard base64 (padding optional).
pub fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b'=')
        .collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            acc |= val(*c).ok_or("invalid base64")? << (18 - 6 * i);
        }
        let n = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => return Err("truncated base64".into()),
        };
        out.extend_from_slice(&acc.to_be_bytes()[1..1 + n]);
    }
    Ok(out)
}

fn load_uri(uri: &str, base: Option<&Path>) -> Result<Vec<u8>, String> {
    if let Some(rest) = uri.strip_prefix("data:") {
        let (_, data) = rest
            .split_once(";base64,")
            .ok_or_else(|| "unsupported data URI".to_string())?;
        return base64_decode(data);
    }
    let base = base.ok_or_else(|| format!("external URI {uri:?} without a base directory"))?;
    let decoded = percent_decode(uri);
    std::fs::read(base.join(&decoded)).map_err(|e| format!("cannot read {decoded:?}: {e}"))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some(h) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(h);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Loads a `.glb` or `.gltf` (JSON) from bytes; external URIs resolve against `base_dir`.
pub fn load(bytes: &[u8], base_dir: Option<&Path>) -> Result<GltfDoc, String> {
    let (json, bin) = if bytes.starts_with(b"glTF") {
        let len = u32_at(bytes, 8).ok_or("truncated GLB header")? as usize;
        let data = bytes.get(..len.min(bytes.len())).ok_or("bad GLB length")?;
        let mut o = 12;
        let (mut json, mut bin) = (None, None);
        while o + 8 <= data.len() {
            let clen = u32_at(data, o).ok_or("bad chunk")? as usize;
            let ctype = u32_at(data, o + 4).ok_or("bad chunk")?;
            let body = data.get(o + 8..o + 8 + clen).ok_or("truncated chunk")?;
            match ctype {
                0x4E4F_534A => json = Some(body),
                0x004E_4942 => bin = Some(body.to_vec()),
                _ => {}
            }
            o += 8 + clen;
        }
        let json: Json = serde_json::from_slice(json.ok_or("GLB without JSON chunk")?)
            .map_err(|e| format!("bad GLB JSON: {e}"))?;
        (json, bin)
    } else {
        let json: Json =
            serde_json::from_slice(bytes).map_err(|e| format!("bad glTF JSON: {e}"))?;
        (json, None)
    };
    let mut buffers = Vec::new();
    let mut bin = bin;
    for (i, b) in arr(&json, "buffers").iter().enumerate() {
        match b.get("uri").and_then(|u| u.as_str()) {
            Some(uri) => buffers.push(load_uri(uri, base_dir)?),
            None if i == 0 => buffers.push(bin.take().unwrap_or_default()),
            None => buffers.push(Vec::new()),
        }
    }
    Ok(GltfDoc {
        json,
        buffers,
        base_dir: base_dir.map(Path::to_owned),
    })
}

fn arr<'a>(v: &'a Json, key: &str) -> &'a [Json] {
    v.get(key)
        .and_then(|a| a.as_array())
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn idx(v: &Json, key: &str) -> Option<usize> {
    v.get(key).and_then(|i| i.as_u64()).map(|i| i as usize)
}

fn components(ty: &str) -> Option<usize> {
    Some(match ty {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" | "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => return None,
    })
}

fn comp_size(ct: u64) -> Option<usize> {
    Some(match ct {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return None,
    })
}

fn read_comp(b: &[u8], ct: u64, normalized: bool) -> f64 {
    let raw = match ct {
        5120 => b[0] as i8 as f64,
        5121 => b[0] as f64,
        5122 => i16::from_le_bytes([b[0], b[1]]) as f64,
        5123 => u16::from_le_bytes([b[0], b[1]]) as f64,
        5125 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
    };
    if !normalized {
        return raw;
    }
    match ct {
        5120 => (raw / 127.0).max(-1.0),
        5121 => raw / 255.0,
        5122 => (raw / 32767.0).max(-1.0),
        5123 => raw / 65535.0,
        _ => raw,
    }
}

impl GltfDoc {
    fn view_bytes(&self, view: usize) -> Result<(&[u8], Option<usize>), String> {
        let v = arr(&self.json, "bufferViews")
            .get(view)
            .ok_or_else(|| format!("bufferView {view} missing"))?;
        let buf = self
            .buffers
            .get(idx(v, "buffer").unwrap_or(0))
            .ok_or("buffer missing")?;
        let off = idx(v, "byteOffset").unwrap_or(0);
        let len = idx(v, "byteLength").unwrap_or(0);
        let bytes = buf
            .get(off..off + len)
            .ok_or_else(|| format!("bufferView {view} out of range"))?;
        Ok((bytes, idx(v, "byteStride")))
    }

    fn read_elems(
        &self,
        view: Option<usize>,
        offset: usize,
        count: usize,
        ct: u64,
        n: usize,
        normalized: bool,
    ) -> Result<Vec<Vec<f64>>, String> {
        let Some(view) = view else {
            return Ok(vec![vec![0.0; n]; count]);
        };
        let (bytes, stride) = self.view_bytes(view)?;
        let cs = comp_size(ct).ok_or("bad componentType")?;
        let stride = stride.unwrap_or(cs * n);
        (0..count)
            .map(|i| {
                let base = offset + i * stride;
                (0..n)
                    .map(|c| {
                        let o = base + c * cs;
                        bytes
                            .get(o..o + cs)
                            .map(|b| read_comp(b, ct, normalized))
                            .ok_or_else(|| "accessor out of range".to_string())
                    })
                    .collect()
            })
            .collect()
    }

    /// Decodes accessor `i` into one `Vec<f64>` per element.
    pub fn accessor(&self, i: usize) -> Result<Vec<Vec<f64>>, String> {
        let a = arr(&self.json, "accessors")
            .get(i)
            .ok_or_else(|| format!("accessor {i} missing"))?;
        let count = idx(a, "count").unwrap_or(0);
        let ct = a.get("componentType").and_then(|c| c.as_u64()).unwrap_or(5126);
        let n = components(a.get("type").and_then(|t| t.as_str()).unwrap_or(""))
            .ok_or("bad accessor type")?;
        let normalized = a.get("normalized").and_then(|b| b.as_bool()).unwrap_or(false);
        let mut out = self.read_elems(
            idx(a, "bufferView"),
            idx(a, "byteOffset").unwrap_or(0),
            count,
            ct,
            n,
            normalized,
        )?;
        if let Some(s) = a.get("sparse") {
            let sc = idx(s, "count").unwrap_or(0);
            let ind = s.get("indices").ok_or("sparse without indices")?;
            let ict = ind.get("componentType").and_then(|c| c.as_u64()).unwrap_or(5125);
            let indices = self.read_elems(
                idx(ind, "bufferView"),
                idx(ind, "byteOffset").unwrap_or(0),
                sc,
                ict,
                1,
                false,
            )?;
            let val = s.get("values").ok_or("sparse without values")?;
            let values = self.read_elems(
                idx(val, "bufferView"),
                idx(val, "byteOffset").unwrap_or(0),
                sc,
                ct,
                n,
                normalized,
            )?;
            for (k, v) in indices.iter().zip(values) {
                let k = k[0] as usize;
                *out.get_mut(k).ok_or("sparse index out of range")? = v;
            }
        }
        Ok(out)
    }

    fn image_bytes(&self, img: &Json) -> Result<Vec<u8>, String> {
        if let Some(v) = idx(img, "bufferView") {
            return Ok(self.view_bytes(v)?.0.to_vec());
        }
        match img.get("uri").and_then(|u| u.as_str()) {
            Some(uri) => load_uri(uri, self.base_dir.as_deref()),
            None => Ok(Vec::new()),
        }
    }
}

/// FNV-1a 64-bit (stable across runs and platforms).
pub fn fnv64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn num(x: f64) -> Json {
    serde_json::Number::from_f64(x)
        .map(Json::Number)
        .unwrap_or_else(|| json!({"$float": if x.is_nan() { "nan" } else if x > 0.0 { "inf" } else { "-inf" }}))
}

fn qkey(v: &[f64]) -> Vec<i64> {
    v.iter().map(|x| (x * ORDER_Q).round() as i64).collect()
}

struct Canon<'a> {
    doc: &'a GltfDoc,
}

impl Canon<'_> {
    fn image(&self, i: usize) -> Result<Json, String> {
        let img = arr(&self.doc.json, "images")
            .get(i)
            .ok_or_else(|| format!("image {i} missing"))?;
        let bytes = self.doc.image_bytes(img)?;
        let mut o = Map::new();
        if let Some(n) = img.get("name") {
            o.insert("name".into(), n.clone());
        }
        if let Some(m) = img.get("mimeType") {
            o.insert("mimeType".into(), m.clone());
        }
        match crate::compare::image_ssim::decode_png(&bytes) {
            Ok(d) => {
                o.insert("width".into(), json!(d.width));
                o.insert("height".into(), json!(d.height));
                let rgba = crate::compare::image_ssim::to_rgba8(&d);
                o.insert("pixels_fnv64".into(), json!(fnv64(&rgba)));
            }
            Err(_) => {
                o.insert("bytes_fnv64".into(), json!(fnv64(&bytes)));
            }
        }
        Ok(Json::Object(o))
    }

    fn texture(&self, i: usize) -> Result<Json, String> {
        let t = arr(&self.doc.json, "textures")
            .get(i)
            .ok_or_else(|| format!("texture {i} missing"))?;
        let mut o = Map::new();
        let s = idx(t, "sampler").and_then(|s| arr(&self.doc.json, "samplers").get(s));
        let mut sampler = Map::new();
        for (k, d) in [
            ("magFilter", Json::Null),
            ("minFilter", Json::Null),
            ("wrapS", json!(10497)),
            ("wrapT", json!(10497)),
        ] {
            sampler.insert(
                k.into(),
                s.and_then(|s| s.get(k)).cloned().unwrap_or(d),
            );
        }
        o.insert("sampler".into(), Json::Object(sampler));
        if let Some(src) = idx(t, "source") {
            o.insert("source".into(), self.image(src)?);
        }
        if let Some(ext) = t.get("extensions").and_then(|e| e.as_object()) {
            let mut e2 = Map::new();
            for (k, v) in ext {
                let mut v = v.clone();
                if let Some(src) = idx(&v, "source") {
                    v["source"] = self.image(src)?;
                }
                e2.insert(k.clone(), v);
            }
            o.insert("extensions".into(), Json::Object(e2));
        }
        Ok(Json::Object(o))
    }

    /// Replaces every `{ "index": i, … }` texture-info object (keys ending in `Texture`) by its
    /// resolved texture, recursively.
    fn resolve_textures(&self, v: &mut Json) -> Result<(), String> {
        match v {
            Json::Object(o) => {
                for (k, x) in o.iter_mut() {
                    if k.ends_with("Texture")
                        && let Some(i) = idx(x, "index")
                    {
                        let tex = self.texture(i)?;
                        let xo = x.as_object_mut().expect("invariant: has index");
                        xo.remove("index");
                        xo.entry("texCoord").or_insert(json!(0));
                        xo.insert("texture".into(), tex);
                    }
                    self.resolve_textures(x)?;
                }
            }
            Json::Array(a) => {
                for x in a {
                    self.resolve_textures(x)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn material(&self, i: usize) -> Result<Json, String> {
        let mut m = arr(&self.doc.json, "materials")
            .get(i)
            .cloned()
            .ok_or_else(|| format!("material {i} missing"))?;
        let o = m.as_object_mut().ok_or("material is not an object")?;
        let pbr = o
            .entry("pbrMetallicRoughness")
            .or_insert_with(|| json!({}));
        if let Some(p) = pbr.as_object_mut() {
            p.entry("baseColorFactor").or_insert(json!([1.0, 1.0, 1.0, 1.0]));
            p.entry("metallicFactor").or_insert(json!(1.0));
            p.entry("roughnessFactor").or_insert(json!(1.0));
        }
        o.entry("emissiveFactor").or_insert(json!([0.0, 0.0, 0.0]));
        let mode = o
            .entry("alphaMode")
            .or_insert(json!("OPAQUE"))
            .as_str()
            .unwrap_or("OPAQUE")
            .to_owned();
        if mode == "MASK" {
            o.entry("alphaCutoff").or_insert(json!(0.5));
        } else {
            o.remove("alphaCutoff");
        }
        o.entry("doubleSided").or_insert(json!(false));
        self.resolve_textures(&mut m)?;
        Ok(m)
    }

    fn primitive(&self, p: &Json) -> Result<Json, String> {
        let mode = p.get("mode").and_then(|m| m.as_u64()).unwrap_or(4);
        let mut cols: BTreeMap<String, Vec<Vec<f64>>> = BTreeMap::new();
        if let Some(a) = p.get("attributes").and_then(|a| a.as_object()) {
            for (k, v) in a {
                let i = v.as_u64().ok_or("bad attribute index")? as usize;
                cols.insert(k.clone(), self.doc.accessor(i)?);
            }
        }
        for (t, target) in arr(p, "targets").iter().enumerate() {
            if let Some(a) = target.as_object() {
                for (k, v) in a {
                    let i = v.as_u64().ok_or("bad target index")? as usize;
                    cols.insert(format!("target{t}:{k}"), self.doc.accessor(i)?);
                }
            }
        }
        let count = cols.values().map(Vec::len).max().unwrap_or(0);
        let layout: Vec<Json> = cols
            .iter()
            .map(|(k, v)| json!(format!("{k}:{}", v.first().map_or(0, Vec::len))))
            .collect();
        let corner = |i: usize| -> Vec<f64> {
            cols.values()
                .flat_map(|c| c.get(i).cloned().unwrap_or_default())
                .collect()
        };
        let indices: Vec<usize> = match idx(p, "indices") {
            Some(a) => self
                .doc
                .accessor(a)?
                .into_iter()
                .map(|v| v[0] as usize)
                .collect(),
            None => (0..count).collect(),
        };
        let mut elems: Vec<Vec<usize>> = Vec::new();
        let n = indices.len();
        match mode {
            0 => elems.extend(indices.iter().map(|&i| vec![i])),
            1 => elems.extend(indices.chunks_exact(2).map(|c| c.to_vec())),
            2 | 3 => {
                for k in 0..n.saturating_sub(1) {
                    elems.push(vec![indices[k], indices[k + 1]]);
                }
                if mode == 2 && n > 1 {
                    elems.push(vec![indices[n - 1], indices[0]]);
                }
            }
            4 => elems.extend(indices.chunks_exact(3).map(|c| c.to_vec())),
            5 => {
                for k in 0..n.saturating_sub(2) {
                    if k % 2 == 0 {
                        elems.push(vec![indices[k], indices[k + 1], indices[k + 2]]);
                    } else {
                        elems.push(vec![indices[k + 1], indices[k], indices[k + 2]]);
                    }
                }
            }
            6 => {
                for k in 1..n.saturating_sub(1) {
                    elems.push(vec![indices[k], indices[k + 1], indices[0]]);
                }
            }
            _ => return Err(format!("bad primitive mode {mode}")),
        }
        let mut canon: Vec<(SortKey, Corners)> = elems
            .into_iter()
            .map(|e| {
                let mut cs: Vec<Vec<f64>> = e.iter().map(|&i| corner(i)).collect();
                if cs.len() == 2 {
                    cs.sort_by_key(|c| qkey(c));
                } else if cs.len() == 3 {
                    let r = (0..3).min_by_key(|&k| qkey(&cs[k])).unwrap_or(0);
                    cs.rotate_left(r);
                }
                (cs.iter().map(|c| qkey(c)).collect(), cs)
            })
            .collect();
        canon.sort_by(|a, b| a.0.cmp(&b.0));
        let elements: Vec<Json> = canon
            .into_iter()
            .map(|(_, cs)| {
                Json::Array(
                    cs.into_iter()
                        .map(|c| Json::Array(c.into_iter().map(num).collect()))
                        .collect(),
                )
            })
            .collect();
        let material = match idx(p, "material") {
            Some(m) => self.material(m)?,
            None => Json::Null,
        };
        let mut o = Map::new();
        o.insert(
            "mode".into(),
            json!(["POINTS", "LINES", "LINE_LOOP", "LINE_STRIP", "TRIANGLES", "TRIANGLE_STRIP", "TRIANGLE_FAN"]
                .get(mode as usize)
                .copied()
                .unwrap_or("?")),
        );
        o.insert("layout".into(), Json::Array(layout));
        o.insert("material".into(), material);
        o.insert("elements".into(), Json::Array(elements));
        for k in ["extensions", "extras"] {
            if let Some(v) = p.get(k) {
                o.insert(k.into(), v.clone());
            }
        }
        Ok(Json::Object(o))
    }

    fn mesh(&self, i: usize) -> Result<Json, String> {
        let m = arr(&self.doc.json, "meshes")
            .get(i)
            .ok_or_else(|| format!("mesh {i} missing"))?;
        let mut prims = arr(m, "primitives")
            .iter()
            .map(|p| self.primitive(p))
            .collect::<Result<Vec<_>, _>>()?;
        prims.sort_by_key(|p| {
            (
                p["material"]["name"].to_string(),
                p["mode"].to_string(),
                p["layout"].to_string(),
                p["elements"].as_array().map_or(0, Vec::len),
            )
        });
        let mut o = Map::new();
        o.insert("name".into(), m.get("name").cloned().unwrap_or(Json::Null));
        o.insert("primitives".into(), Json::Array(prims));
        for k in ["weights", "extensions", "extras"] {
            if let Some(v) = m.get(k) {
                o.insert(k.into(), v.clone());
            }
        }
        Ok(Json::Object(o))
    }

    fn node_name(&self, i: usize) -> Json {
        arr(&self.doc.json, "nodes")
            .get(i)
            .and_then(|n| n.get("name"))
            .cloned()
            .unwrap_or_else(|| json!(format!("#{i}")))
    }

    fn node(&self, i: usize, depth: usize) -> Result<Json, String> {
        if depth > 256 {
            return Err("node hierarchy too deep (cycle?)".into());
        }
        let n = arr(&self.doc.json, "nodes")
            .get(i)
            .ok_or_else(|| format!("node {i} missing"))?;
        let mut o = Map::new();
        o.insert("name".into(), self.node_name(i));
        if let Some(m) = n.get("matrix") {
            o.insert("matrix".into(), m.clone());
        } else {
            o.insert(
                "translation".into(),
                n.get("translation").cloned().unwrap_or(json!([0.0, 0.0, 0.0])),
            );
            o.insert(
                "rotation".into(),
                n.get("rotation").cloned().unwrap_or(json!([0.0, 0.0, 0.0, 1.0])),
            );
            o.insert(
                "scale".into(),
                n.get("scale").cloned().unwrap_or(json!([1.0, 1.0, 1.0])),
            );
        }
        if let Some(m) = idx(n, "mesh") {
            o.insert("mesh".into(), self.mesh(m)?);
        }
        if let Some(c) = idx(n, "camera") {
            o.insert(
                "camera".into(),
                arr(&self.doc.json, "cameras").get(c).cloned().unwrap_or(Json::Null),
            );
        }
        if let Some(s) = idx(n, "skin") {
            let skin = arr(&self.doc.json, "skins").get(s).ok_or("skin missing")?;
            let mut so = Map::new();
            so.insert(
                "joints".into(),
                Json::Array(
                    arr(skin, "joints")
                        .iter()
                        .filter_map(|j| j.as_u64())
                        .map(|j| self.node_name(j as usize))
                        .collect(),
                ),
            );
            if let Some(a) = idx(skin, "inverseBindMatrices") {
                so.insert(
                    "inverseBindMatrices".into(),
                    Json::Array(
                        self.doc
                            .accessor(a)?
                            .into_iter()
                            .map(|v| Json::Array(v.into_iter().map(num).collect()))
                            .collect(),
                    ),
                );
            }
            if let Some(sk) = idx(skin, "skeleton") {
                so.insert("skeleton".into(), self.node_name(sk));
            }
            o.insert("skin".into(), Json::Object(so));
        }
        if let Some(w) = n.get("weights") {
            o.insert("weights".into(), w.clone());
        }
        if let Some(ext) = n.get("extensions").and_then(|e| e.as_object()) {
            let mut e2 = Map::new();
            for (k, v) in ext {
                let mut v = v.clone();
                if k == "KHR_lights_punctual"
                    && let Some(l) = idx(&v, "light")
                {
                    v = self
                        .doc
                        .json
                        .pointer(&format!("/extensions/KHR_lights_punctual/lights/{l}"))
                        .cloned()
                        .unwrap_or(Json::Null);
                }
                e2.insert(k.clone(), v);
            }
            o.insert("extensions".into(), Json::Object(e2));
        }
        if let Some(x) = n.get("extras") {
            o.insert("extras".into(), x.clone());
        }
        let mut children = arr(n, "children")
            .iter()
            .filter_map(|c| c.as_u64())
            .map(|c| self.node(c as usize, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        children.sort_by_key(|c| c["name"].to_string());
        if !children.is_empty() {
            o.insert("children".into(), Json::Array(children));
        }
        Ok(Json::Object(o))
    }

    fn animation(&self, a: &Json) -> Result<Json, String> {
        let samplers = arr(a, "samplers");
        let mut channels = Vec::new();
        for c in arr(a, "channels") {
            let target = c.get("target").cloned().unwrap_or(Json::Null);
            let node = idx(&target, "node").map(|n| self.node_name(n));
            let path = target.get("path").cloned().unwrap_or(Json::Null);
            let s = idx(c, "sampler")
                .and_then(|s| samplers.get(s))
                .ok_or("animation sampler missing")?;
            let decode = |k: &str| -> Result<Json, String> {
                Ok(match idx(s, k) {
                    Some(i) => Json::Array(
                        self.doc
                            .accessor(i)?
                            .into_iter()
                            .map(|v| Json::Array(v.into_iter().map(num).collect()))
                            .collect(),
                    ),
                    None => Json::Null,
                })
            };
            let mut t2 = Map::new();
            t2.insert("node".into(), node.unwrap_or(Json::Null));
            t2.insert("path".into(), path);
            if let Some(e) = target.get("extensions") {
                t2.insert("extensions".into(), e.clone());
            }
            channels.push(json!({
                "target": Json::Object(t2),
                "interpolation": s.get("interpolation").cloned().unwrap_or(json!("LINEAR")),
                "input": decode("input")?,
                "output": decode("output")?,
            }));
        }
        channels.sort_by_key(|c| c["target"].to_string());
        Ok(json!({
            "name": a.get("name").cloned().unwrap_or(Json::Null),
            "channels": channels,
        }))
    }

    fn run(&self) -> Result<Json, String> {
        let j = &self.doc.json;
        let mut o = Map::new();
        o.insert(
            "asset".into(),
            json!({"version": j.pointer("/asset/version").cloned().unwrap_or(Json::Null)}),
        );
        for k in ["extensionsUsed", "extensionsRequired"] {
            let mut v: Vec<String> = arr(j, k)
                .iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect();
            v.sort();
            v.dedup();
            o.insert(k.into(), json!(v));
        }
        let scenes = arr(j, "scenes");
        let mut sc = Vec::new();
        for s in scenes {
            let mut roots = arr(s, "nodes")
                .iter()
                .filter_map(|n| n.as_u64())
                .map(|n| self.node(n as usize, 0))
                .collect::<Result<Vec<_>, _>>()?;
            roots.sort_by_key(|r| r["name"].to_string());
            let mut so = Map::new();
            so.insert("name".into(), s.get("name").cloned().unwrap_or(Json::Null));
            so.insert("nodes".into(), Json::Array(roots));
            for k in ["extensions", "extras"] {
                if let Some(v) = s.get(k) {
                    so.insert(k.into(), v.clone());
                }
            }
            sc.push(Json::Object(so));
        }
        sc.sort_by_key(|s| s["name"].to_string());
        o.insert(
            "scene".into(),
            idx(j, "scene")
                .and_then(|s| scenes.get(s))
                .and_then(|s| s.get("name"))
                .cloned()
                .unwrap_or(Json::Null),
        );
        o.insert("scenes".into(), Json::Array(sc));
        let mut anims = arr(j, "animations")
            .iter()
            .map(|a| self.animation(a))
            .collect::<Result<Vec<_>, _>>()?;
        anims.sort_by_key(|a| a["name"].to_string());
        o.insert("animations".into(), Json::Array(anims));
        // Root-level extensions other than the ones inlined above, and extras.
        if let Some(ext) = j.get("extensions").and_then(|e| e.as_object()) {
            let rest: Map<String, Json> = ext
                .iter()
                .filter(|(k, _)| k.as_str() != "KHR_lights_punctual")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !rest.is_empty() {
                o.insert("extensions".into(), Json::Object(rest));
            }
        }
        if let Some(x) = j.get("extras") {
            o.insert("extras".into(), x.clone());
        }
        Ok(Json::Object(o))
    }
}

/// The canonical form of a loaded document.
pub fn canonicalize(doc: &GltfDoc) -> Result<Json, String> {
    Canon { doc }.run()
}

/// Loads and canonicalises a `.glb`/`.gltf` file.
pub fn canonicalize_file(path: &Path) -> Result<Json, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    canonicalize(&load(&bytes, path.parent())?)
}

/// Compares two canonical forms (floats within `tol`).
pub fn compare(what: &str, frap: &Json, blender: &Json, tol: f32) -> Vec<Mismatch> {
    let mut out = Vec::new();
    json_close(NAME, what, frap, blender, tol as f64, &mut out);
    out
}

/// Test fixtures: a tiny glTF builder (shared with `gltf_roundtrip`'s tests).
#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) struct Builder {
        pub json: Json,
        pub bin: Vec<u8>,
    }

    impl Builder {
        pub(crate) fn new() -> Self {
            Builder {
                json: json!({"asset": {"version": "2.0", "generator": "test"},
                             "buffers": [{}], "bufferViews": [], "accessors": [],
                             "meshes": [], "nodes": [], "scenes": [], "materials": []}),
                bin: Vec::new(),
            }
        }
        pub(crate) fn f32s(&mut self, data: &[f32], ty: &str) -> usize {
            while !self.bin.len().is_multiple_of(4) {
                self.bin.push(0);
            }
            let off = self.bin.len();
            for x in data {
                self.bin.extend_from_slice(&x.to_le_bytes());
            }
            self.view_accessor(off, data.len() * 4, 5126, data.len() / components(ty).unwrap(), ty)
        }
        pub(crate) fn u16s(&mut self, data: &[u16]) -> usize {
            while !self.bin.len().is_multiple_of(4) {
                self.bin.push(0);
            }
            let off = self.bin.len();
            for x in data {
                self.bin.extend_from_slice(&x.to_le_bytes());
            }
            self.view_accessor(off, data.len() * 2, 5123, data.len(), "SCALAR")
        }
        fn view_accessor(&mut self, off: usize, len: usize, ct: u32, count: usize, ty: &str) -> usize {
            let views = self.json["bufferViews"].as_array_mut().unwrap();
            views.push(json!({"buffer": 0, "byteOffset": off, "byteLength": len}));
            let v = views.len() - 1;
            let acc = self.json["accessors"].as_array_mut().unwrap();
            acc.push(json!({"bufferView": v, "componentType": ct, "count": count, "type": ty}));
            acc.len() - 1
        }
        pub(crate) fn glb(&self) -> Vec<u8> {
            let mut j = self.json.clone();
            j["buffers"][0]["byteLength"] = json!(self.bin.len());
            let mut js = serde_json::to_vec(&j).unwrap();
            while !js.len().is_multiple_of(4) {
                js.push(b' ');
            }
            let mut bin = self.bin.clone();
            while !bin.len().is_multiple_of(4) {
                bin.push(0);
            }
            let total = 12 + 8 + js.len() + 8 + bin.len();
            let mut out = Vec::new();
            out.extend_from_slice(b"glTF");
            out.extend_from_slice(&2u32.to_le_bytes());
            out.extend_from_slice(&(total as u32).to_le_bytes());
            out.extend_from_slice(&(js.len() as u32).to_le_bytes());
            out.extend_from_slice(&0x4E4F_534Au32.to_le_bytes());
            out.extend_from_slice(&js);
            out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
            out.extend_from_slice(&0x004E_4942u32.to_le_bytes());
            out.extend_from_slice(&bin);
            out
        }
    }

    /// A quad (two triangles) in a node named `name`, vertices in the given order.
    pub(crate) fn quad_scene(order: [usize; 4], tris: [[u16; 3]; 2], z: f32, generator: &str) -> Vec<u8> {
        let verts = [[0.0, 0.0, z], [1.0, 0.0, z], [1.0, 1.0, z], [0.0, 1.0, z]];
        let mut b = Builder::new();
        b.json["asset"]["generator"] = json!(generator);
        let mut pos = Vec::new();
        let mut nor = Vec::new();
        for &i in &order {
            pos.extend_from_slice(&verts[i]);
            nor.extend_from_slice(&[0.0, 0.0, 1.0]);
        }
        let p = b.f32s(&pos, "VEC3");
        let n = b.f32s(&nor, "VEC3");
        let idx: Vec<u16> = tris.iter().flatten().copied().collect();
        let ix = b.u16s(&idx);
        b.json["materials"] = json!([{"name": "Mat", "pbrMetallicRoughness": {"metallicFactor": 0.0}}]);
        b.json["meshes"] = json!([{"name": "Plane", "primitives": [
            {"attributes": {"POSITION": p, "NORMAL": n}, "indices": ix, "material": 0}]}]);
        b.json["nodes"] = json!([
            {"name": "Root", "children": [1, 2]},
            {"name": "Plane", "mesh": 0, "translation": [0.0, 0.0, 0.0]},
            {"name": "Empty"}
        ]);
        b.json["scenes"] = json!([{"name": "Scene", "nodes": [0]}]);
        b.json["scene"] = json!(0);
        b.json["extensionsUsed"] = json!(["B_ext", "A_ext"]);
        b.glb()
    }

    fn canon(bytes: &[u8]) -> Json {
        canonicalize(&load(bytes, None).unwrap()).unwrap()
    }

    #[test]
    fn equal_under_permutation() {
        let a = quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.0, "frap");
        // Vertices reordered, triangles reordered and rotated, other generator.
        // new vertex k holds old vertex order[k]: old 0->new 2, 1->3, 2->0, 3->1
        let b = quad_scene([2, 3, 0, 1], [[1, 2, 0], [0, 2, 3]], 0.0, "Khronos glTF Blender I/O");
        assert_ne!(a, b);
        let (ca, cb) = (canon(&a), canon(&b));
        assert_eq!(compare("glb", &ca, &cb, 1e-5), vec![]);
        // Children order and extension order are irrelevant too.
        let mut d: Json = cb.clone();
        let kids = d["scenes"][0]["nodes"][0]["children"].as_array_mut().unwrap();
        kids.reverse();
        assert_ne!(compare("glb", &ca, &d, 1e-5), vec![]); // raw reversal is visible …
        let e = canon(&b);
        assert_eq!(compare("glb", &ca, &e, 1e-5), vec![]); // … but canonicalisation sorts.
        assert_eq!(ca["extensionsUsed"], json!(["A_ext", "B_ext"]));
        assert!(ca["asset"].get("generator").is_none());
        // Defaults are filled in.
        let mat = &ca["scenes"][0]["nodes"][0]["children"][1]["mesh"]["primitives"][0]["material"];
        assert_eq!(mat["pbrMetallicRoughness"]["roughnessFactor"], json!(1.0));
        assert_eq!(mat["alphaMode"], json!("OPAQUE"));
    }

    #[test]
    fn differs_under_one_change() {
        let a = canon(&quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.0, "x"));
        // One vertex moved.
        let b = canon(&quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.5, "x"));
        assert!(!compare("glb", &a, &b, 1e-5).is_empty());
        // One triangle's winding flipped.
        let c = canon(&quad_scene([0, 1, 2, 3], [[0, 2, 1], [0, 2, 3]], 0.0, "x"));
        assert!(!compare("glb", &a, &c, 1e-5).is_empty());
        // One triangle changed.
        let d = canon(&quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 1, 3]], 0.0, "x"));
        assert!(!compare("glb", &a, &d, 1e-5).is_empty());
    }

    #[test]
    fn strips_and_fans_equal_triangle_lists() {
        let tri = |mode: u64, idx: &[u16]| {
            let mut b = Builder::new();
            let p = b.f32s(
                &[0., 0., 0., 1., 0., 0., 0., 1., 0., 1., 1., 0.],
                "VEC3",
            );
            let ix = b.u16s(idx);
            b.json["meshes"] = json!([{"primitives": [{"attributes": {"POSITION": p}, "indices": ix, "mode": mode}]}]);
            b.json["nodes"] = json!([{"name": "N", "mesh": 0}]);
            b.json["scenes"] = json!([{"nodes": [0]}]);
            let mut c = canon(&b.glb());
            // The mode itself differs; compare the triangle multisets.
            c["scenes"][0]["nodes"][0]["mesh"]["primitives"][0]["elements"].take()
        };
        let list = tri(4, &[0, 1, 2, 2, 1, 3]);
        let strip = tri(5, &[0, 1, 2, 3]);
        assert_eq!(list, strip);
        let fan_list = tri(4, &[0, 1, 3, 0, 3, 2]);
        let fan = tri(6, &[0, 1, 3, 2]);
        assert_eq!(fan_list, fan);
    }

    #[test]
    fn normalized_and_sparse_accessors() {
        let mut b = Builder::new();
        b.bin.extend_from_slice(&[255, 0, 128, 0]);
        b.json["bufferViews"] = json!([{"buffer": 0, "byteOffset": 0, "byteLength": 4}]);
        b.json["accessors"] = json!([{"bufferView": 0, "componentType": 5121, "normalized": true,
                                      "count": 4, "type": "SCALAR"}]);
        let doc = load(&b.glb(), None).unwrap();
        let v = doc.accessor(0).unwrap();
        assert_eq!(v[0][0], 1.0);
        assert_eq!(v[1][0], 0.0);
        assert!((v[2][0] - 128.0 / 255.0).abs() < 1e-12);
        // Sparse on top of zeros.
        let mut s = Builder::new();
        let idx = s.u16s(&[2]);
        let val = s.f32s(&[7.0], "SCALAR");
        let iv = s.json["accessors"][idx]["bufferView"].clone();
        let vv = s.json["accessors"][val]["bufferView"].clone();
        s.json["accessors"] = json!([{"componentType": 5126, "count": 3, "type": "SCALAR",
            "sparse": {"count": 1, "indices": {"bufferView": iv, "componentType": 5123},
                       "values": {"bufferView": vv}}}]);
        let doc = load(&s.glb(), None).unwrap();
        assert_eq!(doc.accessor(0).unwrap(), vec![vec![0.0], vec![0.0], vec![7.0]]);
    }

    #[test]
    fn gltf_json_with_data_uri() {
        let glb = quad_scene([0, 1, 2, 3], [[0, 1, 2], [0, 2, 3]], 0.0, "x");
        let doc = load(&glb, None).unwrap();
        let mut j = doc.json.clone();
        let b64 = {
            const T: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = String::new();
            for c in doc.buffers[0].chunks(3) {
                let n = (c[0] as u32) << 16
                    | (*c.get(1).unwrap_or(&0) as u32) << 8
                    | *c.get(2).unwrap_or(&0) as u32;
                for k in 0..4 {
                    if k <= c.len() {
                        s.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
                    } else {
                        s.push('=');
                    }
                }
            }
            s
        };
        j["buffers"][0]["uri"] = json!(format!("data:application/octet-stream;base64,{b64}"));
        let text = serde_json::to_vec(&j).unwrap();
        let a = canonicalize(&doc).unwrap();
        let b = canonicalize(&load(&text, None).unwrap()).unwrap();
        assert_eq!(compare("gltf", &a, &b, 0.0), vec![]);
    }

    #[test]
    fn base64_round_trip() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(base64_decode("aGk=").unwrap(), b"hi");
        assert!(base64_decode("a!==").is_err());
        assert_eq!(percent_decode("a%20b.bin"), "a b.bin");
    }
}
