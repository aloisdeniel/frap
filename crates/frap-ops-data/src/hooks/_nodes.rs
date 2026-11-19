// SPDX-License-Identifier: GPL-3.0-or-later
//! Node-tree helpers of the node hooks of this directory (WP 3.1). Not a concept: the leading
//! underscore keeps it out of the §10.5 file checks.

use frap_nodes::NodeTree;
use frap_rna::{EmbeddedSlot, IdAny, IdCode, NodeTreeId, RnaError, Root, Seg};
use frap_scene::{ChangeLog, Main};

use super::_common::internal;

/// The node tree at `root` (a group, or a material's/world's embedded tree).
pub(crate) fn tree(main: &Main, root: Root) -> Option<&NodeTree> {
    match root {
        Root::Id(a) if a.code == IdCode::NT => a.typed().and_then(|r| main.node_groups.get(r)),
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => owner
                .typed()
                .and_then(|r| main.materials.get(r))
                .map(|m| &m.node_tree),
            IdCode::WO => owner
                .typed()
                .and_then(|r| main.worlds.get(r))
                .map(|w| &w.node_tree),
            _ => None,
        },
        _ => None,
    }
}

/// The node tree at `root`, mutably.
pub(crate) fn tree_mut<'m>(
    main: &'m mut Main,
    root: Root,
    log: &mut ChangeLog,
) -> Result<&'m mut NodeTree, RnaError> {
    match root {
        Root::Id(a) if a.code == IdCode::NT => {
            a.typed().and_then(|r| main.node_groups.get_mut(r, log))
        }
        Root::Embedded {
            owner,
            slot: EmbeddedSlot::NodeTree,
        } => match owner.code {
            IdCode::MA => owner
                .typed()
                .and_then(|r| main.materials.get_mut(r, log))
                .map(|m| &mut m.node_tree),
            IdCode::WO => owner
                .typed()
                .and_then(|r| main.worlds.get_mut(r, log))
                .map(|w| &mut w.node_tree),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| internal("not a node tree"))
}

/// Index of the node a pointer path ends on (`….nodes['Name']` or `….nodes[i]`).
pub(crate) fn node_index(t: &NodeTree, path: &[Seg]) -> Option<usize> {
    match path {
        [.., Seg::Prop("nodes"), Seg::Key(k)] => t.nodes.iter().position(|n| n.name == k.as_str()),
        [.., Seg::Prop("nodes"), Seg::Index(i)] => {
            usize::try_from(*i).ok().filter(|i| *i < t.nodes.len())
        }
        _ => None,
    }
}

/// Whether the group `group` uses the tree at `target` through its group nodes, at any depth
/// (assigning it there would make a cycle).
pub(crate) fn group_reaches(main: &Main, group: IdAny, target: Root) -> bool {
    let mut stack = vec![group];
    let mut seen: Vec<IdAny> = Vec::new();
    while let Some(g) = stack.pop() {
        if Root::Id(g) == target {
            return true;
        }
        if seen.contains(&g) {
            continue;
        }
        seen.push(g);
        if let Some(t) = g
            .typed::<NodeTreeId>()
            .and_then(|r| main.node_groups.get(r))
        {
            stack.extend(t.nodes.iter().filter_map(|n| n.group_tree()));
        }
    }
    false
}

/// Refreshes the group nodes of every material, world and group tree that instance the group
/// at `root` (no-op for an embedded tree, which no group node can use).
pub(crate) fn sync_group_users(main: &mut Main, log: &mut ChangeLog, root: Root) {
    let Root::Id(gid) = root else {
        return;
    };
    let Some(group) = gid
        .typed::<NodeTreeId>()
        .and_then(|r| main.node_groups.get(r))
    else {
        return;
    };
    let iface = group.interface.clone();
    let uses = |t: &NodeTree| t.nodes.iter().any(|n| n.group_tree() == Some(gid));
    let mats: Vec<_> = main
        .materials
        .iter()
        .filter(|(_, m)| uses(&m.node_tree))
        .map(|(r, _)| r)
        .collect();
    for r in mats {
        if let Some(m) = main.materials.get_mut(r, log) {
            m.node_tree.sync_group_nodes_of(gid, &iface);
        }
    }
    let worlds: Vec<_> = main
        .worlds
        .iter()
        .filter(|(_, w)| uses(&w.node_tree))
        .map(|(r, _)| r)
        .collect();
    for r in worlds {
        if let Some(w) = main.worlds.get_mut(r, log) {
            w.node_tree.sync_group_nodes_of(gid, &iface);
        }
    }
    let groups: Vec<_> = main
        .node_groups
        .iter()
        .filter(|(_, g)| uses(g))
        .map(|(r, _)| r)
        .collect();
    for r in groups {
        if let Some(g) = main.node_groups.get_mut(r, log) {
            g.sync_group_nodes_of(gid, &iface);
        }
    }
}
