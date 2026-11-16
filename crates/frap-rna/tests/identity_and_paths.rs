// SPDX-License-Identifier: GPL-3.0-or-later
//! `IdKind` markers for every `IdCode`, typed/erased references, and `RnaPtr` repr /
//! `path_from_id` against strings verified on Blender 5.2.4 (§9.3, RFC 0001 appendix).

mod support;

use std::collections::HashMap;

use frap_rna::ptr::{id_repr, py_str_repr};
use frap_rna::*;
use smol_str::SmolStr;
use support::*;

/// Exhaustive over `IdCode`: adding a code without a marker fails to compile here.
fn marker_of(code: IdCode) -> (IdCode, &'static str, &'static str) {
    fn m<K: IdKind>() -> (IdCode, &'static str, &'static str) {
        (K::CODE, K::COLLECTION, K::DEFAULT_NAME)
    }
    match code {
        IdCode::OB => m::<ObjectId>(),
        IdCode::ME => m::<MeshId>(),
        IdCode::MA => m::<MaterialId>(),
        IdCode::CA => m::<CameraId>(),
        IdCode::LA => m::<LightId>(),
        IdCode::WO => m::<WorldId>(),
        IdCode::IM => m::<ImageId>(),
        IdCode::AR => m::<ArmatureId>(),
        IdCode::AC => m::<ActionId>(),
        IdCode::SC => m::<SceneId>(),
        IdCode::GR => m::<CollectionId>(),
        IdCode::KE => m::<KeyId>(),
        IdCode::NT => m::<NodeTreeId>(),
        IdCode::TE => m::<TextureId>(),
        IdCode::LI => m::<LibraryId>(),
        IdCode::PT => m::<PointCloudId>(),
        _ => unreachable!("IdCode is non_exhaustive only for downstream crates"),
    }
}

#[test]
fn every_id_code_has_a_marker() {
    let mut collections = std::collections::BTreeSet::new();
    for code in IdCode::ALL {
        let (c, coll, default) = marker_of(code);
        assert_eq!(c, code);
        assert_eq!(coll, code.collection(), "{code:?}");
        assert!(!default.is_empty());
        assert_eq!(code.as_str().len(), 2);
        assert!(collections.insert(coll), "duplicate collection {coll}");
    }
    // Default names Blender gives new datablocks (`bpy.data.<coll>.new("")`).
    assert_eq!(MeshId::DEFAULT_NAME, "Mesh");
    assert_eq!(MaterialId::DEFAULT_NAME, "Material");
    assert_eq!(CollectionId::COLLECTION, "collections");
    assert_eq!(KeyId::COLLECTION, "shape_keys");
    assert_eq!(NodeTreeId::COLLECTION, "node_groups");
}

#[test]
fn typed_and_erased_refs() {
    let k = IdKey::next();
    let r: IdRef<ObjectId> = IdRef::from_key(k);
    let a = r.any();
    assert_eq!(a.code, IdCode::OB);
    assert_eq!(IdRef::<ObjectId>::from_any(a), Some(r));
    assert_eq!(IdRef::<MeshId>::from_any(a), None);
    assert_eq!(a.typed::<ObjectId>(), Some(r));
    assert_ne!(IdKey::next(), k);
    assert_eq!(Value::from(r), Value::Id(Some(a)));
    assert_eq!(Value::from(None::<IdRef<ObjectId>>), Value::Id(None));
    assert_eq!(format!("{r:?}"), format!("IdRef<OB>({})", k.get()));
    assert_eq!(EmbeddedSlot::NodeTree.prop_identifier(), "node_tree");
    assert_eq!(
        EmbeddedSlot::MasterCollection.prop_identifier(),
        "collection"
    );
}

struct Names(HashMap<IdAny, (SmolStr, Option<SmolStr>)>);
impl IdNames for Names {
    fn id_name(&self, id: IdAny) -> Option<(SmolStr, Option<SmolStr>)> {
        self.0.get(&id).cloned()
    }
}

fn id(code: IdCode) -> IdAny {
    IdAny {
        code,
        key: IdKey::next(),
    }
}

#[test]
fn embedded_node_tree_socket_repr_and_path_from_id() {
    // Verified: repr(material.node_tree) == "bpy.data.materials['X'].node_tree";
    // repr(inputs['Base Color']) == "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"].inputs[0]";
    // inputs[0].path_from_id("default_value") == "nodes[\"Principled BSDF\"].inputs[0].default_value".
    let mat = id(IdCode::MA);
    let names = Names([(mat, ("X".into(), None))].into_iter().collect());
    let tree = RnaPtr::new(
        Root::Embedded {
            owner: mat,
            slot: EmbeddedSlot::NodeTree,
        },
        &NODE_TREE,
    );
    assert_eq!(tree.repr(&names), "bpy.data.materials['X'].node_tree");
    assert_eq!(tree.path_from_id().as_deref(), Some(""));
    let socket = tree
        .child(Seg::Prop("nodes"), &NODE)
        .child(Seg::Key("Principled BSDF".into()), &NODE)
        .child(Seg::Prop("inputs"), &SOCKET)
        .child(Seg::Index(0), &SOCKET);
    assert_eq!(
        socket.repr(&names),
        "bpy.data.materials['X'].node_tree.nodes[\"Principled BSDF\"].inputs[0]"
    );
    let value = socket.child(Seg::Prop("default_value"), &SOCKET);
    assert_eq!(
        value.path_from_id().as_deref(),
        Some("nodes[\"Principled BSDF\"].inputs[0].default_value")
    );
    // Scene master collection: `scene.collection` is embedded too.
    let scene = id(IdCode::SC);
    let names = Names([(scene, ("Scene".into(), None))].into_iter().collect());
    let coll = RnaPtr::new(
        Root::Embedded {
            owner: scene,
            slot: EmbeddedSlot::MasterCollection,
        },
        &ID,
    );
    assert_eq!(coll.repr(&names), "bpy.data.scenes['Scene'].collection");
}

#[test]
fn id_and_struct_reprs() {
    let ob = id(IdCode::OB);
    let lib_group = id(IdCode::NT);
    let names = Names(
        [
            (ob, ("Cube".into(), None)),
            (
                lib_group,
                (
                    "Smooth by Angle".into(),
                    Some("//assets/geometry_nodes_essentials.blend".into()),
                ),
            ),
        ]
        .into_iter()
        .collect(),
    );
    let p = RnaPtr::id(ob, &OBJECT);
    assert_eq!(p.repr(&names), "bpy.data.objects['Cube']");
    assert_eq!(p.path_from_id().as_deref(), Some(""));
    // `repr(mod)`: single quotes at the root, double quotes below; `mod.path_from_id('ratio')`.
    let m = p
        .child(Seg::Prop("modifiers"), &DECIMATE)
        .child(Seg::Key("D".into()), &DECIMATE);
    assert_eq!(m.repr(&names), "bpy.data.objects['Cube'].modifiers[\"D\"]");
    assert_eq!(
        m.child(Seg::Prop("ratio"), &DECIMATE)
            .path_from_id()
            .as_deref(),
        Some("modifiers[\"D\"].ratio")
    );
    // Material slots and vertices use indices.
    let slot = p
        .child(Seg::Prop("material_slots"), &ID)
        .child(Seg::Index(0), &ID);
    assert_eq!(
        slot.repr(&names),
        "bpy.data.objects['Cube'].material_slots[0]"
    );
    // Linked IDs carry their library filepath (IdNames contract). Deviation: Blender 5.2.4
    // prints `bpy.data.node_groups['Smooth by Angle']` (verified); frap disambiguates it from a
    // local group of the same name (§9.3).
    assert_eq!(
        RnaPtr::id(lib_group, &NODE_TREE).repr(&names),
        "bpy.data.node_groups['Smooth by Angle', '//assets/geometry_nodes_essentials.blend']"
    );
    // Quoting (verified `repr(objects["a'b\"c"])`).
    assert_eq!(
        id_repr(IdCode::OB, "a'b\"c", None),
        "bpy.data.objects['a\\'b\"c']"
    );
    assert_eq!(py_str_repr("it's"), "\"it's\"");
    assert_eq!(py_str_repr("a\\b"), "'a\\\\b'");
    // Data / context roots.
    let data = RnaPtr::new(Root::Data, &ID);
    assert_eq!(data.repr(&names), "bpy.data");
    assert_eq!(data.path_from_id(), None);
    assert_eq!(
        RnaPtr::new(Root::Context, &ID)
            .child(Seg::Prop("object"), &OBJECT)
            .repr(&names),
        "bpy.context.object"
    );
}
