// SPDX-License-Identifier: GPL-3.0-or-later
//! The glTF root object ([`Document`]) and JSON text I/O.

use std::io;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::ser::{CompactFormatter, Formatter, PrettyFormatter};

use super::{
    Accessor, Animation, Asset, Buffer, BufferView, Camera, Extensions, Extras, Image, Index,
    Material, Mesh, Node, Sampler, Scene, Skin, Texture,
};
use crate::error::GltfError;
use crate::glb::WriteOpts;

/// A glTF 2.0 document.
///
/// Fields are declared in the order io_scene_gltf2 writes top-level keys
/// (`io/exp/export.py`, `__fix_json` / key sort), so serialization reproduces Blender's
/// layout without a post-pass.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    /// Version and provenance.
    pub asset: Asset,
    /// Every extension used anywhere in the document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_used: Vec<String>,
    /// Extensions a loader must support.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions_required: Vec<String>,
    /// Root extensions (KHR_lights_punctual lights, KHR_materials_variants variants, …).
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Default scene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<Index>,
    /// Scenes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    /// Nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<Node>,
    /// Cameras.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<Camera>,
    /// Animations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<Animation>,
    /// Materials.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materials: Vec<Material>,
    /// Meshes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meshes: Vec<Mesh>,
    /// Textures.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<Texture>,
    /// Images.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    /// Skins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skins: Vec<Skin>,
    /// Accessors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accessors: Vec<Accessor>,
    /// Buffer views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffer_views: Vec<BufferView>,
    /// Samplers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samplers: Vec<Sampler>,
    /// Buffers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buffers: Vec<Buffer>,
}

impl Document {
    /// Parses glTF JSON text (UTF-8; a leading BOM is tolerated, trailing whitespace and GLB
    /// padding are ignored).
    pub fn from_slice(json: &[u8]) -> Result<Document, GltfError> {
        let json = json.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(json);
        let end = json
            .iter()
            .rposition(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0))
            .map_or(0, |i| i + 1);
        Ok(serde_json::from_slice(&json[..end])?)
    }

    /// Serializes to JSON text (compact unless `opts.indent` is set). Integral numbers are
    /// written without a fraction (`1` rather than `1.0`), as io_scene_gltf2's `__fix_json`
    /// does.
    pub fn to_vec(&self, opts: &WriteOpts) -> Vec<u8> {
        let mut out = Vec::new();
        let res = match &opts.indent {
            None => {
                let mut ser = serde_json::Serializer::with_formatter(
                    &mut out,
                    GltfFormatter(CompactFormatter),
                );
                self.serialize(&mut ser)
            }
            Some(indent) => {
                let mut ser = serde_json::Serializer::with_formatter(
                    &mut out,
                    GltfFormatter(PrettyFormatter::with_indent(indent.as_bytes())),
                );
                self.serialize(&mut ser)
            }
        };
        res.expect("a glTF document always serializes to JSON");
        out
    }

    /// The document as a JSON value.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).expect("a glTF document always serializes to JSON")
    }

    /// Lists `name` in `extensionsUsed` (and `extensionsRequired` when `required`), once.
    pub fn use_extension(&mut self, name: &str, required: bool) {
        if !self.extensions_used.iter().any(|n| n == name) {
            self.extensions_used.push(name.to_owned());
        }
        if required && !self.extensions_required.iter().any(|n| n == name) {
            self.extensions_required.push(name.to_owned());
        }
    }

    /// Calls `f(owner, owner_value, name, value)` for every extension object anywhere in the
    /// document (root, nodes, texture infos, extension-nested texture infos, …), in document
    /// order. `owner` is the JSON pointer of the object holding the `extensions` member (`""`
    /// for the root) and `owner_value` that object. `extras` are never searched.
    pub fn for_each_extension(&self, mut f: impl FnMut(&str, &Value, &str, &Value)) {
        let v = self.to_value();
        let mut ptr = String::new();
        walk_extensions(&v, &mut ptr, &mut f);
    }

    /// Names of the extensions present anywhere in the document, in first-occurrence order.
    pub fn extensions_present(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        self.for_each_extension(|_, _, name, _| {
            if !names.iter().any(|n| n == name) {
                names.push(name.to_owned());
            }
        });
        names
    }
}

/// Escapes one JSON-pointer reference token (RFC 6901).
pub(crate) fn escape_pointer_token(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn walk_extensions(v: &Value, ptr: &mut String, f: &mut impl FnMut(&str, &Value, &str, &Value)) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                if k == "extras" {
                    continue;
                }
                if k == "extensions" {
                    if let Value::Object(exts) = child {
                        for (name, ev) in exts {
                            f(ptr, v, name, ev);
                        }
                        // Extension values can hold texture infos with their own extensions.
                        for (name, ev) in exts {
                            let len = ptr.len();
                            ptr.push_str("/extensions/");
                            ptr.push_str(&escape_pointer_token(name));
                            walk_extensions(ev, ptr, f);
                            ptr.truncate(len);
                        }
                    }
                    continue;
                }
                let len = ptr.len();
                ptr.push('/');
                ptr.push_str(&escape_pointer_token(k));
                walk_extensions(child, ptr, f);
                ptr.truncate(len);
            }
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                let len = ptr.len();
                ptr.push('/');
                ptr.push_str(&i.to_string());
                walk_extensions(child, ptr, f);
                ptr.truncate(len);
            }
        }
        _ => {}
    }
}

/// Wraps a serde_json formatter and writes integral floats as integers, like
/// io_scene_gltf2's `__fix_json` (`io/exp/export.py`: `int(v)` when `v.is_integer()`); this
/// also avoids the validator's INTEGER_WRITTEN_AS_FLOAT. Every other float keeps serde_json's
/// shortest round-trip form, which equals Python's `repr`.
struct GltfFormatter<F>(F);

/// Largest magnitude below which every integral f64 converts to `i64` exactly.
const MAX_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53

impl<F: Formatter> Formatter for GltfFormatter<F> {
    fn write_f32<W: ?Sized + io::Write>(&mut self, w: &mut W, v: f32) -> io::Result<()> {
        self.write_f64(w, f64::from(v))
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, w: &mut W, v: f64) -> io::Result<()> {
        if v.is_finite() && v.fract() == 0.0 && v.abs() < MAX_SAFE_INT {
            #[allow(clippy::cast_possible_truncation)] // checked above
            let i = v as i64;
            self.0.write_i64(w, i)
        } else {
            self.0.write_f64(w, v)
        }
    }

    fn begin_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_array(w)
    }
    fn end_array<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array(w)
    }
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_array_value(w, first)
    }
    fn end_array_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_array_value(w)
    }
    fn begin_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.begin_object(w)
    }
    fn end_object<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object(w)
    }
    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        self.0.begin_object_key(w, first)
    }
    fn end_object_key<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object_key(w)
    }
    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        // `"key":value` in both modes: io_scene_gltf2 passes `separators=(',', ':')` even
        // when it indents `.gltf` output (`io/exp/export.py`).
        w.write_all(b":")
    }
    fn end_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        self.0.end_object_value(w)
    }
    fn write_raw_fragment<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        self.0.write_raw_fragment(w, fragment)
    }
}
