// SPDX-License-Identifier: GPL-3.0-or-later
//
//! HAND-WRITTEN M0 stand-in for the generated `crates/frap-spec-rna/src/generated/**`
//! (ARCHITECTURE §4.2, §9.6), moved here from `spec/m0/rna.rs` by M1 W0 (WP 1.0). Every value is
//! copied from the committed Blender 5.2.4 dumps in `spec/m0/rna/*.json`;
//! `tests/spec_m0_drift.rs` fails if they disagree. `cargo xtask codegen` (WP 1.1) replaces this
//! whole directory with generated code using the same naming (RFC 0001 F-GEN-1):
//! one static per struct at the crate root (`OBJECT`) and one module per struct holding its
//! property consts (`object::LOCATION`).

use frap_rna::*;

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
    deprecated: None,
}];
/// Property consts of `ID`.
pub mod id {
    use super::*;
    /// `ID.name`
    pub const NAME: &PropDef = &ID_PROPS[0];
}

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
    deprecated: None,
}];
/// Property consts of `Object`.
pub mod object {
    use super::*;
    /// `Object.location`
    pub const LOCATION: &PropDef = &OBJECT_PROPS[0];
}

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
// Added in M1 W0 so every IdMap of Main has a StructDef (headers only).
id_struct!(WORLD, "World", "World data-block describing the environment and ambient lighting of a scene");
id_struct!(IMAGE, "Image", "Image data-block referencing an external or packed image");
id_struct!(ARMATURE, "Armature", "Armature data-block containing a hierarchy of bones, usually used for rigging characters");
id_struct!(ACTION, "Action", "A collection of F-Curves for animation");
id_struct!(KEY, "Key", "Shape keys data-block containing different shapes of geometric data-blocks");
id_struct!(NODE_TREE, "NodeTree", "Node tree consisting of linked nodes used for shading, textures and compositing");
id_struct!(TEXTURE, "Texture", "Texture data-block used by materials, lights, worlds and brushes");
id_struct!(LIBRARY, "Library", "External .blend file from which data is linked");
id_struct!(POINT_CLOUD, "PointCloud", "Point cloud data-block");

/// Every M0 struct, for drift tests and lookups by identifier.
pub static ALL_STRUCTS: [&StructDef; 17] = [
    &ID, &OBJECT, &MESH, &CAMERA, &LIGHT, &MATERIAL, &SCENE, &COLLECTION, &WORLD, &IMAGE,
    &ARMATURE, &ACTION, &KEY, &NODE_TREE, &TEXTURE, &LIBRARY, &POINT_CLOUD,
];
