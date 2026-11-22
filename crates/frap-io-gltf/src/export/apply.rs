// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_apply=True` (M4, WP 4.7): the exported mesh of an object with modifiers is its
//! viewport-evaluated mesh (`ExportSettings::evaluated_meshes`, filled by the operator), and the
//! add-on's sharing rules decide which objects still share one glTF mesh. `mesh/gather.rs` asks
//! [`evaluated`] and [`evaluated_materials`].
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/nodes.py
//! (`__gather_mesh`, add-on 5.2.41) and obj_data.py (`get_data_cache_key`):
//!
//! - `modifiers = blender_object.modifiers; if len(modifiers) == 0: modifiers = None`. With
//!   `gltf_apply` and no modifiers the original data is exported, so linked duplicates without
//!   modifiers keep sharing one glTF mesh (and its accessors), whatever their materials.
//! - With at least one modifier — even when none of them is enabled in the viewport — the data
//!   is `blender_object.evaluated_get(depsgraph).to_mesh(preserve_all_data_layers=True)`, a
//!   temporary mesh named after the original data. The cache key `(id(data), modifiers,
//!   materials)` holds the object's `ObjectModifiers` collection, so every object with modifiers
//!   gets its own glTF mesh, with its own vertex accessors (index accessors are still shared by
//!   content, `@cached gather_accessor`); linked duplicates with modifiers never share.
//! - The materials of such a mesh are the temporary mesh's own (`blender_data.materials`, the
//!   evaluated data's slots, which a Boolean TRANSFER extends), so an object-linked slot is
//!   **not** applied (verified on 5.2.4: an object with a disabled Bevel and an OBJECT-linked
//!   slot exports the mesh's material); only when that list is exactly `(None,)` does the
//!   add-on fall back to `ms.material for ms in blender_object.material_slots`.
//! - The temporary mesh carries the original's custom properties (`gltf2_mesh_applied` is
//!   black-listed), so its `extras` are the original's.
//!
//! - An object outside the viewport depsgraph is not evaluated: `evaluated_get()` returns the
//!   original object and `to_mesh()` its data, modifiers ignored (verified on 5.2.4: a Cube with
//!   a Subdivision and `hide_viewport=True` exports 24 vertices). It still has modifiers, so it
//!   still gets a glTF mesh of its own. [`viewport_evaluated_objects`] says which objects the
//!   depsgraph evaluates; the operator evaluates only those.
//!
//! The armature handling of `gltf_skins` (Armature modifiers disabled while evaluating) is M6.

use std::collections::BTreeSet;
use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{CollectionId, IdAny, IdCode, IdRef, MaterialId, NodeTreeId, ObjectId};
use frap_scene::id::{ModifierSettings, Object};
use frap_scene::{Context, LayerCollection, Main};

use crate::export::gather::Gather;
use crate::export::mesh::gather::slot_materials;

/// `export_apply` is honoured (no "not supported yet" WARNING), and so is
/// `export_current_frame`: without it the operator evaluates the meshes at frame 0
/// (`export.save`'s `frame_set(0)`); object transforms do not depend on the frame before
/// animation (M6).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static APPLY_PROPS: &[&str] = &["export_apply", "export_current_frame"];

/// The evaluated mesh to export for `ob` instead of its data (`None`: export the original):
/// `Some` only with `export_apply=True`, for an object with at least one modifier whose
/// evaluation the operator could provide, in the depsgraph of the scene being gathered
/// (`Gather::scene`, the add-on's window scene).
pub fn evaluated(g: &Gather, ob: IdRef<ObjectId>) -> Option<Arc<Mesh>> {
    let meshes = g.settings.evaluated_meshes.as_ref()?;
    let object = g.main.objects.get(ob)?;
    if !has_modifiers(object) {
        return None;
    }
    meshes.get(&(g.scene, ob)).cloned()
}

/// The objects whose geometry the viewport depsgraph of the context view layer evaluates; for
/// every other object `evaluated_get(depsgraph).to_mesh()` is the original data.
///
/// Ported from blender/source/blender/depsgraph/intern/builder/deg_builder_nodes_view_layer.cc
/// (`build_view_layer`: only bases with `BASE_ENABLED_AND_MAYBE_VISIBLE_IN_VIEWPORT`),
/// deg_builder_nodes.cc (`build_collection`: `COLLECTION_HIDE_VIEWPORT` hides the nested
/// collections too) and eval/deg_eval_visibility.cc (`deg_graph_flush_visibility_flags`: an
/// object pulled in by a modifier has its geometry evaluated only when an enabled modifier of an
/// evaluated object reads that geometry). Verified on 5.2.4:
///
/// - enabled: in a non-excluded layer collection whose collection (and every ancestor
///   collection) is not `hide_viewport`, and the object not `hide_viewport`; one such path is
///   enough. `LayerCollection.hide_viewport` and `hide_set(True)` do not matter.
/// - pulled in: the objects a viewport-enabled modifier references ([`geometry_dependencies`]:
///   a Boolean operand, the caps and the offset object of an Array, a Shrinkwrap target, a
///   Geometry Nodes Object Info object, …), recursively (WP 8.12). A modifier disabled in the
///   viewport and a hidden parent (in a freshly loaded file) do not pull the object in.
pub fn viewport_evaluated_objects(main: &Main, ctx: &Context) -> BTreeSet<IdRef<ObjectId>> {
    fn walk(main: &Main, ctx: &Context, lc: &LayerCollection, out: &mut BTreeSet<IdRef<ObjectId>>) {
        if lc.exclude {
            return;
        }
        let coll = match lc.collection {
            None => &ctx.scene().collection,
            Some(c) => match main.collections.get(c) {
                Some(c) => c,
                None => return,
            },
        };
        // A hidden collection hides its children (`is_parent_collection_visible_`).
        if coll.hide_viewport {
            return;
        }
        for &o in &coll.objects {
            if main.objects.get(o).is_some_and(|ob| !ob.hide_viewport) {
                out.insert(o);
            }
        }
        for c in &lc.children {
            walk(main, ctx, c, out);
        }
    }
    let vl = ctx.view_layer();
    let mut out = BTreeSet::new();
    walk(main, ctx, &vl.layer_collection, &mut out);
    out.retain(|o| vl.bases.iter().any(|b| b.object == *o));
    // Geometry dependencies of the evaluated objects' enabled modifiers.
    let mut stack: Vec<IdRef<ObjectId>> = out.iter().copied().collect();
    while let Some(o) = stack.pop() {
        let Some(ob) = main.objects.get(o) else {
            continue;
        };
        for md in ob.modifiers.iter().filter(|m| m.show_viewport) {
            for dep in geometry_dependencies(main, o, &md.settings) {
                if out.insert(dep) {
                    stack.push(dep);
                }
            }
        }
    }
    out
}

/// The objects a modifier of `owner` pulls into the depsgraph (the relations of the types'
/// `update_depsgraph`, blender/source/blender/modifiers/intern/MOD_*.cc, Blender 5.2.4,
/// GPL-2.0-or-later): the Boolean operand object or collection (`all_objects`), the objects a
/// Geometry Nodes tree references ([`node_tree_dependencies`], WP 8.12), and every other object
/// (or collection) pointer of the modifier's settings (Array caps, curve and offset object,
/// Shrinkwrap targets, Lattice, Curve, Hook, Cast, Warp, Data Transfer source, …).
///
/// An object in the depsgraph exports its evaluated mesh even when only its transform is read:
/// the add-on's `to_mesh(preserve_all_data_layers=True, depsgraph=…)` evaluates the modifier
/// stack of any evaluated object (`mesh_new_from_mesh_object_with_layers`; verified on 5.2.4:
/// a hidden Array offset object with a Subdivision exports subdivided, a hidden Shrinkwrap
/// target too). Pointers a type only follows conditionally (a Displace texture-coordinates
/// object outside `OBJECT` mode, a Vertex Weight Proximity target outside `GEOMETRY` mode for
/// its geometry) are followed whatever the mode.
fn geometry_dependencies(
    main: &Main,
    owner: IdRef<ObjectId>,
    s: &ModifierSettings,
) -> Vec<IdRef<ObjectId>> {
    let ids: Vec<Option<IdAny>> = match s {
        ModifierSettings::Boolean(b) if b.operand_type == "OBJECT" => vec![b.object],
        ModifierSettings::Boolean(b) => vec![b.collection],
        ModifierSettings::Nodes(n) => {
            return node_tree_dependencies(main, owner, n.node_group);
        }
        s => s.id_refs().into_iter().map(|(_, id)| id).collect(),
    };
    let mut out = Vec::new();
    for id in ids.into_iter().flatten() {
        match id.code {
            IdCode::OB => out.extend(IdRef::<ObjectId>::from_any(id).filter(|o| *o != owner)),
            IdCode::GR => out.extend(collection_objects(main, id)),
            _ => {}
        }
    }
    out
}

/// `Collection.all_objects` of a collection ID.
fn collection_objects(main: &Main, id: IdAny) -> Vec<IdRef<ObjectId>> {
    let mut out = Vec::new();
    let mut stack: Vec<IdRef<CollectionId>> =
        IdRef::<CollectionId>::from_any(id).into_iter().collect();
    let mut seen = BTreeSet::new();
    while let Some(c) = stack.pop() {
        if !seen.insert(c) {
            continue;
        }
        if let Some(coll) = main.collections.get(c) {
            out.extend(coll.objects.iter().copied());
            stack.extend(coll.children.iter().copied());
        }
    }
    out
}

/// The geometry dependencies of a NODES modifier on `owner` with node group `group`, ported
/// from blender/source/blender/nodes/intern/dependencies.cc (`gather_eval_dependencies_
/// recursive`: every socket value of the tree and its groups that is an object or a
/// collection, linked inputs excepted, and the data-block of every non-muted non-group node)
/// and MOD_nodes.cc (`update_depsgraph`, `add_object_relation`: a transform relation to every
/// object (`all_object_deps`), the geometry of the collection an empty instances, every object
/// of a referenced collection), Blender 5.2.4, GPL-2.0-or-later. frap's NODES
/// inputs hold no data-block, so the modifier's own settings add nothing.
fn node_tree_dependencies(
    main: &Main,
    owner: IdRef<ObjectId>,
    group: Option<IdAny>,
) -> Vec<IdRef<ObjectId>> {
    let mut objects: Vec<IdRef<ObjectId>> = Vec::new();
    let mut collections: Vec<IdAny> = Vec::new();
    let mut trees: Vec<IdRef<NodeTreeId>> = group
        .and_then(IdRef::<NodeTreeId>::from_any)
        .into_iter()
        .collect();
    let mut seen = BTreeSet::new();
    let add = |id: IdAny, objects: &mut Vec<IdRef<ObjectId>>, collections: &mut Vec<IdAny>| match id
        .code
    {
        IdCode::OB => objects.extend(IdRef::<ObjectId>::from_any(id)),
        IdCode::GR => collections.push(id),
        _ => {}
    };
    while let Some(t) = trees.pop() {
        if !seen.insert(t) {
            continue;
        }
        let Some(tree) = main.node_groups.get(t) else {
            continue;
        };
        for n in &tree.nodes {
            for sock in &n.inputs {
                if tree.link_into(n.uid, &sock.identifier).is_some() {
                    continue;
                }
                if let frap_nodes::SocketValue::Id(Some(id)) = sock.default_value {
                    add(id, &mut objects, &mut collections);
                }
            }
            for sock in &n.outputs {
                if let frap_nodes::SocketValue::Id(Some(id)) = sock.default_value {
                    add(id, &mut objects, &mut collections);
                }
            }
            if frap_nodes::is_group_node(n.bl_idname) {
                if let Some(g) = n.group_tree().and_then(IdRef::<NodeTreeId>::from_any) {
                    trees.push(g);
                }
                continue;
            }
            if n.mute {
                continue;
            }
            for v in n.props.values.values() {
                if let frap_rna::Value::Id(Some(id)) = v {
                    add(*id, &mut objects, &mut collections);
                }
            }
        }
    }
    let mut out = Vec::new();
    for o in objects {
        if o == owner {
            continue;
        }
        let Some(ob) = main.objects.get(o) else {
            continue;
        };
        // Every referenced object gets a transform relation; an empty instancing a
        // collection adds that collection's geometry.
        out.push(o);
        if let ("EMPTY", Some(c)) = (ob.object_type(), ob.instance_collection) {
            out.extend(collection_objects(main, c.any()));
        }
    }
    for c in collections {
        out.extend(collection_objects(main, c));
    }
    out
}

/// `len(blender_object.modifiers) != 0` (enabled or not).
pub fn has_modifiers(ob: &Object) -> bool {
    !ob.modifiers.is_empty()
}

/// The material per slot of an applied mesh: the evaluated mesh's own slots, or the object's
/// material slots (over the original data `orig`) when the evaluated mesh has exactly one empty
/// slot.
pub fn evaluated_materials(
    ob: &Object,
    orig: &Mesh,
    evaluated: &Mesh,
) -> Vec<Option<IdRef<MaterialId>>> {
    if evaluated.materials.len() == 1 && evaluated.materials[0].is_none() {
        slot_materials(ob, orig)
    } else {
        evaluated.materials.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::IdKey;
    use frap_scene::id::{SlotLink, SlotOverride};

    fn mat() -> Option<IdRef<MaterialId>> {
        Some(IdRef::from_key(IdKey::next()))
    }

    #[test]
    fn export_apply_is_supported() {
        assert!(crate::export::is_supported("export_apply"));
    }

    #[test]
    fn evaluated_materials_ignore_object_links_unless_single_empty_slot() {
        let (a, b, c) = (mat(), mat(), mat());
        let mut ob = Object::new("Ob", None);
        ob.material_slot_overrides = vec![SlotOverride {
            link: SlotLink::Object,
            material: c,
        }];
        let mut orig = Mesh::new("");
        orig.materials = vec![a];
        // The evaluated mesh's slots win over the object-linked slot.
        let mut ev = Mesh::new("");
        ev.materials = vec![a, b];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![a, b]);
        // No slots at all: no materials (the object's slots are not consulted either).
        ev.materials = vec![];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![]);
        // `(None,)`: the object's material slots.
        ev.materials = vec![None];
        assert_eq!(evaluated_materials(&ob, &orig, &ev), vec![c]);
    }

    /// WP 8.12: a Geometry Nodes tree pulls the objects its sockets and nodes reference into
    /// the depsgraph (`gather_eval_dependencies_recursive`): an Object Info `Object` input (an
    /// unlinked socket value), inside a nested group as well; a linked input's value and the
    /// owner itself do not count.
    #[test]
    fn node_tree_references_pull_objects_in() {
        use frap_nodes::{
            Link, Node, NodeTree, NodeTreeType, PropBag, Socket, SocketType, SocketValue,
        };
        use frap_scene::ChangeLog;
        let mut main = Main::default();
        let mut log = ChangeLog::default();
        let owner = main
            .objects
            .insert("Owner", Object::new("Owner", None), &mut log);
        let info = main
            .objects
            .insert("Info", Object::new("Info", None), &mut log);
        let linked = main
            .objects
            .insert("Linked", Object::new("Linked", None), &mut log);
        let inner_ob = main
            .objects
            .insert("Inner", Object::new("Inner", None), &mut log);
        let node = |uid: u32, bl_idname: &'static str, inputs: Vec<Socket>| Node {
            uid,
            name: bl_idname.into(),
            label: String::new(),
            bl_idname,
            location: [0.0; 2],
            width: 140.0,
            parent: None,
            mute: false,
            hide: false,
            inputs,
            outputs: Vec::new(),
            props: PropBag::default(),
        };
        let object_socket = |id: IdRef<ObjectId>| Socket {
            name: "Object".into(),
            identifier: "Object".into(),
            ty: SocketType::Object,
            enabled: true,
            hide_value: false,
            default_value: SocketValue::Id(Some(id.any())),
        };
        let mut inner = NodeTree::new("Inner", NodeTreeType::Geometry, false);
        inner.nodes.push(node(
            1,
            "GeometryNodeObjectInfo",
            vec![object_socket(inner_ob)],
        ));
        let inner = main.node_groups.insert("Inner", inner, &mut log);
        let mut tree = NodeTree::new("Deps", NodeTreeType::Geometry, false);
        tree.nodes
            .push(node(1, "GeometryNodeObjectInfo", vec![object_socket(info)]));
        tree.nodes.push(node(
            2,
            "GeometryNodeObjectInfo",
            vec![object_socket(linked)],
        ));
        tree.nodes.push(node(
            3,
            "GeometryNodeObjectInfo",
            vec![object_socket(owner)],
        ));
        let mut group = node(4, "GeometryNodeGroup", Vec::new());
        group
            .props
            .values
            .insert("node_tree", frap_rna::Value::Id(Some(inner.any())));
        tree.nodes.push(group);
        tree.links.push(Link {
            from_node: 1,
            from_socket: "Geometry".into(),
            to_node: 2,
            to_socket: "Object".into(),
        });
        let tree = main.node_groups.insert("Deps", tree, &mut log);
        let mut deps = node_tree_dependencies(&main, owner, Some(tree.any()));
        deps.sort();
        let mut want = vec![info, inner_ob];
        want.sort();
        assert_eq!(deps, want);
        assert!(node_tree_dependencies(&main, owner, None).is_empty());
    }
}
