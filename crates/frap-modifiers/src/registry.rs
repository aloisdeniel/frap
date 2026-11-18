// SPDX-License-Identifier: GPL-3.0-or-later
//! The type registry: the evaluators of [`crate::MODIFIER_TYPES`] (lookup by type, the
//! per-type `--describe` support list, the duplicate-registration check) and, for **every**
//! Blender type, evaluator or not, the static part of its `ModifierTypeInfo` ([`meta`]: kind,
//! flags, UI name), which the stack editing rules need (`Single`, `RequiresOriginalData`,
//! `NoUserAdd`, `SupportsEditmode`, deform-only moves).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_*.cc (Blender 5.2.4,
//! GPL-2.0-or-later): the [`meta`] table is the `/*type*/`, `/*flags*/` and `/*name*/` members
//! of every `ModifierTypeInfo modifierType_*`, extracted mechanically; [`object_supports`] is
//! `BKE_object_support_modifier_type_check` (`blenkernel/intern/object.cc`) and
//! [`addable_types`] `modifier_add_itemf` (`editors/object/object_modifier.cc`).

use crate::{MODIFIER_TYPES, ModKind, ModTypeFlags, ModifierType, ModifierTypeInfo};

/// The evaluator registered for `ty`, if any.
pub fn find(ty: ModifierType) -> Option<&'static ModifierTypeInfo> {
    MODIFIER_TYPES.iter().find(|i| i.ty == ty)
}

/// Types registered more than once (must be empty; the registry test checks it).
pub fn duplicates() -> Vec<ModifierType> {
    let mut seen = Vec::new();
    let mut dup = Vec::new();
    for i in MODIFIER_TYPES.iter() {
        if seen.contains(&i.ty) {
            if !dup.contains(&i.ty) {
                dup.push(i.ty);
            }
        } else {
            seen.push(i.ty);
        }
    }
    dup
}

/// Every type with an evaluator, in `ModifierType` order (`--describe`).
pub fn evaluated_types() -> Vec<ModifierType> {
    let mut v: Vec<ModifierType> = MODIFIER_TYPES.iter().map(|i| i.ty).collect();
    v.sort();
    v.dedup();
    v
}

/// The static part of Blender's `ModifierTypeInfo` of one type.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TypeMeta {
    /// `ModifierTypeInfo::type`.
    pub kind: ModKind,
    /// `ModifierTypeInfo::flags`.
    pub flags: ModTypeFlags,
    /// `ModifierTypeInfo::name`: the default name of a new modifier (`DATA_(mti->name)`).
    pub name: &'static str,
}

const fn m(kind: ModKind, flags: ModTypeFlags, name: &'static str) -> TypeMeta {
    TypeMeta { kind, flags, name }
}

/// Kind, flags and default name of `ty` (every Blender 5.2 type).
#[rustfmt::skip]
pub fn meta(ty: ModifierType) -> TypeMeta {
    use ModKind as K;
    use ModTypeFlags as F;
    use ModifierType as T;
    match ty {
        T::Armature => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Armature"),
        T::Array => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Array"),
        T::Bevel => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Bevel"),
        T::Boolean => m(K::Nonconstructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "Boolean"),
        T::Build => m(K::Nonconstructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS), "Build"),
        T::Cast => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Cast"),
        T::Cloth => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::USES_POINT_CACHE).union(F::SINGLE), "Cloth"),
        T::Collision => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SINGLE), "Collision"),
        T::CorrectiveSmooth => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "CorrectiveSmooth"),
        T::Curve => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Curve"),
        T::DataTransfer => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE), "DataTransfer"),
        T::Decimate => m(K::Nonconstructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS), "Decimate"),
        T::Displace => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "Displace"),
        T::DynamicPaint => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::USES_POINT_CACHE).union(F::SINGLE), "Dynamic Paint"),
        T::EdgeSplit => m(K::Constructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "EdgeSplit"),
        T::Explode => m(K::Constructive, F::ACCEPTS_MESH, "Explode"),
        T::Fluid => m(K::Constructive, F::ACCEPTS_MESH.union(F::SINGLE), "Fluid"),
        T::GreasePencilArmature => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Armature"),
        T::GreasePencilArray => m(K::Constructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Array"),
        T::GreasePencilBuild => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_EDITMODE), "Build"),
        T::GreasePencilColor => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Color"),
        T::GreasePencilDash => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Dot Dash"),
        T::GreasePencilEnvelope => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Envelope"),
        T::GreasePencilHook => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Hook"),
        T::GreasePencilLattice => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Lattice"),
        T::GreasePencilLength => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_EDITMODE), "Length"),
        T::GreasePencilMirror => m(K::Constructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Mirror"),
        T::GreasePencilMultiply => m(K::Constructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Multiple Strokes"),
        T::GreasePencilNoise => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Noise"),
        T::GreasePencilOffset => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Offset"),
        T::GreasePencilOpacity => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Opacity"),
        T::GreasePencilOutline => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Outline"),
        T::GreasePencilShrinkwrap => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Shrinkwrap"),
        T::GreasePencilSimplify => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Simplify"),
        T::GreasePencilSmooth => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Smooth"),
        T::GreasePencilSubdiv => m(K::Constructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Subdivide"),
        T::GreasePencilTexture => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "TextureMapping"),
        T::GreasePencilThickness => m(K::OnlyDeform, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Thickness"),
        T::GreasePencilTime => m(K::Nonconstructive, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "TimeOffset"),
        T::GreasePencilTint => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Tint"),
        T::GreasePencilVertexWeightAngle => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Weight Angle"),
        T::GreasePencilVertexWeightProximity => m(K::NonGeometrical, F::ACCEPTS_GREASE_PENCIL.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING), "Weight Proximity"),
        T::Hook => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Hook"),
        T::Laplaciandeform => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "LaplacianDeform"),
        T::Laplaciansmooth => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "LaplacianSmooth"),
        T::Lattice => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Lattice"),
        T::Lineart => m(K::Constructive, F::ACCEPTS_GREASE_PENCIL, "Lineart"),
        T::Mask => m(K::Nonconstructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE), "Mask"),
        T::MeshCache => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "MeshCache"),
        T::MeshDeform => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "MeshDeform"),
        T::MeshSequenceCache => m(K::Constructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS), "MeshSequenceCache"),
        T::MeshToVolume => m(K::Constructive, F::empty(), "Mesh to Volume"),
        T::Mirror => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Mirror"),
        T::Multires => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::REQUIRES_ORIGINAL_DATA), "Multires"),
        T::Nodes => m(K::Constructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::SUPPORTS_MAPPING).union(F::ACCEPTS_GREASE_PENCIL), "GeometryNodes"),
        T::NormalEdit => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "NormalEdit"),
        T::Ocean => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Ocean"),
        T::ParticleInstance => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "ParticleInstance"),
        T::ParticleSystem => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::USES_POINT_CACHE), "ParticleSystem"),
        T::Remesh => m(K::Nonconstructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_EDITMODE), "Remesh"),
        T::Screw => m(K::Constructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Screw"),
        T::Shrinkwrap => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Shrinkwrap"),
        T::SimpleDeform => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "SimpleDeform"),
        T::Skin => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "Skin"),
        T::Smooth => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_EDITMODE), "Smooth"),
        T::SoftBody => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::REQUIRES_ORIGINAL_DATA).union(F::SINGLE).union(F::USES_POINT_CACHE), "Softbody"),
        T::Solidify => m(K::Constructive, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "Solidify"),
        T::Subsurf => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Subdivision"),
        T::Surface => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::ACCEPTS_CVS).union(F::NO_USER_ADD), "Surface"),
        T::SurfaceDeform => m(K::OnlyDeform, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "SurfaceDeform"),
        T::Triangulate => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE).union(F::SUPPORTS_MAPPING).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Triangulate"),
        T::UvProject => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "UVProject"),
        T::UvWarp => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "UVWarp"),
        T::VertexWeightEdit => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE), "VertexWeightEdit"),
        T::VertexWeightMix => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE), "VertexWeightMix"),
        T::VertexWeightProximity => m(K::NonGeometrical, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE), "VertexWeightProximity"),
        T::VolumeDisplace => m(K::NonGeometrical, F::empty(), "Volume Displace"),
        T::VolumeToMesh => m(K::Constructive, F::ACCEPTS_MESH, "Volume to Mesh"),
        T::Warp => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Warp"),
        T::Wave => m(K::OnlyDeform, F::ACCEPTS_CVS.union(F::ACCEPTS_VERTEX_COS_ONLY).union(F::SUPPORTS_EDITMODE), "Wave"),
        T::WeightedNormal => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE), "WeightedNormal"),
        T::Weld => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_MAPPING).union(F::SUPPORTS_EDITMODE).union(F::ENABLE_IN_EDITMODE).union(F::ACCEPTS_CVS), "Weld"),
        T::Wireframe => m(K::Constructive, F::ACCEPTS_MESH.union(F::SUPPORTS_EDITMODE), "Wireframe"),
    }
}

/// `BKE_object_support_modifier_type_check` for the object types frap has
/// (`Object::object_type()`): meshes take every type that accepts meshes or control points,
/// empties only NODES, point clouds and hair curves NODES and MESH_SEQUENCE_CACHE, cameras,
/// lights and armatures none.
pub fn object_supports(object_type: &str, ty: ModifierType) -> bool {
    let flags = meta(ty).flags;
    match object_type {
        "EMPTY" => ty == ModifierType::Nodes,
        "POINTCLOUD" | "CURVES" => {
            matches!(ty, ModifierType::Nodes | ModifierType::MeshSequenceCache)
        }
        "MESH" => flags.intersects(ModTypeFlags::ACCEPTS_CVS.union(ModTypeFlags::ACCEPTS_MESH)),
        _ => false,
    }
}

/// `BKE_object_supports_modifiers` for frap's object types: meshes, point clouds, hair curves.
pub fn object_supports_modifiers(object_type: &str) -> bool {
    matches!(object_type, "MESH" | "POINTCLOUD" | "CURVES")
}

/// `object.modifier_add`'s `type` items (`modifier_add_itemf`) for an active object of
/// `object_type`: the types a user may add (not `NoUserAdd`) that the object supports, in
/// `Modifier.type` item order; every item without an active object.
pub fn addable_types(object_type: Option<&str>) -> Vec<ModifierType> {
    ModifierType::ALL
        .iter()
        .copied()
        .filter(|&t| match object_type {
            None => true,
            Some(o) => !meta(t).flags.contains(ModTypeFlags::NO_USER_ADD) && object_supports(o, t),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_names_match_the_generated_default_names() {
        for t in ModifierType::ALL {
            if let Some(n) = t.default_name() {
                assert_eq!(n, meta(t).name, "{t:?}");
            }
        }
        assert_eq!(meta(ModifierType::Subsurf).name, "Subdivision");
        assert_eq!(meta(ModifierType::Hook).kind, ModKind::OnlyDeform);
        assert!(
            meta(ModifierType::Multires)
                .flags
                .contains(ModTypeFlags::REQUIRES_ORIGINAL_DATA)
        );
        assert!(
            meta(ModifierType::Cloth)
                .flags
                .contains(ModTypeFlags::SINGLE)
        );
    }

    #[test]
    fn mesh_items_match_blender() {
        // The item list of Blender 5.2.4's TypeError for
        // `object.modifier_add(type='GREASE_PENCIL_TINT')` on the default cube.
        let items: Vec<&str> = addable_types(Some("MESH"))
            .into_iter()
            .map(ModifierType::identifier)
            .collect();
        let blender = "DATA_TRANSFER MESH_CACHE MESH_SEQUENCE_CACHE NORMAL_EDIT WEIGHTED_NORMAL \
            UV_PROJECT UV_WARP VERTEX_WEIGHT_EDIT VERTEX_WEIGHT_MIX VERTEX_WEIGHT_PROXIMITY ARRAY \
            BEVEL BOOLEAN BUILD DECIMATE EDGE_SPLIT NODES MASK MIRROR MULTIRES REMESH SCREW SKIN \
            SOLIDIFY SUBSURF TRIANGULATE VOLUME_TO_MESH WELD WIREFRAME ARMATURE CAST CURVE \
            DISPLACE HOOK LAPLACIANDEFORM LATTICE MESH_DEFORM SHRINKWRAP SIMPLE_DEFORM SMOOTH \
            CORRECTIVE_SMOOTH LAPLACIANSMOOTH SURFACE_DEFORM WARP WAVE CLOTH COLLISION \
            DYNAMIC_PAINT EXPLODE FLUID OCEAN PARTICLE_INSTANCE PARTICLE_SYSTEM SOFT_BODY";
        assert_eq!(items, blender.split_whitespace().collect::<Vec<_>>());
        assert_eq!(addable_types(Some("EMPTY")), vec![ModifierType::Nodes]);
        assert!(addable_types(Some("CAMERA")).is_empty());
        assert_eq!(addable_types(None).len(), ModifierType::ALL.len());
    }
}
