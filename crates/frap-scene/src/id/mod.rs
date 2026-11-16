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
mod image;
mod key;
mod library;
mod light;
mod material;
mod modifier;
mod object;
mod pointcloud;
mod scene;
mod texture;
mod world;

pub use armature::{Armature, Pose};
pub use camera::Camera;
pub use collection::Collection;
pub use image::{EncodedImage, Image};
pub use key::Key;
pub use library::Library;
pub use light::Light;
pub use material::{Material, SHADER_NODETREE_NAME};
pub use modifier::{Modifier, ModifierSettings};
pub use object::{Object, SlotLink, SlotOverride};
pub use pointcloud::PointCloud;
pub use scene::Scene;
pub use texture::Texture;
pub use world::World;

/// `'X' object has no attribute 'y'`.
pub(crate) fn no_attr(srna: &'static StructDef, prop: &PropDef) -> RnaError {
    frap_rna::traits::no_attribute(srna, prop.identifier)
}

/// RNA binding for IDs that expose only `ID.name` so far (M0/M1 W0 placeholders).
macro_rules! name_only_rna {
    ($ty:ident, $srna:path, $kind:ident) => {
        impl frap_rna::Rna for $ty {
            fn srna(&self) -> &'static frap_rna::StructDef {
                &$srna
            }
            fn get(
                &self,
                prop: &'static frap_rna::PropDef,
            ) -> Result<frap_rna::Field<'_>, frap_rna::RnaError> {
                if std::ptr::eq(prop, frap_spec_rna::id::NAME) {
                    return Ok(frap_rna::Field::Value(frap_rna::Value::Str(
                        self.header.name.clone(),
                    )));
                }
                Err(crate::id::no_attr(&$srna, prop))
            }
        }
        impl frap_rna::RnaMut for $ty {
            fn set(
                &mut self,
                prop: &'static frap_rna::PropDef,
                _v: frap_rna::Value,
            ) -> Result<frap_rna::SetOutcome, frap_rna::RnaError> {
                if std::ptr::eq(prop, frap_spec_rna::id::NAME) {
                    return Ok(frap_rna::SetOutcome::NeedsHook);
                }
                Err(crate::id::no_attr(&$srna, prop))
            }
            fn struct_mut(
                &mut self,
                prop: &'static frap_rna::PropDef,
            ) -> Result<frap_rna::RnaMutGuard<'_>, frap_rna::RnaError> {
                Err(crate::id::no_attr(&$srna, prop))
            }
            fn coll_mut(
                &mut self,
                prop: &'static frap_rna::PropDef,
            ) -> Result<frap_rna::RnaCollMutGuard<'_>, frap_rna::RnaError> {
                Err(crate::id::no_attr(&$srna, prop))
            }
        }
    };
}

/// `IdType` for a type whose only outgoing references are listed by `$refs`.
macro_rules! id_type {
    ($ty:ident, $kind:ident) => {
        impl frap_rna::IdType for $ty {
            type Kind = frap_rna::$kind;
            fn header(&self) -> &frap_rna::IdHeader {
                &self.header
            }
            fn header_mut(&mut self) -> &mut frap_rna::IdHeader {
                &mut self.header
            }
            fn for_each_ref(&self, _f: &mut dyn FnMut(frap_rna::IdAny)) {}
            fn remap_ref(&mut self, _from: frap_rna::IdAny, _to: Option<frap_rna::IdAny>) {}
        }
    };
}

/// Header-only datablock placeholder (fields arrive with the WP that implements the type).
macro_rules! header_only_id {
    ($(#[$m:meta])* $ty:ident, $srna:path, $kind:ident) => {
        $(#[$m])*
        #[derive(Clone, Debug)]
        pub struct $ty {
            /// ID header.
            pub header: frap_rna::IdHeader,
        }
        impl $ty {
            /// New datablock (name is uniquified by `IdMap::insert`).
            pub fn new(name: impl Into<smol_str::SmolStr>) -> Self {
                $ty {
                    header: frap_rna::IdHeader::new(name),
                }
            }
        }
        crate::id::name_only_rna!($ty, $srna, $kind);
        crate::id::id_type!($ty, $kind);
    };
}

pub(crate) use header_only_id;
pub(crate) use id_type;
pub(crate) use name_only_rna;
