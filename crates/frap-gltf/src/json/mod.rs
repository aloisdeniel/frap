// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF JSON model (`Document`, `Extensions`, `Extension`), one file per top-level object
//! (`root`, `asset`, `scene`, `node`, `mesh`, `accessor`, `buffer`, `material`, `texture`,
//! `image`, `sampler`, `camera`, `skin`, `animation`; §17.1).
//!
//! Conventions shared by every object:
//! - Properties with a spec default are `Option`s, so a file that spells out a default
//!   (`"mode": 4`) is written back the same way; `*_or_default()` helpers give the effective
//!   value.
//! - Non-integer numbers are `f64`: values parsed from a file are kept exactly
//!   (`float_roundtrip`), and f32 values widened by an exporter print like Python's `repr`
//!   (`0.800000011920929`, as Blender writes them).
//! - `extensions` keeps every extension as a JSON value ([`Extensions`]); typed access goes
//!   through [`Extension`]. `extras` keeps the raw JSON text ([`Extras`]).
//! - Empty arrays are omitted on write (the spec forbids empty arrays).
//! - Members that are neither glTF properties nor `extensions`/`extras`, and enum values
//!   outside the spec's lists, are kept in [`Document::unknown`] and written back
//!   ([`unknown`], WP 7.4). Not kept: the exact digits of integers beyond 64 bits inside
//!   extension values (read as the nearest `f64`). `"extras": null` is kept.

pub mod accessor;
pub mod animation;
pub mod asset;
pub mod buffer;
pub mod camera;
pub mod extensions;
pub mod image;
pub mod material;
pub mod mesh;
pub mod node;
pub mod root;
pub mod sampler;
pub mod scene;
pub mod skin;
pub mod texture;
pub mod unknown;

pub use accessor::{Accessor, AccessorType, ComponentType, Sparse, SparseIndices, SparseValues};
pub use animation::{
    Animation, AnimationPath, AnimationSampler, Channel, ChannelTarget, Interpolation,
};
pub use asset::Asset;
pub use buffer::{Buffer, BufferView, Target};
pub use camera::{Camera, CameraType, Orthographic, Perspective};
pub use extensions::{
    EXTENSIONS, Extension, ExtensionCx, ExtensionDef, Extensions, Extras, extension_def, json_eq,
};
pub use image::Image;
pub use material::{AlphaMode, Material, PbrMetallicRoughness};
pub use mesh::{Mesh, Mode, Primitive};
pub use node::Node;
pub use root::Document;
pub use sampler::{MagFilter, MinFilter, Sampler, WrapMode};
pub use scene::Scene;
pub use skin::Skin;
pub use texture::{NormalTextureInfo, OcclusionTextureInfo, Texture, TextureInfo};
pub use unknown::{UnknownKind, UnknownMember, Unknowns};

/// Index into one of the document's top-level arrays.
pub type Index = u32;

/// Declares a `u32`-coded enum (componentType, mode, target, filters) with exact serde.
macro_rules! gl_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $var:ident = $val:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "u32", into = "u32")]
        pub enum $name { $($(#[$vm])* $var = $val),+ }
        impl From<$name> for u32 {
            fn from(v: $name) -> u32 { v as u32 }
        }
        impl $name {
            /// Every value the spec allows, as written in JSON.
            pub const JSON_VALUES: &'static [u32] = &[$($val),+];
        }
        impl TryFrom<u32> for $name {
            type Error = String;
            fn try_from(v: u32) -> Result<Self, String> {
                match v {
                    $($val => Ok($name::$var),)+
                    _ => Err(format!(concat!("invalid ", stringify!($name), " {}"), v)),
                }
            }
        }
    };
}
pub(crate) use gl_enum;

/// Declares a string-coded enum (accessor type, alphaMode, interpolation, …) with exact serde.
macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $var:ident = $val:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub enum $name { $($(#[$vm])* #[serde(rename = $val)] $var),+ }
        impl $name {
            /// Every value the spec allows, as written in JSON.
            pub const JSON_VALUES: &'static [&'static str] = &[$($val),+];
            /// The JSON spelling.
            pub fn as_str(self) -> &'static str {
                match self { $($name::$var => $val),+ }
            }
        }
    };
}
pub(crate) use str_enum;
