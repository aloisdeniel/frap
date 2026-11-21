// SPDX-License-Identifier: GPL-3.0-or-later
//! What the typed model does not hold, kept so that `read → write` gives the file back (WP 7.4,
//! the reader row of deviations §2):
//!
//! - **unknown members**: members of a glTF object that are neither glTF properties nor
//!   `extensions`/`extras` (`"nodes": [{"mesh": 0, "myTag": 3}]`). The Khronos validator
//!   reports them as UNEXPECTED_PROPERTY (a warning) and keeps going; so does
//!   [`crate::validate::validate`].
//! - **unknown enum values**: a value of the right JSON type outside the spec's list
//!   (`"componentType": 5130`, `"alphaMode": "WEIRD"`, `"type": "VEC5"`, a camera `"type"`
//!   other than perspective/orthographic, an interpolation, a primitive `mode`, a buffer view
//!   `target`, sampler filters and wraps). The Khronos validator reports VALUE_NOT_IN_LIST (a
//!   warning); the document loads. The typed member holds the spec default when it has one
//!   (`alphaMode` reads as absent, i.e. OPAQUE; an unknown `mode` as TRIANGLES; an unknown
//!   interpolation as LINEAR), or a stand-in otherwise (`componentType` FLOAT, `type` SCALAR,
//!   camera `perspective`): [`Document::unknown_value`] tells the two apart, the validator
//!   skips the checks that depend on the stand-in, and `read_accessor` refuses an accessor
//!   whose layout is unknown (io_scene_gltf2 fails on it too: `ComponentType.to_numpy_dtype`
//!   raises `KeyError`).
//!
//! Both are recorded on [`Document::unknown`] by the JSON pointer of the object that holds
//! them, in file order, and written back by [`Document::to_vec`] (an unknown enum value
//! replaces its stand-in in place; an unknown member is appended to its object). An entry
//! whose object no longer exists (the document was edited) is dropped on write. A document
//! with entries is written through a `serde_json::Value`, so its `extras` keep their JSON
//! value but not their exact spelling (`1.0` is written `1`).

use serde::de::{self, DeserializeOwned, Visitor};
use serde_json::Value;

use super::{
    Accessor, AccessorType, AlphaMode, Animation, AnimationSampler, Asset, Buffer, BufferView,
    Camera, CameraType, Channel, ChannelTarget, ComponentType, Document, Image, Interpolation,
    MagFilter, Material, Mesh, MinFilter, Mode, Node, NormalTextureInfo, OcclusionTextureInfo,
    Orthographic, PbrMetallicRoughness, Perspective, Primitive, Sampler, Scene, Skin, Sparse,
    SparseIndices, SparseValues, Target, Texture, TextureInfo, WrapMode, extensions::json_eq,
};
use crate::json::root::escape_pointer_token;

/// What an [`UnknownMember`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownKind {
    /// A member the glTF schema does not define for its object (UNEXPECTED_PROPERTY).
    Member,
    /// A known member whose value is not in the spec's list (VALUE_NOT_IN_LIST); `allowed`
    /// lists the valid values as JSON text (`"5120"`, `"\"OPAQUE\""`).
    EnumValue {
        /// The valid values, as JSON text.
        allowed: &'static [&'static str],
    },
}

/// One member kept outside the typed model.
#[derive(Clone, Debug)]
pub struct UnknownMember {
    /// JSON pointer of the object holding the member (`""` for the root, `"/accessors/3"`).
    pub pointer: String,
    /// The member name.
    pub key: String,
    /// The member value, as read.
    pub value: Value,
    /// Unknown member or unknown enum value.
    pub kind: UnknownKind,
}

impl PartialEq for UnknownMember {
    fn eq(&self, o: &UnknownMember) -> bool {
        self.pointer == o.pointer
            && self.key == o.key
            && self.kind == o.kind
            && json_eq(&self.value, &o.value)
    }
}

impl UnknownMember {
    /// JSON pointer of the member itself.
    pub fn member_pointer(&self) -> String {
        format!("{}/{}", self.pointer, escape_pointer_token(&self.key))
    }
}

/// Every member of a read document the typed model does not hold, in file order (empty for a
/// document built in memory).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unknowns(pub Vec<UnknownMember>);

impl Unknowns {
    /// Nothing kept.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Writes every entry into `doc` (the serialized document): an enum value replaces its
    /// stand-in in place, a member is appended to its object. Entries whose object is gone
    /// are skipped.
    pub fn apply(&self, doc: &mut Value) {
        for u in &self.0 {
            let target = if u.pointer.is_empty() {
                Some(&mut *doc)
            } else {
                doc.pointer_mut(&u.pointer)
            };
            if let Some(Value::Object(o)) = target {
                o.insert(u.key.clone(), u.value.clone());
            }
        }
    }
}

impl Document {
    /// The unknown enum value of member `key` of the object at `pointer`
    /// (`doc.unknown_value("/accessors/3", "componentType")`), if the file had one: the typed
    /// member then holds a default or a stand-in (see [`crate::json::unknown`]).
    pub fn unknown_value(&self, pointer: &str, key: &str) -> Option<&Value> {
        self.unknown
            .0
            .iter()
            .find(|u| {
                matches!(u.kind, UnknownKind::EnumValue { .. })
                    && u.pointer == pointer
                    && u.key == key
            })
            .map(|u| &u.value)
    }

    /// Whether the object at `pointer` has any unknown enum value.
    pub fn has_unknown_value(&self, pointer: &str) -> bool {
        self.unknown
            .0
            .iter()
            .any(|u| matches!(u.kind, UnknownKind::EnumValue { .. }) && u.pointer == pointer)
    }

    /// The document as JSON with [`Document::unknown`] written back: what [`Document::to_vec`]
    /// serializes.
    pub fn to_value_with_unknown(&self) -> Value {
        let mut v = self.to_value();
        self.unknown.apply(&mut v);
        v
    }
}

// ------------------------------------------------------------------ field names

/// The member names a derived `Deserialize` impl accepts (its `FIELDS`), read by calling it
/// with a deserializer that only records them.
fn fields_of<T: DeserializeOwned>() -> &'static [&'static str] {
    match T::deserialize(FieldsProbe) {
        Err(Probe(Some(f))) => f,
        _ => &[],
    }
}

#[derive(Debug)]
struct Probe(Option<&'static [&'static str]>);

impl std::fmt::Display for Probe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("field probe")
    }
}
impl std::error::Error for Probe {}
impl de::Error for Probe {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Probe(None)
    }
}

struct FieldsProbe;

impl<'de> de::Deserializer<'de> for FieldsProbe {
    type Error = Probe;
    fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Probe> {
        Err(Probe(None))
    }
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, Probe> {
        Err(Probe(Some(fields)))
    }
    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        option unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier
        ignored_any
    }
}

// ------------------------------------------------------------------ schema

/// An enum-valued member: its name, whether a value is valid, the valid values, and the
/// stand-in the typed model gets for an unknown value (`None`: the member is left out, so it
/// reads as the spec default).
struct EnumMember {
    key: &'static str,
    valid: fn(&Value) -> Option<bool>,
    allowed: fn() -> &'static [&'static str],
    stand_in: Option<fn() -> Value>,
}

/// One glTF object type: its member names, its object-valued members and its enum members.
struct Obj {
    fields: fn() -> &'static [&'static str],
    children: &'static [(&'static str, Child)],
    enums: &'static [EnumMember],
}

#[derive(Clone, Copy)]
enum Child {
    One(&'static Obj),
    Many(&'static Obj),
}

/// `Some(valid)` for an integer (a glTF integer enum), `None` for any other JSON type (a
/// type error the parse reports).
fn valid_u32<T: DeserializeOwned>(v: &Value) -> Option<bool> {
    if v.is_i64() || v.is_u64() {
        Some(T::deserialize(v).is_ok())
    } else {
        None
    }
}

/// `Some(valid)` for a string, `None` otherwise.
fn valid_str<T: DeserializeOwned>(v: &Value) -> Option<bool> {
    v.is_string().then(|| T::deserialize(v).is_ok())
}

macro_rules! allowed {
    ($t:ty, int) => {{
        fn f() -> &'static [&'static str] {
            static S: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
            S.get_or_init(|| {
                <$t>::JSON_VALUES
                    .iter()
                    .map(|v| &*Box::leak(v.to_string().into_boxed_str()))
                    .collect()
            })
        }
        f
    }};
    ($t:ty, str) => {{
        fn f() -> &'static [&'static str] {
            static S: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
            S.get_or_init(|| {
                <$t>::JSON_VALUES
                    .iter()
                    .map(|v| &*Box::leak(format!("{v:?}").into_boxed_str()))
                    .collect()
            })
        }
        f
    }};
}

macro_rules! int_enum {
    ($key:literal, $t:ty, $stand_in:expr) => {
        EnumMember {
            key: $key,
            valid: valid_u32::<$t>,
            allowed: allowed!($t, int),
            stand_in: $stand_in,
        }
    };
}

macro_rules! str_enum_member {
    ($key:literal, $t:ty, $stand_in:expr) => {
        EnumMember {
            key: $key,
            valid: valid_str::<$t>,
            allowed: allowed!($t, str),
            stand_in: $stand_in,
        }
    };
}

const NO_ENUMS: &[EnumMember] = &[];

static TEXTURE_INFO: Obj = Obj {
    fields: fields_of::<TextureInfo>,
    children: &[],
    enums: NO_ENUMS,
};
static NORMAL_TEXTURE_INFO: Obj = Obj {
    fields: fields_of::<NormalTextureInfo>,
    children: &[],
    enums: NO_ENUMS,
};
static OCCLUSION_TEXTURE_INFO: Obj = Obj {
    fields: fields_of::<OcclusionTextureInfo>,
    children: &[],
    enums: NO_ENUMS,
};
static PBR: Obj = Obj {
    fields: fields_of::<PbrMetallicRoughness>,
    children: &[
        ("baseColorTexture", Child::One(&TEXTURE_INFO)),
        ("metallicRoughnessTexture", Child::One(&TEXTURE_INFO)),
    ],
    enums: NO_ENUMS,
};
static MATERIAL: Obj = Obj {
    fields: fields_of::<Material>,
    children: &[
        ("emissiveTexture", Child::One(&TEXTURE_INFO)),
        ("normalTexture", Child::One(&NORMAL_TEXTURE_INFO)),
        ("occlusionTexture", Child::One(&OCCLUSION_TEXTURE_INFO)),
        ("pbrMetallicRoughness", Child::One(&PBR)),
    ],
    enums: &[str_enum_member!("alphaMode", AlphaMode, None)],
};
static ASSET: Obj = Obj {
    fields: fields_of::<Asset>,
    children: &[],
    enums: NO_ENUMS,
};
static SCENE: Obj = Obj {
    fields: fields_of::<Scene>,
    children: &[],
    enums: NO_ENUMS,
};
static NODE: Obj = Obj {
    fields: fields_of::<Node>,
    children: &[],
    enums: NO_ENUMS,
};
static PRIMITIVE: Obj = Obj {
    fields: fields_of::<Primitive>,
    children: &[],
    enums: &[int_enum!("mode", Mode, None)],
};
static MESH: Obj = Obj {
    fields: fields_of::<Mesh>,
    children: &[("primitives", Child::Many(&PRIMITIVE))],
    enums: NO_ENUMS,
};
static SPARSE_INDICES: Obj = Obj {
    fields: fields_of::<SparseIndices>,
    children: &[],
    enums: &[int_enum!(
        "componentType",
        ComponentType,
        Some(|| Value::from(5125u32))
    )],
};
static SPARSE_VALUES: Obj = Obj {
    fields: fields_of::<SparseValues>,
    children: &[],
    enums: NO_ENUMS,
};
static SPARSE: Obj = Obj {
    fields: fields_of::<Sparse>,
    children: &[
        ("indices", Child::One(&SPARSE_INDICES)),
        ("values", Child::One(&SPARSE_VALUES)),
    ],
    enums: NO_ENUMS,
};
static ACCESSOR: Obj = Obj {
    fields: fields_of::<Accessor>,
    children: &[("sparse", Child::One(&SPARSE))],
    enums: &[
        int_enum!(
            "componentType",
            ComponentType,
            Some(|| Value::from(5126u32))
        ),
        str_enum_member!("type", AccessorType, Some(|| Value::from("SCALAR"))),
    ],
};
static BUFFER: Obj = Obj {
    fields: fields_of::<Buffer>,
    children: &[],
    enums: NO_ENUMS,
};
static BUFFER_VIEW: Obj = Obj {
    fields: fields_of::<BufferView>,
    children: &[],
    enums: &[int_enum!("target", Target, None)],
};
static TEXTURE: Obj = Obj {
    fields: fields_of::<Texture>,
    children: &[],
    enums: NO_ENUMS,
};
static IMAGE: Obj = Obj {
    fields: fields_of::<Image>,
    children: &[],
    enums: NO_ENUMS,
};
static SAMPLER: Obj = Obj {
    fields: fields_of::<Sampler>,
    children: &[],
    enums: &[
        int_enum!("magFilter", MagFilter, None),
        int_enum!("minFilter", MinFilter, None),
        int_enum!("wrapS", WrapMode, None),
        int_enum!("wrapT", WrapMode, None),
    ],
};
static PERSPECTIVE: Obj = Obj {
    fields: fields_of::<Perspective>,
    children: &[],
    enums: NO_ENUMS,
};
static ORTHOGRAPHIC: Obj = Obj {
    fields: fields_of::<Orthographic>,
    children: &[],
    enums: NO_ENUMS,
};
static CAMERA: Obj = Obj {
    fields: fields_of::<Camera>,
    children: &[
        ("perspective", Child::One(&PERSPECTIVE)),
        ("orthographic", Child::One(&ORTHOGRAPHIC)),
    ],
    enums: &[str_enum_member!(
        "type",
        CameraType,
        Some(|| Value::from("perspective"))
    )],
};
static SKIN: Obj = Obj {
    fields: fields_of::<Skin>,
    children: &[],
    enums: NO_ENUMS,
};
static CHANNEL_TARGET: Obj = Obj {
    fields: fields_of::<ChannelTarget>,
    children: &[],
    enums: NO_ENUMS,
};
static CHANNEL: Obj = Obj {
    fields: fields_of::<Channel>,
    children: &[("target", Child::One(&CHANNEL_TARGET))],
    enums: NO_ENUMS,
};
static ANIMATION_SAMPLER: Obj = Obj {
    fields: fields_of::<AnimationSampler>,
    children: &[],
    enums: &[str_enum_member!("interpolation", Interpolation, None)],
};
static ANIMATION: Obj = Obj {
    fields: fields_of::<Animation>,
    children: &[
        ("channels", Child::Many(&CHANNEL)),
        ("samplers", Child::Many(&ANIMATION_SAMPLER)),
    ],
    enums: NO_ENUMS,
};
static ROOT: Obj = Obj {
    fields: fields_of::<Document>,
    children: &[
        ("asset", Child::One(&ASSET)),
        ("scenes", Child::Many(&SCENE)),
        ("nodes", Child::Many(&NODE)),
        ("cameras", Child::Many(&CAMERA)),
        ("animations", Child::Many(&ANIMATION)),
        ("materials", Child::Many(&MATERIAL)),
        ("meshes", Child::Many(&MESH)),
        ("textures", Child::Many(&TEXTURE)),
        ("images", Child::Many(&IMAGE)),
        ("skins", Child::Many(&SKIN)),
        ("accessors", Child::Many(&ACCESSOR)),
        ("bufferViews", Child::Many(&BUFFER_VIEW)),
        ("samplers", Child::Many(&SAMPLER)),
        ("buffers", Child::Many(&BUFFER)),
    ],
    enums: NO_ENUMS,
};

// ------------------------------------------------------------------ walks

/// Replaces every unknown enum value of `v` (the parsed file) by its stand-in (or removes
/// it) and returns the replaced values. Values of the wrong JSON type are left for the
/// typed parse to reject.
pub(crate) fn take_unknown_values(v: &mut Value) -> Vec<UnknownMember> {
    let mut out = Vec::new();
    let mut ptr = String::new();
    walk_mut(v, &ROOT, &mut ptr, &mut out);
    out
}

fn walk_mut(v: &mut Value, obj: &Obj, ptr: &mut String, out: &mut Vec<UnknownMember>) {
    let Value::Object(map) = v else { return };
    for e in obj.enums {
        let Some(val) = map.get(e.key) else { continue };
        if (e.valid)(val) != Some(false) {
            continue;
        }
        let value = val.clone();
        match e.stand_in {
            Some(s) => {
                map.insert(e.key.to_owned(), s());
            }
            None => {
                map.shift_remove(e.key);
            }
        }
        out.push(UnknownMember {
            pointer: ptr.clone(),
            key: e.key.to_owned(),
            value,
            kind: UnknownKind::EnumValue {
                allowed: (e.allowed)(),
            },
        });
    }
    for &(key, child) in obj.children {
        let Some(c) = map.get_mut(key) else { continue };
        each_child(c, key, child, ptr, &mut |c, o, ptr| {
            walk_mut(c, o, ptr, out)
        });
    }
}

fn each_child(
    c: &mut Value,
    key: &str,
    child: Child,
    ptr: &mut String,
    f: &mut dyn FnMut(&mut Value, &Obj, &mut String),
) {
    let len = ptr.len();
    ptr.push('/');
    ptr.push_str(key);
    match (child, c) {
        (Child::One(o), c) => f(c, o, ptr),
        (Child::Many(o), Value::Array(items)) => {
            for (i, item) in items.iter_mut().enumerate() {
                let l = ptr.len();
                ptr.push('/');
                ptr.push_str(&i.to_string());
                f(item, o, ptr);
                ptr.truncate(l);
            }
        }
        _ => {}
    }
    ptr.truncate(len);
}

/// The members of `v` (the parsed file, unknown enum values already taken out) that are not
/// glTF properties of their object, in file order.
pub(crate) fn unknown_members(v: &Value) -> Vec<UnknownMember> {
    let mut out = Vec::new();
    let mut ptr = String::new();
    walk(v, &ROOT, &mut ptr, &mut out);
    out
}

fn walk(v: &Value, obj: &Obj, ptr: &mut String, out: &mut Vec<UnknownMember>) {
    let Value::Object(map) = v else { return };
    let fields = (obj.fields)();
    for (k, val) in map {
        if !fields.contains(&k.as_str()) {
            out.push(UnknownMember {
                pointer: ptr.clone(),
                key: k.clone(),
                value: val.clone(),
                kind: UnknownKind::Member,
            });
        }
    }
    for &(key, child) in obj.children {
        let Some(c) = map.get(key) else { continue };
        let len = ptr.len();
        ptr.push('/');
        ptr.push_str(key);
        match (child, c) {
            (Child::One(o), c) => walk(c, o, ptr, out),
            (Child::Many(o), Value::Array(items)) => {
                for (i, item) in items.iter().enumerate() {
                    let l = ptr.len();
                    ptr.push('/');
                    ptr.push_str(&i.to_string());
                    walk(item, o, ptr, out);
                    ptr.truncate(l);
                }
            }
            _ => {}
        }
        ptr.truncate(len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_names_come_from_the_derives() {
        let f = fields_of::<Accessor>();
        for k in [
            "bufferView",
            "componentType",
            "type",
            "extras",
            "extensions",
        ] {
            assert!(f.contains(&k), "{k} missing from {f:?}");
        }
        assert!(fields_of::<Document>().contains(&"bufferViews"));
        assert!(fields_of::<ChannelTarget>().contains(&"path"));
        assert!(fields_of::<TextureInfo>().contains(&"texCoord"));
    }

    #[test]
    fn allowed_lists() {
        assert_eq!(
            allowed!(ComponentType, int)(),
            ["5120", "5121", "5122", "5123", "5125", "5126"]
        );
        assert_eq!(
            allowed!(AlphaMode, str)(),
            ["\"OPAQUE\"", "\"MASK\"", "\"BLEND\""]
        );
    }

    /// Buffer 0: 12 bytes (one float VEC3), as a data URI so `validate_data` can run.
    const BUF: &str = "data:application/octet-stream;base64,AAAAAAAAAAAAAAAA";

    fn parse(v: &Value) -> Document {
        Document::from_slice(&serde_json::to_vec(v).unwrap()).unwrap()
    }

    fn written(doc: &Document) -> Value {
        serde_json::from_slice(&doc.to_vec(&crate::WriteOpts::default())).unwrap()
    }

    fn issues(doc: &Document, code: &str) -> Vec<String> {
        crate::validate::validate(doc)
            .issues
            .iter()
            .filter(|i| i.code == code)
            .map(|i| i.pointer.clone())
            .collect()
    }

    /// The deviations §2 reader row, first half: members that are neither glTF properties
    /// nor `extensions`/`extras` load, are reported as UNEXPECTED_PROPERTY (a warning, as
    /// the Khronos validator does) and are written back.
    #[test]
    fn unknown_members_are_written_back() {
        let src = serde_json::json!({
            "asset": {"version": "2.0", "vendorNote": "x"},
            "rootTag": {"a": [1, 2.5, "three"]},
            "scene": 0,
            "scenes": [{"nodes": [0], "layer": 7}],
            "nodes": [{"mesh": 0, "myTag": 3, "empty": []}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "flag": true}]}],
            "materials": [{"pbrMetallicRoughness": {
                "baseColorTexture": {"index": 0, "uvHint": "main"}, "pbrNote": null}}],
            "textures": [{"source": 0}],
            "images": [{"uri": "a.png"}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3",
                "min": [0, 0, 0], "max": [0, 0, 0], "quantization": {"bits": 16}}],
            "bufferViews": [{"buffer": 0, "byteLength": 12, "viewNote": 1}],
            "buffers": [{"byteLength": 12, "uri": BUF}],
            "cameras": [{"type": "orthographic", "loop": "PINGPONG",
                "orthographic": {"xmag": 1, "ymag": 1, "zfar": 10, "znear": 0.5}}]
        });
        let doc = parse(&src);
        let mut want = vec![
            "/asset/vendorNote",
            "/rootTag",
            "/scenes/0/layer",
            "/nodes/0/myTag",
            "/nodes/0/empty",
            "/meshes/0/primitives/0/flag",
            "/materials/0/pbrMetallicRoughness/pbrNote",
            "/materials/0/pbrMetallicRoughness/baseColorTexture/uvHint",
            "/accessors/0/quantization",
            "/bufferViews/0/viewNote",
            "/cameras/0/loop",
        ];
        let mut got = issues(&doc, "UNEXPECTED_PROPERTY");
        got.sort();
        want.sort_unstable();
        assert_eq!(got, want);
        assert!(crate::validate::validate(&doc).is_ok());
        let out = written(&doc);
        assert!(json_eq(&out, &src), "{out:#}");
        // A second read gives the same document.
        assert_eq!(parse(&out), doc);
    }

    /// The reader row, second half: enum values outside the spec's lists load with a
    /// VALUE_NOT_IN_LIST warning and are written back; the typed member holds the default
    /// or a stand-in.
    #[test]
    fn unknown_enum_values_load_and_are_written_back() {
        let src = serde_json::json!({
            "asset": {"version": "2.0"},
            "cameras": [{"type": "fisheye", "perspective": {"yfov": 1.0, "znear": 0.1}}],
            "materials": [{"alphaMode": "WEIRD"}],
            "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "mode": 9}]}],
            "accessors": [
                {"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3",
                 "min": [0, 0, 0], "max": [0, 0, 0]},
                {"bufferView": 0, "componentType": 5130, "count": 1, "type": "SCALAR"},
                {"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC5"},
                {"bufferView": 0, "componentType": 5126, "count": 1, "type": "SCALAR",
                 "min": [0], "max": [0]}
            ],
            "nodes": [{}],
            "bufferViews": [{"buffer": 0, "byteLength": 12, "target": 1234}],
            "buffers": [{"byteLength": 12, "uri": BUF}],
            "samplers": [{"magFilter": 1, "minFilter": 2, "wrapS": 3, "wrapT": 10497}],
            "animations": [{"channels": [{"sampler": 0, "target": {"node": 0, "path": "translation"}}],
                "samplers": [{"input": 3, "output": 0, "interpolation": "SMOOTH"}]}]
        });
        let doc = parse(&src);
        let mut got = issues(&doc, "VALUE_NOT_IN_LIST");
        got.sort();
        let mut want = vec![
            "/cameras/0/type",
            "/materials/0/alphaMode",
            "/meshes/0/primitives/0/mode",
            "/accessors/1/componentType",
            "/accessors/2/type",
            "/bufferViews/0/target",
            "/samplers/0/magFilter",
            "/samplers/0/minFilter",
            "/samplers/0/wrapS",
            "/animations/0/samplers/0/interpolation",
        ];
        want.sort_unstable();
        assert_eq!(got, want);
        // Defaults where the spec has one, stand-ins otherwise.
        assert_eq!(doc.materials[0].alpha_mode, None);
        assert_eq!(doc.meshes[0].primitives[0].mode, None);
        assert_eq!(doc.animations[0].samplers[0].interpolation, None);
        assert_eq!(doc.cameras[0].type_, CameraType::Perspective);
        assert_eq!(doc.accessors[1].component_type, ComponentType::Float);
        assert_eq!(doc.accessors[2].type_, AccessorType::Scalar);
        assert_eq!(
            doc.unknown_value("/accessors/1", "componentType"),
            Some(&Value::from(5130))
        );
        assert_eq!(doc.unknown_value("/accessors/0", "componentType"), None);
        // Warnings only: the stand-ins raise nothing else.
        let r = crate::validate::validate(&doc);
        assert!(r.is_ok(), "{r}");
        // The accessors without a layout do not decode; the others do.
        let bufs = crate::load_buffers(&doc, None, |_| Err("external".into())).unwrap();
        assert!(crate::read_accessor(&doc, &bufs, 0).is_ok());
        let e = crate::read_accessor(&doc, &bufs, 1)
            .unwrap_err()
            .to_string();
        assert!(e.contains("componentType 5130"), "{e}");
        let e = crate::read_accessor(&doc, &bufs, 2)
            .unwrap_err()
            .to_string();
        assert!(e.contains("type \"VEC5\""), "{e}");
        assert!(crate::validate::validate_data(&doc, &bufs).is_ok());
        // Written back as read.
        let out = written(&doc);
        assert!(json_eq(&out, &src), "{out:#}");
        assert_eq!(parse(&out), doc);
    }

    /// The golden input of `gltf/spec/import_unknown`: frap's validator reports what the
    /// Khronos glTF-Validator 2.0.0-dev.3.10 reports (its UNEXPECTED_PROPERTY and
    /// VALUE_NOT_IN_LIST warnings, copied from its output), and no error; the file is
    /// written back member for member.
    #[test]
    fn agrees_with_the_khronos_validator() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/cases/gltf/spec/import_unknown.inputs/unknown.gltf");
        let bytes = std::fs::read(path).unwrap();
        let khronos = [
            ("UNEXPECTED_PROPERTY", "/appTag"),
            ("UNEXPECTED_PROPERTY", "/asset/vendorNote"),
            ("UNEXPECTED_PROPERTY", "/accessors/0/quantization"),
            ("VALUE_NOT_IN_LIST", "/bufferViews/0/target"),
            ("UNEXPECTED_PROPERTY", "/bufferViews/1/viewNote"),
            ("UNEXPECTED_PROPERTY", "/cameras/0/lensModel"),
            ("VALUE_NOT_IN_LIST", "/cameras/0/type"),
            ("UNEXPECTED_PROPERTY", "/meshes/0/meshNote"),
            ("UNEXPECTED_PROPERTY", "/meshes/0/primitives/0/flag"),
            ("UNEXPECTED_PROPERTY", "/nodes/0/myTag"),
            ("UNEXPECTED_PROPERTY", "/nodes/1/rigTag"),
            ("UNEXPECTED_PROPERTY", "/scenes/0/layer"),
        ];
        let r = crate::validate_file(&bytes).unwrap();
        assert!(r.is_ok(), "{r}");
        let mut got: Vec<(&str, &str)> = r
            .issues
            .iter()
            .filter(|i| matches!(i.code, "UNEXPECTED_PROPERTY" | "VALUE_NOT_IN_LIST"))
            .map(|i| (i.code, i.pointer.as_str()))
            .collect();
        got.sort_unstable();
        let mut want = khronos.to_vec();
        want.sort_unstable();
        assert_eq!(got, want, "{r}");
        let doc = Document::from_slice(&bytes).unwrap();
        let src: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json_eq(&written(&doc), &src));
    }

    /// A value of the wrong JSON type is still a parse error (the validator's TYPE_MISMATCH),
    /// and so is an unknown value next to one.
    #[test]
    fn wrong_types_still_fail() {
        for bad in [
            serde_json::json!({"asset": {"version": "2.0"},
                "accessors": [{"componentType": "FLOAT", "count": 1, "type": "VEC3"}]}),
            serde_json::json!({"asset": {"version": "2.0"},
                "materials": [{"alphaMode": 3}]}),
            serde_json::json!({"asset": {"version": "2.0"},
                "materials": [{"alphaMode": "WEIRD", "doubleSided": "yes"}]}),
        ] {
            let e = Document::from_slice(&serde_json::to_vec(&bad).unwrap());
            assert!(e.is_err(), "{bad}");
        }
    }

    /// Integral floats in integer members still read next to unknown values (both retries).
    #[test]
    fn unknown_values_and_integral_floats() {
        let src = serde_json::json!({"asset": {"version": "2.0"},
            "bufferViews": [{"buffer": 0, "byteLength": 4.0, "target": 7}],
            "buffers": [{"byteLength": 4}]});
        let doc = parse(&src);
        assert_eq!(doc.buffer_views[0].byte_length, 4);
        assert_eq!(doc.buffer_views[0].target, None);
        assert_eq!(
            doc.unknown_value("/bufferViews/0", "target"),
            Some(&Value::from(7))
        );
    }

    /// `extras` keep their raw text when nothing unknown is present, and their value when
    /// something is.
    #[test]
    fn extras_survive_both_write_paths() {
        let text = br#"{"asset":{"version":"2.0"},"nodes":[{"extras":{"w":1.50}}]}"#;
        let doc = Document::from_slice(text).unwrap();
        let out = doc.to_vec(&crate::WriteOpts::default());
        assert!(String::from_utf8_lossy(&out).contains(r#""extras":{"w":1.50}"#));
        let text = br#"{"asset":{"version":"2.0"},"nodes":[{"extras":{"w":1.50},"x":1}]}"#;
        let doc = Document::from_slice(text).unwrap();
        let out: Value = serde_json::from_slice(&doc.to_vec(&crate::WriteOpts::default())).unwrap();
        assert_eq!(out["nodes"][0]["extras"]["w"], 1.5);
        assert_eq!(out["nodes"][0]["x"], 1);
    }
}
