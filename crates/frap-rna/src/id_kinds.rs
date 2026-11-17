// SPDX-License-Identifier: GPL-3.0-or-later
//! One uninhabited marker enum per `IdCode` (ARCHITECTURE §5.1, CONTRACT).

use crate::id::{IdCode, IdKind};

macro_rules! id_kinds {
    ($($(#[$m:meta])* $name:ident = $code:ident, $coll:literal, $def:literal;)*) => {$(
        $(#[$m])*
        #[derive(Copy, Clone, Debug)]
        pub enum $name {}
        impl IdKind for $name {
            const CODE: IdCode = IdCode::$code;
            const COLLECTION: &'static str = $coll;
            const DEFAULT_NAME: &'static str = $def;
        }
    )*};
}

id_kinds! {
    /// Marker for `Object`.
    ObjectId = OB, "objects", "Object";
    /// Marker for `Mesh`.
    MeshId = ME, "meshes", "Mesh";
    /// Marker for `Material`.
    MaterialId = MA, "materials", "Material";
    /// Marker for `Camera`.
    CameraId = CA, "cameras", "Camera";
    /// Marker for `Light`.
    LightId = LA, "lights", "Light";
    /// Marker for `World`.
    WorldId = WO, "worlds", "World";
    /// Marker for `Image`.
    ImageId = IM, "images", "Image";
    /// Marker for `Armature`.
    ArmatureId = AR, "armatures", "Armature";
    /// Marker for `Action`.
    ActionId = AC, "actions", "Action";
    /// Marker for `Scene`.
    SceneId = SC, "scenes", "Scene";
    /// Marker for `Collection`.
    CollectionId = GR, "collections", "Collection";
    /// Marker for `Key` (shape keys).
    KeyId = KE, "shape_keys", "Key";
    /// Marker for `NodeTree`.
    NodeTreeId = NT, "node_groups", "NodeTree";
    /// Marker for `Texture`.
    TextureId = TE, "textures", "Texture";
    /// Marker for `Library`.
    LibraryId = LI, "libraries", "Library";
    /// Marker for `PointCloud`.
    PointCloudId = PT, "pointclouds", "PointCloud";
}
