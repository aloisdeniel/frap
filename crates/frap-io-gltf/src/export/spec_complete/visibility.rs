// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap_node_visibility` (WP 7.4): `KHR_node_visibility` from `Object.hide_render`.
//!
//! io_scene_gltf2 5.2.41 neither writes nor reads the extension (its importer ignores it
//! when it is not required, and refuses a file that requires it), so the option is off by
//! default. With it, the node of every exported object with `hide_render` gets
//! `{"KHR_node_visibility": {"visible": false}}` (the default `true` is never written) and the
//! extension is declared in `extensionsUsed` only: a viewer without it shows the node, which
//! is what Blender's own export gives.
//!
//! The extension hides a node's descendants too, while Blender's `hide_render` hides one
//! object: an exported object that renders in Blender but sits below a hidden one gets a
//! WARNING naming both. Joint nodes of armature bones and the hierarchy-only nodes the
//! exporter adds carry no extension.

use std::collections::HashMap;

use frap_gltf::Extension;
use frap_gltf::ext::khr_node_visibility::KhrNodeVisibility;
use frap_gltf::json::Index;

use crate::export::gather::Gather;
use crate::export::node_map::NodeMap;

/// Marks the nodes of hidden objects and warns about visible objects below them.
pub fn apply(g: &mut Gather) {
    let objects: Vec<_> = g
        .state::<NodeMap>()
        .objects
        .iter()
        .map(|(&(_, ob), &n)| (ob, n))
        .collect();
    let mut hidden: HashMap<Index, bool> = HashMap::new();
    for (ob, n) in objects {
        let Some(o) = g.main.objects.get(ob) else {
            continue;
        };
        hidden.insert(n, o.hide_render);
    }
    let mut any = false;
    let mut marked: Vec<Index> = hidden
        .iter()
        .filter(|(_, h)| **h)
        .map(|(n, _)| *n)
        .collect();
    marked.sort_unstable();
    for &n in &marked {
        if let Some(node) = g.doc.nodes.get_mut(n as usize) {
            node.extensions.set(Some(KhrNodeVisibility {
                visible: Some(false),
                ..KhrNodeVisibility::default()
            }));
            any = true;
        }
    }
    if !any {
        return;
    }
    g.use_extension(
        KhrNodeVisibility::NAME,
        KhrNodeVisibility::REQUIRED_BY_DEFAULT,
    );
    // Visible object nodes under a hidden ancestor.
    let mut parent: HashMap<Index, Index> = HashMap::new();
    for (p, node) in g.doc.nodes.iter().enumerate() {
        for &c in &node.children {
            parent.insert(c, Index::try_from(p).expect("node count fits u32"));
        }
    }
    let name = |g: &Gather, n: Index| {
        g.doc
            .nodes
            .get(n as usize)
            .and_then(|n| n.name.clone())
            .unwrap_or_else(|| format!("node {n}"))
    };
    let mut warnings = Vec::new();
    let mut visible: Vec<Index> = hidden
        .iter()
        .filter(|(_, h)| !**h)
        .map(|(n, _)| *n)
        .collect();
    visible.sort_unstable();
    for n in visible {
        let mut p = parent.get(&n).copied();
        while let Some(a) = p {
            if hidden.get(&a) == Some(&true) {
                warnings.push(format!(
                    "glTF export: KHR_node_visibility hides '{}' with its hidden ancestor '{}' (Blender renders it: hide_render hides one object)",
                    name(g, n),
                    name(g, a)
                ));
                break;
            }
            p = parent.get(&a).copied();
        }
    }
    for w in warnings {
        g.warn(w);
    }
}
