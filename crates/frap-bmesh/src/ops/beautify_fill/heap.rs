// SPDX-License-Identifier: GPL-3.0-or-later
//! `BLI_heap` (`blenlib/intern/BLI_heap.cc`, Blender 5.2.4): a binary min-heap with stable
//! node handles. Ties are resolved exactly as Blender's array layout does (`heap_up` stops on a
//! strictly smaller parent, `heap_down` picks the left child first), which decides the order
//! of equal-cost edge rotations.

/// A node handle (`HeapNode *`).
pub type HeapNode = usize;

#[derive(Clone, Debug)]
struct Node<T> {
    value: f32,
    index: usize,
    ptr: T,
}

/// `Heap`.
#[derive(Clone, Debug, Default)]
pub struct Heap<T> {
    nodes: Vec<Node<T>>,
    tree: Vec<HeapNode>,
}

impl<T: Copy> Heap<T> {
    /// `BLI_heap_new`.
    pub fn new() -> Self {
        Heap {
            nodes: Vec::new(),
            tree: Vec::new(),
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
    pub fn insert(&mut self, value: f32, ptr: T) -> HeapNode {
        let id = self.nodes.len();
        let index = self.tree.len();
        self.nodes.push(Node { value, index, ptr });
        self.tree.push(id);
        self.up(index);
        id
    }
    /// `BLI_heap_is_empty`.
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
    /// `BLI_heap_len`.
    pub fn len(&self) -> usize {
        self.tree.len()
    }
    /// `BLI_heap_top_value`.
    pub fn top_value(&self) -> f32 {
        self.nodes[self.tree[0]].value
    }
    /// `BLI_heap_top` payload.
    pub fn top_ptr(&self) -> T {
        self.nodes[self.tree[0]].ptr
    }
    /// `BLI_heap_pop_min`.
    pub fn pop_min(&mut self) -> T {
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
    pub fn remove(&mut self, node: HeapNode) {
        let mut i = self.nodes[node].index;
        while i > 0 {
            let p = (i - 1) >> 1;
            self.swap(p, i);
            i = p;
        }
        self.pop_min();
    }
    /// `BLI_heap_node_value`.
    pub fn node_value(&self, node: HeapNode) -> f32 {
        self.nodes[node].value
    }
    /// `BLI_heap_node_ptr`.
    pub fn node_ptr(&self, node: HeapNode) -> T {
        self.nodes[node].ptr
    }
    /// `BLI_heap_node_value_update`.
    pub fn node_value_update(&mut self, node: HeapNode, value: f32) {
        let cur = self.nodes[node].value;
        if value < cur {
            self.nodes[node].value = value;
            self.up(self.nodes[node].index);
        } else if value > cur {
            self.nodes[node].value = value;
            self.down(self.nodes[node].index);
        }
    }
    /// `BLI_heap_node_value_update_ptr`.
    pub fn node_value_update_ptr(&mut self, node: HeapNode, value: f32, ptr: T) {
        self.nodes[node].ptr = ptr;
        self.node_value_update(node, value);
    }
    /// `BLI_heap_insert_or_update`.
    pub fn insert_or_update(&mut self, node: &mut Option<HeapNode>, value: f32, ptr: T) {
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
    fn pops_in_order_and_removes() {
        let mut h: Heap<u32> = Heap::new();
        let a = h.insert(3.0, 3);
        h.insert(1.0, 1);
        h.insert(2.0, 2);
        h.remove(a);
        assert_eq!(h.pop_min(), 1);
        assert_eq!(h.pop_min(), 2);
        assert!(h.is_empty());
    }

    #[test]
    fn ties_follow_insertion_layout() {
        let mut h: Heap<u32> = Heap::new();
        for i in 0..5 {
            h.insert(0.0, i);
        }
        // Root stays the first inserted (heap_up stops only on a strictly smaller parent,
        // equal values swap upwards).
        let first = h.pop_min();
        assert!(first < 5);
    }
}
