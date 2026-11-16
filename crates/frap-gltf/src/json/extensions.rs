// SPDX-License-Identifier: GPL-3.0-or-later
//! `extensions` and `extras`, present on every glTF object, plus the typed-extension registry.

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::value::RawValue;

use crate::error::GltfError;
use crate::json::Document;
use crate::validate::Report;

/// A typed extension object. One file per extension in `ext/` (§17.1).
///
/// The same extension can have different shapes on different owners (KHR_lights_punctual is
/// a light list on the root and `{light}` on a node); each shape is its own type with the same
/// `NAME`.
pub trait Extension: Serialize + DeserializeOwned {
    /// Extension name as it appears in `extensions` and `extensionsUsed`.
    const NAME: &'static str;
    /// Whether an exporter lists it in `extensionsRequired` when it writes it (io_scene_gltf2's
    /// `Extension(..., required=True)` default, `io/com/gltf2_io_extensions.py`).
    const REQUIRED_BY_DEFAULT: bool = false;
}

/// The `extensions` object of a glTF object: every extension, typed or not, as JSON, in file
/// order. Unknown extensions round-trip untouched. Equality is [`json_eq`] per entry.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Extensions(pub IndexMap<String, Value>);

impl Extensions {
    /// No extension present.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Decodes extension `E` if present.
    pub fn get<E: Extension>(&self) -> Result<Option<E>, GltfError> {
        match self.0.get(E::NAME) {
            None => Ok(None),
            Some(v) => E::deserialize(v)
                .map(Some)
                .map_err(|source| GltfError::Extension {
                    name: E::NAME.to_owned(),
                    source,
                }),
        }
    }

    /// Sets (`Some`) or removes (`None`) extension `E`. Setting keeps the position of an
    /// existing entry; new entries go last.
    pub fn set<E: Extension>(&mut self, e: Option<E>) {
        match e {
            None => {
                self.0.shift_remove(E::NAME);
            }
            Some(e) => {
                let v = serde_json::to_value(&e)
                    .expect("typed glTF extensions serialize to JSON without error");
                self.0.insert(E::NAME.to_owned(), v);
            }
        }
    }

    /// The raw JSON of extension `name`.
    pub fn get_raw(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    /// Inserts raw JSON for extension `name` (replacing any existing value in place).
    pub fn insert_raw(&mut self, name: impl Into<String>, value: Value) {
        self.0.insert(name.into(), value);
    }

    /// Removes extension `name`, keeping the order of the others.
    pub fn remove(&mut self, name: &str) -> Option<Value> {
        self.0.shift_remove(name)
    }

    /// Whether extension `name` is present.
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// Extension names in file order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

impl PartialEq for Extensions {
    fn eq(&self, other: &Extensions) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|((ka, va), (kb, vb))| ka == kb && json_eq(va, vb))
    }
}

/// JSON equality where numbers compare by value (`2` equals `2.0`: JSON numbers carry no
/// integer/float distinction) and object members compare regardless of order.
pub fn json_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            match (x.as_u64(), y.as_u64(), x.as_i64(), y.as_i64()) {
                (Some(p), Some(q), _, _) => p == q,
                (_, _, Some(p), Some(q)) => p == q,
                _ => x.as_f64() == y.as_f64(),
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

/// The `extras` of a glTF object: application JSON kept as raw text, byte for byte.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Extras(pub Box<RawValue>);

impl Extras {
    /// Wraps a JSON value.
    pub fn from_value(v: &Value) -> Extras {
        Extras(serde_json::value::to_raw_value(v).expect("a JSON value serializes"))
    }

    /// Parses JSON text.
    pub fn from_json(text: impl Into<String>) -> Result<Extras, GltfError> {
        Ok(Extras(RawValue::from_string(text.into())?))
    }

    /// The JSON text.
    pub fn get(&self) -> &str {
        self.0.get()
    }

    /// The parsed JSON value.
    pub fn to_value(&self) -> Value {
        serde_json::from_str(self.0.get()).expect("RawValue holds valid JSON")
    }
}

/// Equal when the JSON values are equal (whitespace and number spelling aside).
impl PartialEq for Extras {
    fn eq(&self, other: &Extras) -> bool {
        self.get() == other.get() || json_eq(&self.to_value(), &other.to_value())
    }
}

/// What a typed extension's validator sees: one occurrence of the extension.
#[derive(Clone, Copy, Debug)]
pub struct ExtensionCx<'a> {
    /// The whole document.
    pub doc: &'a Document,
    /// JSON pointer of the object whose `extensions` holds this occurrence (`""` for the root,
    /// `"/nodes/3"`, `"/materials/0/pbrMetallicRoughness/baseColorTexture"`, …).
    pub owner: &'a str,
    /// That object, as JSON.
    pub owner_value: &'a Value,
    /// Extension name.
    pub name: &'a str,
    /// The extension's JSON value.
    pub value: &'a Value,
}

impl ExtensionCx<'_> {
    /// JSON pointer of the extension value itself.
    pub fn pointer(&self) -> String {
        format!(
            "{}/extensions/{}",
            self.owner,
            crate::json::root::escape_pointer_token(self.name)
        )
    }

    /// JSON pointer of member `key` of the extension value.
    pub fn member(&self, key: &str) -> String {
        format!("{}/{key}", self.pointer())
    }

    /// Whether the owner is the root object.
    pub fn owner_is_root(&self) -> bool {
        self.owner.is_empty()
    }

    /// The index of the owner when it is an element of top-level array `collection`
    /// (`"nodes"` matches `/nodes/<i>`).
    pub fn owner_index_in(&self, collection: &str) -> Option<usize> {
        self.owner
            .strip_prefix('/')?
            .strip_prefix(collection)?
            .strip_prefix('/')?
            .parse()
            .ok()
    }

    /// Whether the owner is a texture reference (an object with an integer `index`).
    pub fn owner_is_texture_info(&self) -> bool {
        self.owner_value.get("index").is_some_and(Value::is_u64)
    }

    /// Decodes the value as `E`, reporting `EXTENSION_INVALID` on failure.
    pub fn decode<E: Extension>(&self, r: &mut Report) -> Option<E> {
        match E::deserialize(self.value) {
            Ok(e) => Some(e),
            Err(e) => {
                r.error(
                    "EXTENSION_INVALID",
                    self.pointer(),
                    format!("{}: {e}", self.name),
                );
                None
            }
        }
    }

    /// Reports `EXTENSION_INVALID_LOCATION` (the extension is not defined on this object).
    pub fn wrong_location(&self, r: &mut Report, expected: &str) {
        r.error(
            "EXTENSION_INVALID_LOCATION",
            self.pointer(),
            format!("{} belongs on {expected}", self.name),
        );
    }
}

/// A typed extension's registration (one per file in `ext/`): its name, whether exporters
/// require it by default, and its validator.
#[derive(Debug)]
pub struct ExtensionDef {
    /// Extension name.
    pub name: &'static str,
    /// [`Extension::REQUIRED_BY_DEFAULT`].
    pub required_by_default: bool,
    /// Checks one occurrence and appends issues.
    pub validate: fn(&ExtensionCx, &mut Report),
}

/// Every typed extension this crate knows (rule 2.2.1: files register, nobody edits a list).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static EXTENSIONS: [ExtensionDef];

/// The registered definition of extension `name`, if this crate has a typed model for it.
pub fn extension_def(name: &str) -> Option<&'static ExtensionDef> {
    EXTENSIONS.iter().find(|d| d.name == name)
}
