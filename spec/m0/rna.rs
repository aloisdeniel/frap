// SPDX-License-Identifier: GPL-3.0-or-later
//
// HAND-WRITTEN M0 stand-in for `crates/frap-spec-rna/src/gen/*.rs` (ARCHITECTURE §4.2, §9.6).
// Every value here is copied from the committed Blender 5.2.4 dumps in spec/m0/rna/*.json;
// `crates/frap-rna/tests/spec_m0_drift.rs` fails if they disagree. Included by
// `frap_rna::spec_m0`. Deleted when M1 W0 generates frap-spec-rna.

use crate::def::*;

/// `bpy.types.ID`
pub static ID: StructDef = StructDef {
    identifier: "ID",
    base: None,
    name: "ID",
    description: "Base type for data-blocks, defining a unique name, linking from other libraries and garbage collection",
    props: &ID_PROPS,
    funcs: &[],
    name_property: Some("name"),
    is_id: true,
};
/// `ID` own properties (M0 subset).
pub static ID_PROPS: [PropDef; 1] = [PropDef {
    identifier: "name",
    name: "Name",
    description: "Unique data-block ID name (within a same type and library)",
    kind: PropKind::String { default: "", max_bytes: 256 },
    subtype: Subtype::None,
    unit: Unit::None,
    array: ArrayShape::Scalar,
    flags: PropFlags::NEVER_NULL,
}];
/// `ID.name`
pub const ID_NAME: &PropDef = &ID_PROPS[0];

/// `bpy.types.Object`
pub static OBJECT: StructDef = StructDef {
    identifier: "Object",
    base: Some(&ID),
    name: "Object",
    description: "Object data-block defining an object in a scene",
    props: &OBJECT_PROPS,
    funcs: &[],
    name_property: Some("name"),
    is_id: true,
};
/// `Object` own properties (M0 subset: `location`).
pub static OBJECT_PROPS: [PropDef; 1] = [PropDef {
    identifier: "location",
    name: "Location",
    description: "Location of the object",
    kind: PropKind::Float {
        default: Defaults::Array(&[0.0, 0.0, 0.0]),
        hard: (-3.4028234663852886e38, 3.4028234663852886e38),
        soft: (-3.4028234663852886e38, 3.4028234663852886e38),
        precision: 5,
        storage: FloatStorage::F32,
    },
    subtype: Subtype::Translation,
    unit: Unit::Length,
    array: ArrayShape::Fixed(3),
    flags: PropFlags::ANIMATABLE,
}];
/// `Object.location`
pub const OBJECT_LOCATION: &PropDef = &OBJECT_PROPS[0];

macro_rules! id_struct {
    ($stat:ident, $ident:literal, $desc:literal) => {
        #[doc = concat!("`bpy.types.", $ident, "` (M0: header only, props inherited from ID)")]
        pub static $stat: StructDef = StructDef {
            identifier: $ident,
            base: Some(&ID),
            name: $ident,
            description: $desc,
            props: &[],
            funcs: &[],
            name_property: Some("name"),
            is_id: true,
        };
    };
}

id_struct!(MESH, "Mesh", "Mesh data-block defining geometric surfaces");
id_struct!(CAMERA, "Camera", "Camera data-block for storing camera settings");
id_struct!(LIGHT, "Light", "Light data-block for lighting a scene");
id_struct!(MATERIAL, "Material", "Material data-block to define the appearance of geometric objects for rendering");
id_struct!(SCENE, "Scene", "Scene data-block, consisting in objects and defining time and render related settings");
id_struct!(COLLECTION, "Collection", "Collection of Object data-blocks");

/// Every M0 struct, for drift tests and lookups by identifier.
pub static ALL_STRUCTS: [&StructDef; 8] = [&ID, &OBJECT, &MESH, &CAMERA, &LIGHT, &MATERIAL, &SCENE, &COLLECTION];
