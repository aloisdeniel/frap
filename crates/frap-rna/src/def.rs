// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA metadata (ARCHITECTURE §9.1, CONTRACT frozen at M1 W0, amended by RFC 0001 F-DEF-1..4).
//! The statics are generated from the Blender dumps into `frap-spec-rna` / `frap-spec-ops-*`
//! (L2); until WP 1.1 lands, hand-written M0 stand-ins live there, checked by drift tests.

/// An RNA struct (`bpy.types.<identifier>`).
#[derive(Debug)]
pub struct StructDef {
    /// "Object", "Mesh", "ID".
    pub identifier: &'static str,
    /// Base struct ("ID" for every datablock).
    pub base: Option<&'static StructDef>,
    /// UI name.
    pub name: &'static str,
    /// Description.
    pub description: &'static str,
    /// Own props only; lookup walks the base chain.
    pub props: &'static [PropDef],
    /// Own functions.
    pub funcs: &'static [FuncDef],
    /// Used for `coll['Name']` lookup and listing.
    pub name_property: Option<&'static str>,
    /// True for datablocks.
    pub is_id: bool,
}

impl StructDef {
    /// Finds a property, walking the base chain.
    pub fn find_prop(&'static self, identifier: &str) -> Option<&'static PropDef> {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(def) = s {
            if let Some(p) = def.props.iter().find(|p| p.identifier == identifier) {
                return Some(p);
            }
            s = def.base;
        }
        None
    }
    /// Finds a function, walking the base chain.
    pub fn find_func(&'static self, identifier: &str) -> Option<&'static FuncDef> {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(def) = s {
            if let Some(f) = def.funcs.iter().find(|f| f.identifier == identifier) {
                return Some(f);
            }
            s = def.base;
        }
        None
    }
    /// True when `identifier` is this struct or one of its bases (`isinstance`).
    pub fn is_a(&'static self, identifier: &str) -> bool {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(def) = s {
            if def.identifier == identifier {
                return true;
            }
            s = def.base;
        }
        false
    }
    /// Every property, base first.
    pub fn all_props(&'static self) -> Vec<&'static PropDef> {
        let mut chain = Vec::new();
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(def) = s {
            chain.push(def);
            s = def.base;
        }
        chain.iter().rev().flat_map(|d| d.props.iter()).collect()
    }
}

/// An RNA property (struct member or operator property).
#[derive(Debug)]
pub struct PropDef {
    /// Identifier ("location").
    pub identifier: &'static str,
    /// UI name ("Location").
    pub name: &'static str,
    /// Description.
    pub description: &'static str,
    /// Type and type-specific metadata.
    pub kind: PropKind,
    /// RNA subtype.
    pub subtype: Subtype,
    /// RNA unit (FLOAT only; `Unit::None` otherwise).
    pub unit: Unit,
    /// Array shape.
    pub array: ArrayShape,
    /// Flags.
    pub flags: PropFlags,
    /// Deprecation metadata (5.2 `is_deprecated`, e.g. `Material.use_nodes`); frap emits
    /// Blender's DeprecationWarning report (RFC 0001 F-DEF-3).
    pub deprecated: Option<&'static Deprecation>,
}

/// Blender 5.2 deprecation metadata of a property.
#[derive(Debug)]
pub struct Deprecation {
    /// `deprecated_note`
    pub note: &'static str,
    /// `deprecated_version`
    pub version: [u16; 3],
    /// `deprecated_removal_version`
    pub removal_version: [u16; 3],
}

impl PropDef {
    /// Blender's `bpy.types.Property.type` string.
    pub fn type_str(&self) -> &'static str {
        match self.kind {
            PropKind::Bool { .. } => "BOOLEAN",
            PropKind::Int { .. } => "INT",
            PropKind::Float { .. } => "FLOAT",
            PropKind::String { .. } => "STRING",
            PropKind::Enum { .. } => "ENUM",
            PropKind::Pointer { .. } => "POINTER",
            PropKind::Collection { .. } => "COLLECTION",
        }
    }
    /// Number of scalar elements of an array property (1 for scalars).
    pub fn array_len(&self) -> usize {
        match self.array {
            ArrayShape::Scalar | ArrayShape::Dynamic => 1,
            ArrayShape::Fixed(n) => n as usize,
            ArrayShape::Matrix(r, c) => r as usize * c as usize,
        }
    }
    /// True for `READONLY` props.
    pub fn is_readonly(&self) -> bool {
        self.flags.contains(PropFlags::READONLY)
    }
}

/// Type-specific metadata.
#[derive(Debug)]
#[non_exhaustive]
pub enum PropKind {
    /// BOOLEAN.
    Bool {
        /// Default.
        default: Defaults<bool>,
    },
    /// INT.
    Int {
        /// Default.
        default: Defaults<i64>,
        /// Hard range.
        hard: (i64, i64),
        /// Soft range.
        soft: (i64, i64),
    },
    /// FLOAT.
    Float {
        /// Default (widened to f64 as Blender reports it).
        default: Defaults<f64>,
        /// Hard range.
        hard: (f64, f64),
        /// Soft range.
        soft: (f64, f64),
        /// UI precision.
        precision: u8,
        /// Storage width.
        storage: FloatStorage,
    },
    /// STRING.
    String {
        /// Default.
        default: &'static str,
        /// Blender's `length_max` (0 = unlimited). Includes the NUL terminator, as Blender reports it.
        max_bytes: u32,
    },
    /// ENUM.
    Enum {
        /// Items (for dynamic enums: frap's superset from the overlay).
        items: &'static [EnumItem],
        /// Default.
        default: EnumDefault,
        /// Overlay key when the items come from a callback in Blender.
        dynamic: Option<&'static str>,
    },
    /// POINTER.
    Pointer {
        /// Target struct identifier.
        fixed_type: &'static str,
        /// Whether the target is an ID.
        is_id: bool,
    },
    /// COLLECTION.
    Collection {
        /// Item struct identifier.
        item_type: &'static str,
        /// API struct ("ObjectModifiers").
        api: Option<&'static str>,
    },
}

/// Scalar or per-element defaults.
#[derive(Debug)]
pub enum Defaults<T: 'static> {
    /// Scalar default.
    Scalar(T),
    /// Array default.
    Array(&'static [T]),
}

/// Enum default.
#[derive(Debug)]
pub enum EnumDefault {
    /// Single item.
    Item(&'static str),
    /// Enum-flag set.
    Flags(&'static [&'static str]),
}

/// One enum item.
#[derive(Debug)]
pub struct EnumItem {
    /// Identifier ("WORLD").
    pub identifier: &'static str,
    /// UI name.
    pub name: &'static str,
    /// Description.
    pub description: &'static str,
    /// Integer value (`EnumPropertyItem.value`; bit values for enum flags).
    pub value: i32,
}

/// Float storage width. Not dumpable through bpy (RFC 0001 F-DEF-2): codegen emits `F32` except
/// for the props listed in the overlay (`spec/overlay/float_storage.toml`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FloatStorage {
    /// f32 (almost everything).
    F32,
    /// f64.
    F64,
}

/// Array shape.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArrayShape {
    /// Not an array.
    Scalar,
    /// `[n]`.
    Fixed(u16),
    /// `[rows, cols]`.
    Matrix(u16, u16),
    /// Dynamic length.
    Dynamic,
}

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($(#[$vm:meta])* $v:ident = $s:literal),* $(,)? }) => {
        $(#[$m])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        #[non_exhaustive]
        pub enum $name { $($(#[$vm])* $v),* }
        impl $name {
            /// Blender's identifier string.
            pub fn as_str(self) -> &'static str { match self { $($name::$v => $s),* } }
            /// Parses Blender's identifier string.
            pub fn from_blender(s: &str) -> Option<Self> { match s { $($s => Some($name::$v),)* _ => None } }
        }
    };
}

str_enum! {
    /// RNA property subtype (complete Blender 5.2 list, `bpy.types.Property.subtype`).
    Subtype {
        /// NONE
        None = "NONE", /// FILE_PATH
        FilePath = "FILE_PATH", /// DIR_PATH
        DirPath = "DIR_PATH", /// FILE_NAME
        FileName = "FILE_NAME", /// BYTE_STRING
        ByteString = "BYTE_STRING", /// PASSWORD
        Password = "PASSWORD", /// PIXEL
        Pixel = "PIXEL", /// PIXEL_DIAMETER
        PixelDiameter = "PIXEL_DIAMETER", /// UNSIGNED
        Unsigned = "UNSIGNED", /// PERCENTAGE
        Percentage = "PERCENTAGE", /// FACTOR
        Factor = "FACTOR", /// MASS
        Mass = "MASS", /// ANGLE
        Angle = "ANGLE", /// TIME
        Time = "TIME", /// TIME_ABSOLUTE
        TimeAbsolute = "TIME_ABSOLUTE", /// DISTANCE
        Distance = "DISTANCE", /// DISTANCE_DIAMETER
        DistanceDiameter = "DISTANCE_DIAMETER", /// DISTANCE_CAMERA
        DistanceCamera = "DISTANCE_CAMERA", /// POWER
        Power = "POWER", /// TEMPERATURE
        Temperature = "TEMPERATURE", /// WAVELENGTH
        Wavelength = "WAVELENGTH", /// COLOR_TEMPERATURE
        ColorTemperature = "COLOR_TEMPERATURE", /// FREQUENCY
        Frequency = "FREQUENCY", /// COLOR
        Color = "COLOR", /// TRANSLATION
        Translation = "TRANSLATION", /// DIRECTION
        Direction = "DIRECTION", /// VELOCITY
        Velocity = "VELOCITY", /// ACCELERATION
        Acceleration = "ACCELERATION", /// MATRIX
        Matrix = "MATRIX", /// EULER
        Euler = "EULER", /// QUATERNION
        Quaternion = "QUATERNION", /// AXISANGLE
        AxisAngle = "AXISANGLE", /// XYZ
        Xyz = "XYZ", /// XYZ_LENGTH
        XyzLength = "XYZ_LENGTH", /// COLOR_GAMMA
        ColorGamma = "COLOR_GAMMA", /// COORDINATES
        Coordinates = "COORDINATES", /// LAYER
        Layer = "LAYER", /// LAYER_MEMBER
        LayerMember = "LAYER_MEMBER",
    }
}

str_enum! {
    /// RNA unit (complete Blender 5.2 list).
    Unit {
        /// NONE
        None = "NONE", /// LENGTH
        Length = "LENGTH", /// AREA
        Area = "AREA", /// VOLUME
        Volume = "VOLUME", /// ROTATION
        Rotation = "ROTATION", /// TIME
        Time = "TIME", /// TIME_ABSOLUTE
        TimeAbsolute = "TIME_ABSOLUTE", /// VELOCITY
        Velocity = "VELOCITY", /// ACCELERATION
        Acceleration = "ACCELERATION", /// MASS
        Mass = "MASS", /// CAMERA
        Camera = "CAMERA", /// POWER
        Power = "POWER", /// TEMPERATURE
        Temperature = "TEMPERATURE", /// WAVELENGTH
        Wavelength = "WAVELENGTH", /// COLOR_TEMPERATURE
        ColorTemperature = "COLOR_TEMPERATURE", /// FREQUENCY
        Frequency = "FREQUENCY",
    }
}

bitflags::bitflags! {
    /// Property flags (§9.1 set plus RFC 0001 F-DEF-1). Names equal Blender's dump names.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct PropFlags: u32 {
        /// READONLY
        const READONLY = 1 << 0;
        /// HIDDEN
        const HIDDEN = 1 << 1;
        /// SKIP_SAVE
        const SKIP_SAVE = 1 << 2;
        /// ENUM_FLAG
        const ENUM_FLAG = 1 << 3;
        /// REQUIRED
        const REQUIRED = 1 << 4;
        /// OUTPUT (function return values)
        const OUTPUT = 1 << 5;
        /// ANIMATABLE
        const ANIMATABLE = 1 << 6;
        /// PYTHON_ONLY (frap: members from `spec/overlay/python_only.toml`)
        const PYTHON_ONLY = 1 << 7;
        /// DYNAMIC_ENUM (items from a callback in Blender; superset in the overlay)
        const DYNAMIC_ENUM = 1 << 8;
        /// NEVER_NULL
        const NEVER_NULL = 1 << 9;
        /// ARGUMENT_OPTIONAL (function params that may be omitted)
        const ARGUMENT_OPTIONAL = 1 << 10;
        /// DEPRECATED (see `PropDef::deprecated`)
        const DEPRECATED = 1 << 11;
        /// PATH_SUPPORTS_BLEND_RELATIVE (`//` paths, e.g. `render.filepath`)
        const PATH_SUPPORTS_BLEND_RELATIVE = 1 << 12;
        /// CONTEXT_MEMBER (a `bpy.context` member that is not an RNA prop of `Context`, F-DEF-5)
        const CONTEXT_MEMBER = 1 << 13;
        /// SKIP_PRESET
        const SKIP_PRESET = 1 << 14;
    }
}

impl PropFlags {
    /// No flag (M0 spelling of `PropFlags::empty()`).
    pub const NONE: PropFlags = PropFlags::empty();
    /// Blender dump names of the set flags, in declaration order.
    pub fn names(self) -> Vec<&'static str> {
        self.iter_names().map(|(n, _)| n).collect()
    }
}

bitflags::bitflags! {
    /// RNA function flags (RFC 0001 F-DEF-4: only what bpy exposes, plus PYTHON_ONLY).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct FuncFlags: u8 {
        /// `use_self == False` (static / class functions)
        const NO_SELF = 1 << 0;
        /// `use_self_type`
        const USE_SELF_TYPE = 1 << 1;
        /// Python-only member (`spec/overlay/python_only.toml`), implemented as a hook.
        const PYTHON_ONLY = 1 << 2;
    }
}

/// An RNA function.
#[derive(Debug)]
pub struct FuncDef {
    /// Identifier.
    pub identifier: &'static str,
    /// Description.
    pub description: &'static str,
    /// Parameters, positional order.
    pub params: &'static [PropDef],
    /// Return values.
    pub returns: &'static [PropDef],
    /// Index of the first keyword-only parameter. Not dumpable: comes only from
    /// `spec/overlay/python_only.toml` (e.g. `keyframe_insert(data_path, *, …)`).
    pub keyword_only_from: Option<u8>,
    /// Flags.
    pub flags: FuncFlags,
}
