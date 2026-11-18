// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_heap`: a binary min-heap of `(value, item)` nodes with stable node handles, ported
//! from blender/source/blender/blenlib/intern/BLI_heap.cc (Blender 5.2.4, GPL-2.0-or-later) so
//! ties pop in Blender's order (`heap_up` stops at an equal parent, `heap_down` prefers the
//! left child, `BLI_heap_remove` bubbles the node to the root before popping).

/// A node handle (`HeapNode *`).
pub(crate) type NodeId = usize;

#[derive(Clone, Debug)]
struct Node<T> {
    value: f32,
    index: usize,
    ptr: T,
}

/// `Heap`.
#[derive(Clone, Debug)]
pub(crate) struct Heap<T> {
    tree: Vec<NodeId>,
    nodes: Vec<Node<T>>,
}

impl<T: Copy> Heap<T> {
    /// `BLI_heap_new_ex`.
    pub(crate) fn new(reserve: usize) -> Heap<T> {
        Heap {
            tree: Vec::with_capacity(reserve),
            nodes: Vec::with_capacity(reserve),
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
            let l = 2 * i + 1;
            let r = 2 * i + 2;
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
        self.nodes.push(Node { value, index, ptr });
        self.tree.push(id);
        self.up(index);
        id
    }

    /// `BLI_heap_top`.
    pub(crate) fn top(&self) -> Option<NodeId> {
        self.tree.first().copied()
    }

    /// `BLI_heap_node_value`.
    pub(crate) fn value(&self, n: NodeId) -> f32 {
        self.nodes[n].value
    }

    /// `BLI_heap_node_ptr`.
    pub(crate) fn ptr(&self, n: NodeId) -> T {
        self.nodes[n].ptr
    }

    /// `BLI_heap_pop_min`.
    pub(crate) fn pop_min(&mut self) -> Option<T> {
        let top = *self.tree.first()?;
        let ptr = self.nodes[top].ptr;
        let last = self.tree.len() - 1;
        if last > 0 {
            self.swap(0, last);
            self.tree.pop();
            self.down(0);
        } else {
            self.tree.pop();
        }
        Some(ptr)
    }

    /// `BLI_heap_remove`.
    pub(crate) fn remove(&mut self, n: NodeId) {
        let mut i = self.nodes[n].index;
        while i > 0 {
            let p = (i - 1) >> 1;
            self.swap(p, i);
            i = p;
        }
        self.pop_min();
    }

    /// `BLI_heap_node_value_update`.
    pub(crate) fn update(&mut self, n: NodeId, value: f32) {
        let old = self.nodes[n].value;
        if value < old {
            self.nodes[n].value = value;
            self.up(self.nodes[n].index);
        } else if value > old {
            self.nodes[n].value = value;
            self.down(self.nodes[n].index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_order_and_updates() {
        let mut h: Heap<u32> = Heap::new(8);
        let ids: Vec<NodeId> = [5.0, 1.0, 3.0, 4.0, 2.0]
            .iter()
            .enumerate()
            .map(|(i, &v)| h.insert(v, i as u32))
            .collect();
        h.update(ids[0], 0.5);
        h.remove(ids[2]);
        let mut out = Vec::new();
        while let Some(p) = h.pop_min() {
            out.push(p);
        }
        assert_eq!(out, vec![0, 1, 4, 3]);
    }
}
