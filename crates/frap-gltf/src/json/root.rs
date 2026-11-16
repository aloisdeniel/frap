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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::json::extensions::de_extras"
    )]
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
    ///
    /// Integer properties written as integral floats (`"byteLength": 4.0`) are accepted, as
    /// the Khronos validator does (it reports them; so does [`crate::validate::validate_file`]).
    ///
    /// Limits of the model (documented, not round-tripped): members that are neither glTF
    /// properties nor `extensions`/`extras` are dropped (reported as UNEXPECTED_PROPERTY by
    /// [`crate::validate::validate_file`]); integers beyond `u64`/`i64` inside unknown
    /// extensions are kept as the nearest `f64`; enum values outside the spec's lists
    /// (`"type": "VEC5"`, `"componentType": 5130`, `"alphaMode": "WEIRD"`, …) make the parse
    /// fail with an error naming the value.
    pub fn from_slice(json: &[u8]) -> Result<Document, GltfError> {
        let json = trim_json(json);
        match serde_json::from_slice(json) {
            Ok(doc) => Ok(doc),
            Err(strict) => {
                // Retry with integral floats turned into integers; report the strict error
                // when that does not help.
                let Ok(mut v) = serde_json::from_slice::<Value>(json) else {
                    return Err(strict.into());
                };
                if !integralize_floats(&mut v) {
                    return Err(strict.into());
                }
                serde_json::from_value(v).map_err(|_| strict.into())
            }
        }
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

/// `json` without a leading BOM and trailing whitespace / GLB padding.
pub(crate) fn trim_json(json: &[u8]) -> &[u8] {
    let json = json.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(json);
    let end = json
        .iter()
        .rposition(|&b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0))
        .map_or(0, |i| i + 1);
    &json[..end]
}

/// Rewrites every integral float outside `extensions` and `extras` (`4.0`) as an integer
/// (`4`); returns whether anything changed. Float-typed properties read integers fine, so
/// only integer-typed properties are affected in effect.
fn integralize_floats(v: &mut Value) -> bool {
    match v {
        Value::Number(n) if n.is_f64() => {
            let f = n.as_f64().unwrap_or(f64::NAN);
            let int = if f.fract() != 0.0 {
                None
            } else if (0.0..18_446_744_073_709_551_616.0).contains(&f) {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // checked
                let u = f as u64;
                Some(Value::from(u))
            } else if (-9_223_372_036_854_775_808.0..0.0).contains(&f) {
                #[allow(clippy::cast_possible_truncation)] // checked
                let i = f as i64;
                Some(Value::from(i))
            } else {
                None
            };
            int.map(|i| *v = i).is_some()
        }
        Value::Array(items) => items
            .iter_mut()
            .fold(false, |changed, i| integralize_floats(i) | changed),
        Value::Object(map) => map
            .iter_mut()
            .filter(|(k, _)| *k != "extensions" && *k != "extras")
            .fold(false, |changed, (_, i)| integralize_floats(i) | changed),
        _ => false,
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

/// Wraps a serde_json formatter and writes numbers the way io_scene_gltf2's `json.dumps`
/// does: integral floats as integers, like `__fix_json` (`io/exp/export.py`: `int(v)` when
/// `v.is_integer()`; this also avoids the validator's INTEGER_WRITTEN_AS_FLOAT), and every
/// other float as Python's `repr` ([`write_python_repr`]).
struct GltfFormatter<F>(F);

/// Magnitude below which every integral f64 converts to `i64` exactly (and is written as one).
const I64_RANGE: f64 = 9_223_372_036_854_775_808.0; // 2^63

/// Writes a finite float as Python's `repr(float)` (`PyOS_double_to_string(v, 'r', 0,
/// Py_DTSF_ADD_DOT_0)`, CPython `Python/pystrtod.c`): the shortest digits that round-trip,
/// in positional notation when the decimal exponent is in `-4..16`, otherwise as
/// `d.ddde±XX` (sign always, at least two exponent digits: `1e-05`, `1.5e+16`).
fn write_python_repr<W: ?Sized + io::Write>(w: &mut W, v: f64) -> io::Result<()> {
    // `{:e}` is Rust's shortest round-trip form, e.g. `-1.9999999494757503e-5`.
    let sci = format!("{v:e}");
    let (mantissa, exp) = sci.split_once('e').expect("`{:e}` always has an exponent");
    let exp: i32 = exp.parse().expect("`{:e}` writes a decimal exponent");
    if (-4..16).contains(&exp) {
        // Rust's `Display` is positional with the same shortest digits.
        let fixed = format!("{v}");
        w.write_all(fixed.as_bytes())?;
        if !fixed.contains('.') {
            w.write_all(b".0")?;
        }
        Ok(())
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        write!(w, "{mantissa}e{sign}{:02}", exp.unsigned_abs())
    }
}

impl<F: Formatter> Formatter for GltfFormatter<F> {
    fn write_f32<W: ?Sized + io::Write>(&mut self, w: &mut W, v: f32) -> io::Result<()> {
        self.write_f64(w, f64::from(v))
    }

    fn write_f64<W: ?Sized + io::Write>(&mut self, w: &mut W, v: f64) -> io::Result<()> {
        if !v.is_finite() {
            // serde_json writes `null` for NaN and infinities before reaching the formatter;
            // keep its behaviour should that change.
            self.0.write_f64(w, v)
        } else if v.fract() == 0.0 && v.abs() < I64_RANGE {
            #[allow(clippy::cast_possible_truncation)] // integral and in range, checked above
            let i = v as i64;
            self.0.write_i64(w, i)
        } else if v.fract() == 0.0 {
            // Python's `int(v)` is exact at any magnitude; so is `{:.0}` (`1e20` →
            // `100000000000000000000`).
            write!(w, "{v:.0}")
        } else {
            write_python_repr(w, v)
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

#[cfg(test)]
mod tests {
    use super::write_python_repr;

    fn repr(v: f64) -> String {
        let mut out = Vec::new();
        write_python_repr(&mut out, v).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn floats_print_like_python_repr() {
        // Expected strings are `repr(float(...))` in CPython 3.11 (Blender 5.2's Python).
        for (v, want) in [
            (0.5, "0.5"),
            (-1.9999999494757503e-05, "-1.9999999494757503e-05"),
            (1e-7, "1e-07"),
            (1.5e-8, "1.5e-08"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (0.00012345, "0.00012345"),
            (0.800000011920929, "0.800000011920929"),
            (1e16, "1e+16"),
            (1.5e16, "1.5e+16"),
            (1234567890123456.7, "1234567890123456.8"),
            (1e300, "1e+300"),
            (5e-324, "5e-324"),
            (-2.5e-310, "-2.5e-310"),
            (f64::MAX, "1.7976931348623157e+308"),
        ] {
            assert_eq!(repr(v), want, "{v:e}");
        }
    }
}
