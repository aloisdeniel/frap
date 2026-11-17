// SPDX-License-Identifier: GPL-3.0-or-later
//! JSON building blocks of `scene.json` (WP 1.23): shortest-spelling floats, typed readers with
//! error paths, matrices, ID properties, RNA values and socket values.
//!
//! Floats: an `f32` is written with its shortest round-trip spelling (`0.1`, not
//! `0.10000000149011612`): the value goes through `serde_json`'s `f32` writer and is parsed
//! back, so the `f64` held by the tree prints the same digits. Non-finite floats are written as
//! `{"$float": "nan" | "inf" | "-inf"}` (the §13.3 encoding, deviations.md D-14).

use frap_rna::IdProp;
use indexmap::IndexMap;
use serde_json::{Map, Value as J, json};
use smol_str::SmolStr;

use super::MapError;

/// `f32` → JSON number with its shortest spelling (non-finite: `{"$float": …}`).
pub(crate) fn f32j(x: f32) -> J {
    if !x.is_finite() {
        return nonfinite(f64::from(x));
    }
    // serde_json writes an f32 with ryu's f32 shortest form; reading it back as f64 keeps
    // exactly those digits when the tree is printed.
    let s = serde_json::to_string(&x).expect("invariant: finite f32 serializes");
    serde_json::from_str(&s).expect("invariant: serde_json reads its own number")
}

/// `f64` → JSON number (non-finite: `{"$float": …}`).
pub(crate) fn f64j(x: f64) -> J {
    if !x.is_finite() {
        return nonfinite(x);
    }
    json!(x)
}

fn nonfinite(x: f64) -> J {
    let s = if x.is_nan() {
        "nan"
    } else if x > 0.0 {
        "inf"
    } else {
        "-inf"
    };
    json!({"$float": s})
}

/// A slice of `f32` as a JSON array.
pub(crate) fn f32s(v: &[f32]) -> J {
    J::Array(v.iter().map(|x| f32j(*x)).collect())
}

/// Row-major rows of a column-major `glam::Mat4` (RNA reads matrices row-major, §13.3).
pub(crate) fn mat4j(m: &glam::Mat4) -> J {
    let rows = m.transpose().to_cols_array_2d();
    J::Array(rows.iter().map(|r| f32s(r)).collect())
}

/// Any `Serialize` value through `serde_json`'s writer (keeps the shortest `f32` spelling of
/// derived structs, which `serde_json::to_value` would widen).
pub(crate) fn ser<T: serde::Serialize + ?Sized>(v: &T) -> Result<J, MapError> {
    let bytes = serde_json::to_vec(v).map_err(|e| MapError::Invalid {
        path: String::new(),
        message: e.to_string(),
    })?;
    serde_json::from_slice(&bytes).map_err(|e| MapError::Invalid {
        path: String::new(),
        message: e.to_string(),
    })
}

/// Reader of one JSON object with error paths (`objects["Cube"].location`); with `strict`,
/// [`Obj::finish`] fails on keys nobody read.
pub(crate) struct Obj<'a> {
    pub(crate) path: String,
    map: &'a Map<String, J>,
    seen: Vec<&'a str>,
    strict: bool,
}

impl<'a> Obj<'a> {
    pub(crate) fn new(v: &'a J, path: impl Into<String>, strict: bool) -> Result<Self, MapError> {
        let path = path.into();
        match v {
            J::Object(map) => Ok(Obj {
                path,
                map,
                seen: Vec::new(),
                strict,
            }),
            other => Err(MapError::invalid(
                &path,
                format!("expected an object, found {other}"),
            )),
        }
    }
    pub(crate) fn sub(&self, key: &str) -> String {
        format!("{}.{key}", self.path)
    }
    /// The raw value of `key` (marks it read).
    pub(crate) fn opt(&mut self, key: &str) -> Option<&'a J> {
        let (k, v) = self.map.get_key_value(key)?;
        self.seen.push(k.as_str());
        Some(v)
    }
    pub(crate) fn req(&mut self, key: &str) -> Result<&'a J, MapError> {
        let p = self.sub(key);
        self.opt(key)
            .ok_or_else(|| MapError::invalid(&p, "missing"))
    }
    pub(crate) fn obj(&mut self, key: &str) -> Result<Obj<'a>, MapError> {
        let p = self.sub(key);
        let strict = self.strict;
        Obj::new(self.req(key)?, p, strict)
    }
    pub(crate) fn bool(&mut self, key: &str, out: &mut bool) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = v
                .as_bool()
                .ok_or_else(|| MapError::invalid(&self.sub(key), "expected a boolean"))?;
        }
        Ok(())
    }
    pub(crate) fn i32(&mut self, key: &str, out: &mut i32) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = int(v, &self.sub(key))?;
        }
        Ok(())
    }
    pub(crate) fn u32(&mut self, key: &str, out: &mut u32) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            let p = self.sub(key);
            *out = v
                .as_u64()
                .and_then(|x| u32::try_from(x).ok())
                .ok_or_else(|| MapError::invalid(&p, "expected an unsigned integer"))?;
        }
        Ok(())
    }
    pub(crate) fn f32(&mut self, key: &str, out: &mut f32) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = num(v, &self.sub(key))?;
        }
        Ok(())
    }
    pub(crate) fn smol(&mut self, key: &str, out: &mut SmolStr) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = SmolStr::new(string(v, &self.sub(key))?);
        }
        Ok(())
    }
    pub(crate) fn string(&mut self, key: &str, out: &mut String) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = string(v, &self.sub(key))?.to_owned();
        }
        Ok(())
    }
    pub(crate) fn opt_smol(
        &mut self,
        key: &str,
        out: &mut Option<SmolStr>,
    ) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = match v {
                J::Null => None,
                v => Some(SmolStr::new(string(v, &self.sub(key))?)),
            };
        }
        Ok(())
    }
    pub(crate) fn arr<const N: usize>(
        &mut self,
        key: &str,
        out: &mut [f32; N],
    ) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = farr(v, &self.sub(key))?;
        }
        Ok(())
    }
    pub(crate) fn iarr<const N: usize>(
        &mut self,
        key: &str,
        out: &mut [i32; N],
    ) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            let p = self.sub(key);
            let a = array(v, &p)?;
            if a.len() != N {
                return Err(MapError::invalid(&p, format!("expected {N} integers")));
            }
            for (o, x) in out.iter_mut().zip(a) {
                *o = int(x, &p)?;
            }
        }
        Ok(())
    }
    pub(crate) fn mat4(&mut self, key: &str, out: &mut glam::Mat4) -> Result<(), MapError> {
        if let Some(v) = self.opt(key) {
            *out = mat4(v, &self.sub(key))?;
        }
        Ok(())
    }
    /// Every key not read yet (marked read), for the generated structs that deserialize the
    /// rest of an object.
    pub(crate) fn rest(&mut self) -> Map<String, J> {
        let mut out = Map::new();
        for (k, v) in self.map {
            if !self.seen.contains(&k.as_str()) {
                self.seen.push(k.as_str());
                out.insert(k.clone(), v.clone());
            }
        }
        out
    }
    /// Fails (strict mode) on keys that were never read.
    pub(crate) fn finish(self) -> Result<(), MapError> {
        if !self.strict {
            return Ok(());
        }
        if let Some(k) = self.map.keys().find(|k| !self.seen.contains(&k.as_str())) {
            return Err(MapError::Unknown(format!("{}.{k}", self.path)));
        }
        Ok(())
    }
}

pub(crate) fn int(v: &J, path: &str) -> Result<i32, MapError> {
    v.as_i64()
        .and_then(|x| i32::try_from(x).ok())
        .ok_or_else(|| MapError::invalid(path, format!("expected a 32-bit integer, found {v}")))
}

pub(crate) fn num(v: &J, path: &str) -> Result<f32, MapError> {
    Ok(num64(v, path)? as f32)
}

pub(crate) fn num64(v: &J, path: &str) -> Result<f64, MapError> {
    if let Some(x) = v.as_f64() {
        return Ok(x);
    }
    if let Some(s) = v.get("$float").and_then(J::as_str) {
        return match s {
            "nan" => Ok(f64::NAN),
            "inf" => Ok(f64::INFINITY),
            "-inf" => Ok(f64::NEG_INFINITY),
            _ => Err(MapError::invalid(path, format!("bad $float {s:?}"))),
        };
    }
    Err(MapError::invalid(
        path,
        format!("expected a number, found {v}"),
    ))
}

pub(crate) fn string<'a>(v: &'a J, path: &str) -> Result<&'a str, MapError> {
    v.as_str()
        .ok_or_else(|| MapError::invalid(path, format!("expected a string, found {v}")))
}

pub(crate) fn array<'a>(v: &'a J, path: &str) -> Result<&'a Vec<J>, MapError> {
    v.as_array()
        .ok_or_else(|| MapError::invalid(path, format!("expected an array, found {v}")))
}

pub(crate) fn farr<const N: usize>(v: &J, path: &str) -> Result<[f32; N], MapError> {
    let a = array(v, path)?;
    if a.len() != N {
        return Err(MapError::invalid(
            path,
            format!("expected {N} numbers, found {}", a.len()),
        ));
    }
    let mut out = [0.0; N];
    for (o, x) in out.iter_mut().zip(a) {
        *o = num(x, path)?;
    }
    Ok(out)
}

pub(crate) fn mat4(v: &J, path: &str) -> Result<glam::Mat4, MapError> {
    let rows = array(v, path)?;
    if rows.len() != 4 {
        return Err(MapError::invalid(path, "expected 4 rows"));
    }
    let mut r = [[0.0f32; 4]; 4];
    for (o, row) in r.iter_mut().zip(rows) {
        *o = farr(row, path)?;
    }
    Ok(glam::Mat4::from_cols_array_2d(&r).transpose())
}

/// Deserializes a derived (generated plain) struct; with `strict`, keys the struct does not
/// know fail (the generated structs use `#[serde(default)]` and would ignore them).
pub(crate) fn de<T: serde::Serialize + serde::de::DeserializeOwned>(
    v: &J,
    path: &str,
    strict: bool,
) -> Result<T, MapError> {
    let t: T =
        serde_json::from_value(v.clone()).map_err(|e| MapError::invalid(path, e.to_string()))?;
    if strict {
        let back = ser(&t)?;
        unknown_keys(v, &back, path)?;
    }
    Ok(t)
}

fn unknown_keys(given: &J, known: &J, path: &str) -> Result<(), MapError> {
    if let (J::Object(g), J::Object(k)) = (given, known) {
        for (key, gv) in g {
            match k.get(key) {
                None => return Err(MapError::Unknown(format!("{path}.{key}"))),
                Some(kv) => unknown_keys(gv, kv, &format!("{path}.{key}"))?,
            }
        }
    }
    Ok(())
}

// --- ID properties (`ID["prop"]`) ---------------------------------------------------------

/// `IdProp` → JSON: numbers, booleans, strings, objects (groups) and arrays (lists) as they
/// are; typed arrays tagged (`{"$int_array": […]}`, `{"$float_array": […]}`), floats always with
/// a fraction or exponent so they read back as floats.
pub(crate) fn idprop_j(p: &IdProp) -> J {
    match p {
        IdProp::Int(i) => json!(i),
        IdProp::Float(f) => float_mark(*f),
        IdProp::Bool(b) => json!(b),
        IdProp::Str(s) => json!(s.as_str()),
        IdProp::IntArray(v) => json!({"$int_array": v}),
        IdProp::FloatArray(v) => {
            json!({"$float_array": v.iter().map(|x| float_mark(*x)).collect::<Vec<_>>()})
        }
        IdProp::Group(g) => J::Object(
            g.iter()
                .map(|(k, v)| (k.to_string(), idprop_j(v)))
                .collect(),
        ),
        IdProp::List(l) => J::Array(l.iter().map(idprop_j).collect()),
        _ => J::Null,
    }
}

/// A float that stays a float in JSON (`serde_json` prints `1.0`, never `1`).
fn float_mark(f: f64) -> J {
    f64j(f)
}

pub(crate) fn idprop_from(v: &J, path: &str) -> Result<IdProp, MapError> {
    Ok(match v {
        J::Bool(b) => IdProp::Bool(*b),
        J::Number(n) if n.is_f64() => IdProp::Float(n.as_f64().unwrap_or(0.0)),
        J::Number(n) => IdProp::Int(
            n.as_i64()
                .ok_or_else(|| MapError::invalid(path, "integer out of range"))?,
        ),
        J::String(s) => IdProp::Str(SmolStr::new(s)),
        J::Array(a) => IdProp::List(
            a.iter()
                .enumerate()
                .map(|(i, x)| idprop_from(x, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        J::Object(m) if m.len() == 1 && m.contains_key("$float") => IdProp::Float(num64(v, path)?),
        J::Object(m) if m.len() == 1 && m.contains_key("$int_array") => IdProp::IntArray(
            array(&m["$int_array"], path)?
                .iter()
                .map(|x| {
                    x.as_i64()
                        .ok_or_else(|| MapError::invalid(path, "expected integers"))
                })
                .collect::<Result<_, _>>()?,
        ),
        J::Object(m) if m.len() == 1 && m.contains_key("$float_array") => IdProp::FloatArray(
            array(&m["$float_array"], path)?
                .iter()
                .map(|x| num64(x, path))
                .collect::<Result<_, _>>()?,
        ),
        J::Object(m) => IdProp::Group(
            m.iter()
                .map(|(k, x)| Ok((SmolStr::new(k), idprop_from(x, &format!("{path}[{k:?}]"))?)))
                .collect::<Result<IndexMap<_, _>, MapError>>()?,
        ),
        J::Null => return Err(MapError::invalid(path, "null ID property")),
    })
}
