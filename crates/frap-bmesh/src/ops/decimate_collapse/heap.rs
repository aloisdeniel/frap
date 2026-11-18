// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_heap`: the binary min-heap the edge-collapse decimator pops its cheapest edge from.
//! Ported exactly (same sift order; `HEAP_COMPARE` is a strict `<`, so `heap_up` stops at an
//! equal parent and `heap_down` prefers the left child; `BLI_heap_remove` bubbles the node to
//! the root and pops it), because equal costs are common on regular meshes (flat areas take
//! the topology fallback cost) and the pop order decides which edges collapse.
//!
//! Ported from blender/source/blender/blenlib/intern/BLI_heap.cc (Blender 5.2.4,
//! GPL-2.0-or-later). Nodes are indices into an arena instead of pointers; Blender recycles
//! freed nodes, which is not observable.

/// A node handle (`HeapNode *`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct NodeId(usize);

#[derive(Clone, Debug)]
struct Node<T> {
    value: f32,
    ptr: T,
    /// Position in `tree`.
    index: usize,
}

/// `Heap`.
#[derive(Clone, Debug)]
pub(crate) struct Heap<T> {
    nodes: Vec<Node<T>>,
    tree: Vec<usize>,
}

impl<T: Copy> Heap<T> {
    /// `BLI_heap_new_ex(reserve)`.
    pub(crate) fn with_capacity(reserve: usize) -> Heap<T> {
        Heap {
            nodes: Vec::with_capacity(reserve),
            tree: Vec::with_capacity(reserve),
        }
    }

    fn less(&self, a: usize, b: usize) -> bool {
        self.nodes[self.tree[a]].value < self.nodes[self.tree[b]].value
    }

    fn swap(&mut self, i: usize, j: usize) {
        let (pi, pj) = (self.tree[i], self.tree[j]);
        self.nodes[pi].index = j;
        self.tree[j] = pi;
        self.nodes[pj].index = i;
        self.tree[i] = pj;
    }

    fn down(&mut self, mut i: usize) {
        let size = self.tree.len();
        loop {
            let l = (i << 1) + 1;
            let r = (i << 1) + 2;
            let mut smallest = i;
            if l < size && self.less(l, smallest) {
                smallest = l;
            }
            if r < size && self.less(r, smallest) {
                smallest = r;
            }
            if smallest == i {
                break;
            }
            self.swap(i, smallest);
            i = smallest;
        }
    }

    fn up(&mut self, mut i: usize) {
        while i > 0 {
            let p = (i - 1) >> 1;
            if self.less(p, i) {
                break;
            }
            self.swap(p, i);
            i = p;
        }
    }

    /// `BLI_heap_insert`.
    pub(crate) fn insert(&mut self, value: f32, ptr: T) -> NodeId {
        let id = self.nodes.len();
        let index = self.tree.len();
        self.nodes.push(Node { value, ptr, index });
        self.tree.push(id);
        self.up(index);
        NodeId(id)
    }

    /// `BLI_heap_is_empty`.
    pub(crate) fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// `BLI_heap_top_value` (the heap must not be empty).
    pub(crate) fn top_value(&self) -> f32 {
        self.nodes[self.tree[0]].value
    }

    /// `BLI_heap_pop_min` (the heap must not be empty).
    pub(crate) fn pop_min(&mut self) -> T {
        let ptr = self.nodes[self.tree[0]].ptr;
        let last = self.tree.len() - 1;
        if last > 0 {
            self.swap(0, last);
            self.tree.pop();
            self.down(0);
        } else {
            self.tree.pop();
        }
        ptr
    }

    /// `BLI_heap_remove`.
    pub(crate) fn remove(&mut self, node: NodeId) {
        let mut i = self.nodes[node.0].index;
        while i > 0 {
            let p = (i - 1) >> 1;
            self.swap(p, i);
            i = p;
        }
        self.pop_min();
    }

    /// `BLI_heap_node_ptr`.
    pub(crate) fn node_ptr(&self, node: NodeId) -> T {
        self.nodes[node.0].ptr
    }

    /// `BLI_heap_node_value_update_ptr`.
    fn node_value_update_ptr(&mut self, node: NodeId, value: f32, ptr: T) {
        self.nodes[node.0].ptr = ptr;
        let old = self.nodes[node.0].value;
        if value < old {
            self.nodes[node.0].value = value;
            self.up(self.nodes[node.0].index);
        } else if value > old {
            self.nodes[node.0].value = value;
            self.down(self.nodes[node.0].index);
        }
    }

    /// `BLI_heap_insert_or_update`.
    pub(crate) fn insert_or_update(&mut self, node: &mut Option<NodeId>, value: f32, ptr: T) {
        match *node {
            None => *node = Some(self.insert(value, ptr)),
            Some(n) => self.node_value_update_ptr(n, value, ptr),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ties_pop_in_blender_order() {
        let mut h = Heap::with_capacity(8);
        for (i, v) in [3.0, 1.0, 2.0, 1.0, 0.5].into_iter().enumerate() {
            h.insert(v, i);
        }
        let mut out = Vec::new();
        while !h.is_empty() {
            out.push(h.pop_min());
        }
        assert_eq!(out, [4, 1, 3, 2, 0]);
    }

    #[test]
    fn insert_or_update_and_remove() {
        let mut h = Heap::with_capacity(4);
        let mut a = None;
        h.insert_or_update(&mut a, 5.0, 'a');
        let b = h.insert(4.0, 'b');
        h.insert(3.0, 'c');
        h.insert_or_update(&mut a, 1.0, 'A');
        assert_eq!(h.top_value(), 1.0);
        assert_eq!(h.node_ptr(a.unwrap()), 'A');
        h.remove(b);
        assert_eq!(h.pop_min(), 'A');
        assert_eq!(h.pop_min(), 'c');
        assert!(h.is_empty());
    }
}
