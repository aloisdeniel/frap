// SPDX-License-Identifier: GPL-3.0-or-later
//! `EXT_mesh_gpu_instancing` export (WP 7.5): `export_gpu_instances` with the add-on's rule,
//! a post-pass over the gathered document. `export()` calls [`apply`] after the animations and
//! skins.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/exp/exporter.py`
//! (`manage_gpu_instancing_nodes`, `manage_gpu_instancing`, `recursive_slide_node_idx`),
//! Apache-2.0, The glTF-Blender-IO authors. Per glTF scene, from each root node:
//!
//! - the children of a node that have no children of their own and a mesh are grouped by mesh
//!   (first-seen order); a group of at least two is instanced;
//! - one group under a node without a mesh: the node itself holds the instances (it gets the
//!   mesh); otherwise (several groups, or a node with a mesh) each group gets a new child node
//!   `<node name>.<k>` appended to the node and to `nodes`;
//! - the holder gets `EXT_mesh_gpu_instancing` with FLOAT `TRANSLATION` (VEC3), `ROTATION`
//!   (VEC4, glTF `xyzw`) and `SCALE` (VEC3) accessors of the instance nodes' TRS (absent
//!   members: identity), written after everything else, each in its own buffer view without
//!   target, min or max;
//! - the instance nodes leave their parent's children and are removed from `nodes` (after the
//!   scene's pass); the scene's roots, the children of the nodes it reaches and every skin's
//!   joints and skeleton are renumbered, animation channels targeting a removed node are
//!   dropped and animations left without channels too; the recursion continues into the
//!   node's remaining children (new holders included).
//!
//! Quirks kept under `frap_compat='BLENDER'` (the default): the surviving channels keep their
//! `target.node` (the add-on does not renumber them, so a channel of a node after a removed
//! one targets the wrong node), and so do the roots of the *other* scenes; their samplers and
//! accessors stay. `frap_compat='SPEC'` renumbers both. With meshopt compression the add-on
//! also compresses the three accessors; frap writes them uncompressed (deviations.md).

use frap_gltf::accessor::AccessorData;
use frap_gltf::builder::AccessorOpts;
use frap_gltf::ext::ext_mesh_gpu_instancing::ExtMeshGpuInstancing;
use frap_gltf::json::{Index, Node};
use frap_spec_ops_io::export_scene::gltf::FrapCompat;
use indexmap::IndexMap;

use crate::export::ExportError;
use crate::export::gather::Gather;

const NAME: &str = "EXT_mesh_gpu_instancing";

/// `export_gpu_instances` is honoured here (M7, WP 7.5).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &["export_gpu_instances"];

fn node_at<'g>(g: &'g Gather<'_>, i: Index) -> Result<&'g Node, ExportError> {
    // Python's `self.__gltf.nodes[i]`.
    g.doc
        .nodes
        .get(i as usize)
        .ok_or_else(|| ExportError::Json("list index out of range".into()))
}

/// Instance accessors by content (a `Gather` state): the add-on's `@cached gather_accessor`
/// keys on the `BinaryData` (equal by bytes), the count and the type, so equal instance
/// attributes of two holders share one accessor and buffer view.
#[derive(Default)]
struct InstanceAccessors(std::collections::HashMap<(Vec<u8>, &'static str), Index>);

fn shared_accessor(g: &mut Gather, data: AccessorData) -> Index {
    let key = (data.encode(), data.type_.as_str());
    if let Some(&i) = g.state::<InstanceAccessors>().0.get(&key) {
        return i;
    }
    let i = g.buf.push_accessor(&data, &AccessorOpts::default());
    g.state::<InstanceAccessors>().0.insert(key, i);
    i
}

/// Shifts `i` down by the removed indices below it.
fn slide(i: Index, removed: &[Index]) -> Index {
    i - removed.iter().filter(|&&r| r < i).count() as Index
}

/// `manage_gpu_instancing(node, also_mesh)`.
fn manage(
    g: &mut Gather,
    node_idx: Index,
    also_mesh: bool,
    removed: &mut Vec<Index>,
    depth: usize,
) -> Result<(), ExportError> {
    if depth > g.doc.nodes.len() + 1 {
        return Err(ExportError::Invalid("node hierarchy has a cycle".into()));
    }
    let mut instances: IndexMap<Index, Vec<Index>> = IndexMap::new();
    for &c in &node_at(g, node_idx)?.children.clone() {
        let child = node_at(g, c)?;
        if !child.children.is_empty() {
            continue;
        }
        if let Some(m) = child.mesh {
            instances.entry(m).or_default().push(c);
        }
    }
    instances.retain(|_, v| v.len() > 1);
    let mut holders: Vec<Index> = Vec::new();
    if instances.len() == 1 && !also_mesh {
        holders.push(node_idx);
    } else if !instances.is_empty() {
        let base = node_at(g, node_idx)?.name.clone().unwrap_or_default();
        for h in 0..instances.len() {
            g.doc.nodes.push(Node {
                name: Some(format!("{base}.{h}")),
                ..Node::default()
            });
            let idx = (g.doc.nodes.len() - 1) as Index;
            g.doc.nodes[node_idx as usize].children.push(idx);
            holders.push(idx);
        }
    }
    for (k, (mesh, insts)) in instances.into_iter().enumerate() {
        let holder = holders[k];
        let mut translation: Vec<[f32; 3]> = Vec::with_capacity(insts.len());
        let mut rotation: Vec<[f32; 4]> = Vec::with_capacity(insts.len());
        let mut scale: Vec<[f32; 3]> = Vec::with_capacity(insts.len());
        for &i in &insts {
            let n = node_at(g, i)?;
            // `BinaryData.from_list(…, Float)`: float32.
            translation.push(n.translation.unwrap_or([0.0; 3]).map(|x| x as f32));
            rotation.push(n.rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]).map(|x| x as f32));
            scale.push(n.scale.unwrap_or([1.0; 3]).map(|x| x as f32));
        }
        let mut attributes = IndexMap::new();
        attributes.insert(
            "TRANSLATION".to_owned(),
            shared_accessor(g, AccessorData::from_elements(&translation)),
        );
        attributes.insert(
            "ROTATION".to_owned(),
            shared_accessor(g, AccessorData::from_elements(&rotation)),
        );
        attributes.insert(
            "SCALE".to_owned(),
            shared_accessor(g, AccessorData::from_elements(&scale)),
        );
        let ext = ExtMeshGpuInstancing {
            attributes,
            ..ExtMeshGpuInstancing::default()
        };
        let value = serde_json::to_value(&ext).map_err(|e| ExportError::Invalid(e.to_string()))?;
        let h = &mut g.doc.nodes[holder as usize];
        h.extensions.0.insert(NAME.to_owned(), value);
        h.mesh = Some(mesh);
        g.use_extension(NAME, false);
        g.doc.nodes[node_idx as usize]
            .children
            .retain(|c| !insts.contains(c));
        removed.extend(insts);
    }
    for c in node_at(g, node_idx)?.children.clone() {
        let also = node_at(g, c)?.mesh.is_some();
        manage(g, c, also, removed, depth + 1)?;
    }
    Ok(())
}

/// `recursive_slide_node_idx`.
fn slide_children(
    g: &mut Gather,
    node_idx: Index,
    removed: &[Index],
    depth: usize,
) -> Result<(), ExportError> {
    if depth > g.doc.nodes.len() + 1 {
        return Err(ExportError::Invalid("node hierarchy has a cycle".into()));
    }
    let children = node_at(g, node_idx)?.children.clone();
    for &c in &children {
        slide_children(g, c, removed, depth + 1)?;
    }
    g.doc.nodes[node_idx as usize].children = children.iter().map(|&c| slide(c, removed)).collect();
    Ok(())
}

/// `manage_gpu_instancing_nodes`: collapses instanced nodes (see the module docs).
pub fn apply(g: &mut Gather) -> Result<(), ExportError> {
    if !g.settings.props.export_gpu_instances.value {
        return Ok(());
    }
    let spec = g.settings.props.frap_compat.value == FrapCompat::Spec;
    for scene_num in 0..g.doc.scenes.len() {
        let mut removed: Vec<Index> = Vec::new();
        for node_idx in g.doc.scenes[scene_num].nodes.clone() {
            let also = node_at(g, node_idx)?.mesh.is_some();
            manage(g, node_idx, also, &mut removed, 0)?;
        }
        removed.sort_unstable();
        if spec {
            for n in &mut g.doc.nodes {
                n.children = n.children.iter().map(|&c| slide(c, &removed)).collect();
            }
        } else {
            for node_idx in g.doc.scenes[scene_num].nodes.clone() {
                slide_children(g, node_idx, &removed, 0)?;
            }
        }
        for (s, scene) in g.doc.scenes.iter_mut().enumerate() {
            if s == scene_num || spec {
                scene.nodes = scene.nodes.iter().map(|&n| slide(n, &removed)).collect();
            }
        }
        for skin in &mut g.doc.skins {
            skin.joints = skin.joints.iter().map(|&j| slide(j, &removed)).collect();
            if let Some(sk) = skin.skeleton {
                skin.skeleton = Some(slide(sk, &removed));
            }
        }
        for anim in &mut g.doc.animations {
            anim.channels
                .retain(|c| c.target.node.is_none_or(|n| !removed.contains(&n)));
            if spec {
                for c in &mut anim.channels {
                    if let Some(n) = c.target.node {
                        c.target.node = Some(slide(n, &removed));
                    }
                }
            }
        }
        g.doc.animations.retain(|a| !a.channels.is_empty());
        let mut i = 0;
        g.doc.nodes.retain(|_| {
            let keep = !removed.contains(&i);
            i += 1;
            keep
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::settings::ExportSettings;
    use frap_gltf::json::{Animation, AnimationPath, Channel, ChannelTarget, Scene};
    use frap_rna::{OpArgs, OpProps, Value};
    use frap_spec_ops_io::export_scene::ExportSceneGltf;

    fn settings(compat: &str) -> ExportSettings {
        let mut a = OpArgs::default();
        for def in ExportSceneGltf::spec().props {
            a.values.insert(def.identifier, Value::default_for(def));
        }
        a.values.insert("export_gpu_instances", Value::Bool(true));
        a.values.insert("frap_compat", Value::Enum(compat.into()));
        ExportSettings::from_props(&ExportSceneGltf::from_args(&a))
    }

    fn node(name: &str, mesh: Option<Index>, children: Vec<Index>) -> Node {
        Node {
            name: Some(name.into()),
            mesh,
            children,
            translation: Some([1.0, 2.0, 3.0]),
            ..Node::default()
        }
    }

    /// Scene 0: `E` with instances `A`, `B` (mesh 0); scene 1 (or scene 0 too, with
    /// `one_scene`): `S` after them, animated.
    fn run(compat: &str, one_scene: bool) -> Result<frap_gltf::Document, ExportError> {
        let (main, win) = frap_scene::startup::factory();
        let prefs = frap_scene::Preferences::default();
        let ctx = frap_scene::Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let s = settings(compat);
        let mut g = Gather::new(&main, &ctx, &s);
        g.doc.nodes = vec![
            node("A", Some(0), vec![]),
            node("B", Some(0), vec![]),
            node("E", None, vec![0, 1]),
            node("S", Some(1), vec![]),
        ];
        g.doc.scenes = vec![
            Scene {
                nodes: vec![2],
                ..Scene::default()
            },
            Scene {
                nodes: vec![3],
                ..Scene::default()
            },
        ];
        let ch = |n: Index| Channel {
            extensions: Default::default(),
            extras: None,
            sampler: 0,
            target: ChannelTarget {
                extensions: Default::default(),
                extras: None,
                node: Some(n),
                path: AnimationPath::Translation,
            },
        };
        g.doc.animations = vec![
            Animation {
                channels: vec![ch(1)],
                ..Animation::default()
            },
            Animation {
                channels: vec![ch(3)],
                ..Animation::default()
            },
        ];
        if one_scene {
            g.doc.scenes = vec![Scene {
                nodes: vec![2, 3],
                ..Scene::default()
            }];
        }
        apply(&mut g)?;
        Ok(g.doc)
    }

    #[test]
    fn the_empty_holds_one_group() {
        let d = run("BLENDER", true).unwrap();
        assert_eq!(d.nodes.len(), 2);
        let e = &d.nodes[0];
        assert_eq!(e.name.as_deref(), Some("E"));
        assert_eq!(e.mesh, Some(0));
        assert!(e.children.is_empty());
        assert!(e.extensions.contains(NAME));
        // The animation of the removed `B` is gone; the add-on keeps `S`'s stale target.
        assert_eq!(d.animations.len(), 1);
        assert_eq!(d.animations[0].channels[0].target.node, Some(3));
        assert_eq!(d.scenes[0].nodes, vec![0, 1]);
    }

    #[test]
    fn a_later_scene_keeps_stale_roots_as_the_addon() {
        // Scene 1 still lists node 3 after scene 0 removed two nodes: the add-on's
        // `self.__gltf.nodes[3]` is an `IndexError`.
        let e = run("BLENDER", false).unwrap_err().to_string();
        assert_eq!(e, "list index out of range");
    }

    #[test]
    fn spec_compat_renumbers_everything() {
        let d = run("SPEC", false).unwrap();
        assert_eq!(d.animations[0].channels[0].target.node, Some(1));
        assert_eq!(d.scenes[1].nodes, vec![1]);
        let d = run("SPEC", true).unwrap();
        assert_eq!(d.animations[0].channels[0].target.node, Some(1));
    }

    #[test]
    fn indices_slide_past_removed_nodes() {
        let removed = [1, 2, 5];
        assert_eq!(slide(0, &removed), 0);
        assert_eq!(slide(3, &removed), 1);
        assert_eq!(slide(6, &removed), 3);
    }
}
