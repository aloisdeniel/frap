// SPDX-License-Identifier: GPL-3.0-or-later
//! Element types of `.frap` arrays: the `dtype` of a `{"$buf", "dtype", "len"}` reference
//! (`"i32"`, `"f32x3"`, `"bool"`, …) and the little-endian / JSON codecs of each scalar.

use std::fmt;
use std::str::FromStr;

use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

/// Scalar component type. `bool` is stored as one byte (0 or 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ScalarType {
    Bool,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl ScalarType {
    /// Every scalar type, in declaration order.
    pub const ALL: [Self; 11] = [
        Self::Bool,
        Self::I8,
        Self::U8,
        Self::I16,
        Self::U16,
        Self::I32,
        Self::U32,
        Self::I64,
        Self::U64,
        Self::F32,
        Self::F64,
    ];

    /// The spelling used in `dtype`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    /// Size of one scalar in bytes.
    #[must_use]
    pub const fn size(self) -> usize {
        match self {
            Self::Bool | Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }

    fn from_name(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == s)
    }
}

/// Element type of an array: a scalar type and a lane count (`f32x3` = 3 × `f32` per element).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Dtype {
    pub scalar: ScalarType,
    /// Scalars per element, `1..=MAX_LANES`. One lane is spelled without the `x1` suffix.
    pub lanes: u16,
}

impl Dtype {
    /// Largest lane count (a 4×4 matrix is `f32x16`).
    pub const MAX_LANES: u16 = 256;

    /// `scalar` × `lanes`. Panics if `lanes` is 0 or above [`Self::MAX_LANES`].
    #[must_use]
    pub const fn new(scalar: ScalarType, lanes: u16) -> Self {
        assert!(
            lanes >= 1 && lanes <= Self::MAX_LANES,
            "dtype lanes out of range"
        );
        Self { scalar, lanes }
    }

    /// Bytes per element.
    #[must_use]
    pub const fn item_size(self) -> usize {
        self.scalar.size() * self.lanes as usize
    }
}

/// A string that is not a valid `dtype`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid dtype {0:?}")]
pub struct ParseDtypeError(pub String);

impl FromStr for Dtype {
    type Err = ParseDtypeError;

    /// Canonical spellings only: `"f32"`, `"f32x3"`; `"f32x1"`, `"f32x03"` and `"f32x0"` are rejected.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseDtypeError(s.to_owned());
        let (scalar, lanes) = match s.split_once('x') {
            None => (s, 1),
            Some((scalar, lanes)) => {
                if lanes.is_empty()
                    || lanes.starts_with('0')
                    || !lanes.bytes().all(|b| b.is_ascii_digit())
                {
                    return Err(err());
                }
                let lanes: u16 = lanes.parse().map_err(|_| err())?;
                if !(2..=Self::MAX_LANES).contains(&lanes) {
                    return Err(err());
                }
                (scalar, lanes)
            }
        };
        Ok(Self {
            scalar: ScalarType::from_name(scalar).ok_or_else(err)?,
            lanes,
        })
    }
}

impl fmt::Display for Dtype {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.lanes == 1 {
            f.write_str(self.scalar.name())
        } else {
            write!(f, "{}x{}", self.scalar.name(), self.lanes)
        }
    }
}

impl Serialize for Dtype {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Dtype {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A scalar that `.frap` arrays can hold. Sealed: the set matches [`ScalarType`].
pub trait Scalar: Copy + PartialEq + fmt::Debug + Send + Sync + sealed::Sealed + 'static {
    const TYPE: ScalarType;

    /// Appends the little-endian bytes.
    fn write_le(self, out: &mut Vec<u8>);
    /// Reads from exactly `TYPE.size()` little-endian bytes; `None` for an invalid `bool` byte.
    fn read_le(bytes: &[u8]) -> Option<Self>;
    /// Serializes the scalar as an inline JSON value (`f32` uses the shortest round-trip form).
    fn serialize_json<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error>;
    /// Reads an inline JSON value; `None` if the type or range does not fit.
    fn from_json(v: &Value) -> Option<Self>;
    /// Whether the value can be written inline (JSON has no NaN or infinity).
    fn json_representable(self) -> bool {
        true
    }
}

macro_rules! int_scalar {
    ($($t:ty => $variant:ident, $ser:ident, $get:ident;)*) => {$(
        impl sealed::Sealed for $t {}
        impl Scalar for $t {
            const TYPE: ScalarType = ScalarType::$variant;
            fn write_le(self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn read_le(bytes: &[u8]) -> Option<Self> {
                Some(<$t>::from_le_bytes(bytes.try_into().ok()?))
            }
            fn serialize_json<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
                s.$ser(self)
            }
            fn from_json(v: &Value) -> Option<Self> {
                v.$get().and_then(|x| <$t>::try_from(x).ok())
            }
        }
    )*};
}

int_scalar! {
    i8 => I8, serialize_i8, as_i64;
    u8 => U8, serialize_u8, as_u64;
    i16 => I16, serialize_i16, as_i64;
    u16 => U16, serialize_u16, as_u64;
    i32 => I32, serialize_i32, as_i64;
    u32 => U32, serialize_u32, as_u64;
    i64 => I64, serialize_i64, as_i64;
    u64 => U64, serialize_u64, as_u64;
}

impl sealed::Sealed for bool {}
impl Scalar for bool {
    const TYPE: ScalarType = ScalarType::Bool;
    fn write_le(self, out: &mut Vec<u8>) {
        out.push(u8::from(self));
    }
    fn read_le(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [0] => Some(false),
            [1] => Some(true),
            _ => None,
        }
    }
    fn serialize_json<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bool(self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        v.as_bool()
    }
}

impl sealed::Sealed for f32 {}
impl Scalar for f32 {
    const TYPE: ScalarType = ScalarType::F32;
    fn write_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
    fn read_le(bytes: &[u8]) -> Option<Self> {
        Some(f32::from_le_bytes(bytes.try_into().ok()?))
    }
    fn serialize_json<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f32(self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        // The shortest f32 spelling parsed as f64 and narrowed gives back the same f32;
        // values outside the f32 range are rejected rather than turned into infinities.
        #[allow(clippy::cast_possible_truncation)]
        let x = v.as_f64()? as f32;
        x.is_finite().then_some(x)
    }
    fn json_representable(self) -> bool {
        self.is_finite()
    }
}

impl sealed::Sealed for f64 {}
impl Scalar for f64 {
    const TYPE: ScalarType = ScalarType::F64;
    fn write_le(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
    fn read_le(bytes: &[u8]) -> Option<Self> {
        Some(f64::from_le_bytes(bytes.try_into().ok()?))
    }
    fn serialize_json<S: Serializer>(self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(self)
    }
    fn from_json(v: &Value) -> Option<Self> {
        v.as_f64()
    }
    fn json_representable(self) -> bool {
        self.is_finite()
    }
}

/// An array element: a [`Scalar`] or a fixed-size array of one (`[f32; 3]` is `f32x3`).
pub trait Element: Copy + fmt::Debug + Send + Sync + 'static {
    type Scalar: Scalar;
    /// Scalars per element.
    const LANES: u16;

    /// The element's `dtype`.
    #[must_use]
    fn dtype() -> Dtype {
        Dtype::new(Self::Scalar::TYPE, Self::LANES)
    }
    /// The element's scalars, in order.
    fn scalars(&self) -> &[Self::Scalar];
    /// Builds an element from exactly `LANES` scalars.
    fn from_scalars(s: &[Self::Scalar]) -> Self;
    /// Serializes the element inline: a scalar, or a JSON array of `LANES` scalars.
    fn serialize_json<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl<T: Scalar> Element for T {
    type Scalar = T;
    const LANES: u16 = 1;

    fn scalars(&self) -> &[T] {
        std::slice::from_ref(self)
    }
    fn from_scalars(s: &[T]) -> Self {
        s[0]
    }
    fn serialize_json<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        Scalar::serialize_json(*self, s)
    }
}

impl<T: Scalar, const N: usize> Element for [T; N] {
    type Scalar = T;
    const LANES: u16 = {
        assert!(
            N >= 2 && N <= Dtype::MAX_LANES as usize,
            "element arrays need 2..=256 lanes"
        );
        #[allow(clippy::cast_possible_truncation)]
        let lanes = N as u16;
        lanes
    };

    fn scalars(&self) -> &[T] {
        self
    }
    fn from_scalars(s: &[T]) -> Self {
        std::array::from_fn(|i| s[i])
    }
    fn serialize_json<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        struct One<T>(T);
        impl<T: Scalar> Serialize for One<T> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                self.0.serialize_json(s)
            }
        }
        let mut t = s.serialize_tuple(N)?;
        for x in self {
            t.serialize_element(&One(*x))?;
        }
        t.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dtype_spellings() {
        for t in ScalarType::ALL {
            let d = Dtype::new(t, 1);
            assert_eq!(d.to_string(), t.name());
            assert_eq!(d.to_string().parse::<Dtype>(), Ok(d));
            let d3 = Dtype::new(t, 3);
            assert_eq!(d3.to_string(), format!("{}x3", t.name()));
            assert_eq!(d3.to_string().parse::<Dtype>(), Ok(d3));
        }
        assert_eq!(<[f32; 3]>::dtype().to_string(), "f32x3");
        assert_eq!(<[f32; 16]>::dtype().item_size(), 64);
        assert_eq!(<bool>::dtype().to_string(), "bool");
        assert_eq!(<[i32; 2]>::dtype().to_string(), "i32x2");
        for bad in [
            "f32x1", "f32x0", "f32x03", "f32x", "f32x257", "f16", "F32", "", "x3", "f32x3x2",
            "f32x+3",
        ] {
            assert!(bad.parse::<Dtype>().is_err(), "{bad} accepted");
        }
    }

    #[test]
    fn json_ranges() {
        assert_eq!(
            <u8 as Scalar>::from_json(&serde_json::json!(255)),
            Some(255)
        );
        assert_eq!(<u8 as Scalar>::from_json(&serde_json::json!(256)), None);
        assert_eq!(<i8 as Scalar>::from_json(&serde_json::json!(-129)), None);
        assert_eq!(<i32 as Scalar>::from_json(&serde_json::json!(1.5)), None);
        assert_eq!(<f32 as Scalar>::from_json(&serde_json::json!(1e300)), None);
        assert_eq!(<f32 as Scalar>::from_json(&serde_json::json!(3)), Some(3.0));
        assert_eq!(<bool as Scalar>::from_json(&serde_json::json!(1)), None);
        assert_eq!(<bool as Scalar>::read_le(&[2]), None);
    }
}
