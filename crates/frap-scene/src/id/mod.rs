// SPDX-License-Identifier: GPL-3.0-or-later
//! Datablock types, one file each (ARCHITECTURE §5.6; Mesh lives in frap-mesh, NodeTree in
//! frap-nodes, Action in frap-anim). M1 W0 froze the field sets of the M1 types (Object, Scene,
//! Collection, Material, Camera, Light, World, Image, Library, decision log W0-21); the types of
//! later milestones (Armature, Key, Texture, PointCloud) are header-only placeholders completed
//! by their milestone's W0. WP 1.10 owns this directory.

use frap_rna::*;

mod armature;
mod camera;
mod collection;
pub mod idlist;
mod image;
mod key;
pub mod layers;
mod library;
mod light;
mod material;
mod modifier;
mod object;
pub mod paths;
mod pointcloud;
mod scene;
/// The glTF add-on's scene-level variant data (M7 W0 contract; WP 7.5).
pub mod scene_gltf_variants;
pub mod slots;
mod texture;
/// Timeline markers (M7 W0 contract; WP 7.6).
pub mod timeline_marker;
pub mod transform;
mod world;

pub use armature::{Armature, Bone, BoneCollection, Pose, PoseBone};
pub use camera::Camera;
pub use collection::Collection;
pub use image::{
    EncodedImage, IMAGE_DECODERS, IMAGE_PROBES, ImBuf, ImBufData, ImInfo, Image, ImageDecoder,
    ImageProbe, LoadError, PathBase, decode_image, imbuf, io_message, probe_image,
};
pub use key::{Key, ShapeKey};
pub use library::Library;
pub use light::Light;
pub use material::{Material, SHADER_NODETREE_NAME};
pub use modifier::{GnInput, GnInputs, GnValue, Modifier, ModifierSettings, object_pointer_rule};
pub use object::{Object, SlotLink, SlotOverride};
pub use pointcloud::PointCloud;
pub use scene::Scene;
pub use scene_gltf_variants::{GltfVariant, SceneGltfVariants};
pub use texture::Texture;
pub use timeline_marker::TimelineMarker;
pub use world::World;

/// Retargets `animation_data.action` (`IdType::remap_ref`).
pub(crate) fn remap_action(ad: &mut Option<frap_anim::AnimData>, from: IdAny, to: Option<IdAny>) {
    if let Some(ad) = ad.as_mut()
        && ad.action.map(|a| a.any()) == Some(from)
    {
        ad.action = to.and_then(IdRef::from_any);
    }
}

/// The `ID` base properties for hand-written bindings (the `id_rna!` list): `None` when
/// `prop` is not one of them.
pub(crate) fn id_common_get(
    h: &IdHeader,
    id_type: &'static str,
    prop: &'static PropDef,
) -> Option<Result<Field<'static>, RnaError>> {
    let b = |v: bool| Some(Ok(Field::Value(Value::Bool(v))));
    match prop.identifier {
        "name" | "name_full" | "users" => Some(Ok(Field::NeedsHook)),
        "id_type" => Some(Ok(Field::Value(Value::Enum(id_type.into())))),
        "session_uid" => Some(Ok(Field::Value(Value::Int(i64::from(
            h.session_uid as i32,
        ))))),
        "is_evaluated" | "is_missing" | "is_library_indirect" => b(false),
        "use_fake_user" => b(h.use_fake_user),
        "is_embedded_data" => b(h.is_embedded_data),
        "is_linked_packed" => b(h.is_linked_packed),
        "is_editable" => b(h.library.is_none()),
        "library" => Some(Ok(Field::Id(h.library.map(|l| l.any())))),
        _ => None,
    }
}

/// Writes of the `ID` base properties for hand-written bindings.
pub(crate) fn id_common_set(
    h: &mut IdHeader,
    srna: &'static StructDef,
    prop: &'static PropDef,
    v: &Value,
) -> Option<Result<SetOutcome, RnaError>> {
    match prop.identifier {
        "name" => Some(Ok(SetOutcome::NeedsHook)),
        "use_fake_user" => {
            h.use_fake_user = v.as_bool().unwrap_or(h.use_fake_user);
            Some(Ok(SetOutcome::Done))
        }
        "name_full"
        | "users"
        | "id_type"
        | "session_uid"
        | "is_evaluated"
        | "is_missing"
        | "is_library_indirect"
        | "is_embedded_data"
        | "is_linked_packed"
        | "is_editable"
        | "library" => Some(Err(frap_rna::traits::read_only(srna, prop.identifier))),
        _ => None,
    }
}

/// `'X' object has no attribute 'y'`.
pub(crate) fn no_attr(srna: &'static StructDef, prop: &PropDef) -> RnaError {
    frap_rna::traits::no_attribute(srna, prop.identifier)
}

/// `rna_struct!` binding of an ID type: the `ID` base properties every datablock shares
/// (`name`, `name_full` and `users` through the `ID` hooks; header flags read-only), then the
/// type's own bindings.
macro_rules! id_rna {
    ($ty:ident as $krate:ident :: $srna:ident, $id_type:literal { $($rest:tt)* }) => {
        frap_rna::rna_struct! { $ty as $krate::$srna {
            name => hook(on = "ID"),
            name_full => hook(on = "ID"),
            users => hook(on = "ID"),
            id_type => readonly(|_s| $id_type),
            session_uid => readonly(|s| s.header.session_uid as i32),
            is_evaluated => readonly(|_s| false),
            use_fake_user => header.use_fake_user,
            is_embedded_data => readonly(|s| s.header.is_embedded_data),
            is_linked_packed => readonly(|s| s.header.is_linked_packed),
            is_missing => readonly(|_s| false),
            is_editable => readonly(|s| s.header.library.is_none()),
            is_library_indirect => readonly(|_s| false),
            library => id(header.library),
            $($rest)*
        }}
    };
}

pub(crate) use id_rna;
