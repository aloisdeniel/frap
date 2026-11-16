// SPDX-License-Identifier: GPL-3.0-or-later
//! Left-to-right topological order of a node tree and `link.is_valid` (WP 1.11).
//!
//! Port of `update_toposort` / `toposort_from_start_node`
//! (`source/blender/blenkernel/intern/node_runtime.cc`) and of `update_link_validation`
//! (`source/blender/blenkernel/intern/node_tree_update.cc`), Blender 5.2, GPL-2.0-or-later.
//!
//! The sort is a depth-first walk upstream (through the inputs, in socket order, then the
//! links into each socket in `tree.links` order, skipping links that touch a disabled
//! socket) started from every node without an available outgoing link, then from every node
//! left over (cycles). Meeting a node that is still on the stack ends the walk of the current
//! node, which is appended at once: that is how Blender orders cycles.
//!
//! A link is valid unless (verified on 5.2.4): one of its sockets is disabled (Mix `A_Color`
//! while `data_type='FLOAT'`); its source comes after its target in that order (the back
//! edge of a cycle, and links into a node whose walk a cycle cut short: after a self link
//! into the Principled BSDF's `Metallic`, a Value linked into its `Alpha` is invalid while
//! one linked into `Base Color` stays valid); or it goes from a SHADER output into a
//! non-shader input (`shader_validate_link`). A self link between data sockets (Math →
//! itself) is valid, and so are extra links into one input added with
//! `verify_limits=False`.

use crate::{Link, NodeTree, SocketType};

impl NodeTree {
    /// Whether link `l` touches only enabled sockets (`bNodeLink::is_available`).
    fn link_available(&self, l: &Link) -> bool {
        let from = self
            .node(l.from_node)
            .and_then(|n| n.outputs.iter().find(|s| s.identifier == l.from_socket));
        let to = self
            .node(l.to_node)
            .and_then(|n| n.inputs.iter().find(|s| s.identifier == l.to_socket));
        matches!((from, to), (Some(f), Some(t)) if f.enabled && t.enabled)
    }

    /// `ntree.toposort_left_to_right()`: node indices (in `nodes`) with every node after the
    /// nodes it reads from, except across cycles.
    pub fn toposort_left_to_right(&self) -> Vec<usize> {
        let n = self.nodes.len();
        // Per node: per input socket, the source node indices of its available links.
        let mut upstream: Vec<Vec<Vec<usize>>> = self
            .nodes
            .iter()
            .map(|node| vec![Vec::new(); node.inputs.len()])
            .collect();
        let mut has_linked_outputs = vec![false; n];
        for l in &self.links {
            if !self.link_available(l) {
                continue;
            }
            let (Some(from), Some(to)) = (self.node_index(l.from_node), self.node_index(l.to_node))
            else {
                continue;
            };
            let Some(s) = self.nodes[to].input_by_identifier(&l.to_socket) else {
                continue;
            };
            upstream[to][s].push(from);
            has_linked_outputs[from] = true;
        }

        #[derive(Clone, Copy, Default)]
        struct State {
            done: bool,
            in_stack: bool,
        }
        struct Item {
            node: usize,
            socket: usize,
            link: usize,
        }
        let mut states = vec![State::default(); n];
        let mut sorted = Vec::with_capacity(n);
        let from_start = |start: usize, states: &mut Vec<State>, sorted: &mut Vec<usize>| {
            let mut stack = vec![Item {
                node: start,
                socket: 0,
                link: 0,
            }];
            states[start].in_stack = true;
            while let Some(item) = stack.last_mut() {
                let node = item.node;
                let mut pushed = None;
                // Until every input socket has been visited.
                while let Some(links) = upstream[node].get(item.socket) {
                    let Some(&linked) = links.get(item.link) else {
                        item.socket += 1;
                        item.link = 0;
                        continue;
                    };
                    if states[linked].done {
                        item.link += 1;
                        continue;
                    }
                    if !states[linked].in_stack {
                        pushed = Some(linked);
                    }
                    // A node still on the stack: a cycle, which ends this node's walk.
                    break;
                }
                match pushed {
                    Some(next) => {
                        states[next].in_stack = true;
                        stack.push(Item {
                            node: next,
                            socket: 0,
                            link: 0,
                        });
                    }
                    None => {
                        states[node] = State {
                            done: true,
                            in_stack: false,
                        };
                        sorted.push(node);
                        stack.pop();
                    }
                }
            }
        };
        for i in 0..n {
            if !states[i].done && !has_linked_outputs[i] {
                from_start(i, &mut states, &mut sorted);
            }
        }
        for i in 0..n {
            if !states[i].done {
                from_start(i, &mut states, &mut sorted);
            }
        }
        sorted
    }

    /// `link.is_valid` (see the module docs for the rules).
    pub fn link_is_valid(&self, l: &Link) -> bool {
        self.links_valid_with(&self.toposort_positions(), l)
    }

    /// `is_valid` of every link, in `links` order (one sort for the whole tree).
    pub fn links_valid(&self) -> Vec<bool> {
        let pos = self.toposort_positions();
        self.links
            .iter()
            .map(|l| self.links_valid_with(&pos, l))
            .collect()
    }

    /// Position of each node (by index) in [`NodeTree::toposort_left_to_right`].
    fn toposort_positions(&self) -> Vec<usize> {
        let mut pos = vec![0; self.nodes.len()];
        for (p, i) in self.toposort_left_to_right().into_iter().enumerate() {
            pos[i] = p;
        }
        pos
    }

    fn links_valid_with(&self, pos: &[usize], l: &Link) -> bool {
        if !self.link_available(l) {
            return false;
        }
        let (Some(fi), Some(ti)) = (self.node_index(l.from_node), self.node_index(l.to_node))
        else {
            return false;
        };
        if pos[fi] > pos[ti] {
            return false;
        }
        let from = self.nodes[fi]
            .outputs
            .iter()
            .find(|s| s.identifier == l.from_socket);
        let to = self.nodes[ti]
            .inputs
            .iter()
            .find(|s| s.identifier == l.to_socket);
        match (from, to) {
            (Some(f), Some(t)) => !(f.ty == SocketType::Shader && t.ty != SocketType::Shader),
            _ => false,
        }
    }
}
