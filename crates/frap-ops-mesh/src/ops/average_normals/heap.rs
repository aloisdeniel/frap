// SPDX-License-Identifier: GPL-3.0-or-later
//! `HeapSimple`: the binary min-heap `edbm_average_normals_exec` orders a fan's corners with.
//!
//! Ported from blender/source/blender/blenlib/intern/BLI_heap_simple.cc (Blender 5.2.4,
//! GPL-2.0-or-later): the same sift-up and sift-down, so corners of equal weight pop in
//! Blender's order (which decides the order of the float sum).

#[derive(Copy, Clone, Debug)]
struct Node<T> {
    value: f32,
    ptr: T,
}

/// `HeapSimple` with payloads of type `T`.
#[derive(Clone, Debug)]
pub(crate) struct HeapSimple<T> {
    tree: Vec<Node<T>>,
}

impl<T: Copy> HeapSimple<T> {
    /// `BLI_heapsimple_new`.
    pub(crate) fn new() -> Self {
        HeapSimple { tree: Vec::new() }
    }

    /// `BLI_heapsimple_is_empty`.
    pub(crate) fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// `BLI_heapsimple_top_value`.
    pub(crate) fn top_value(&self) -> f32 {
        self.tree[0].value
    }

    /// `BLI_heapsimple_insert` (`heapsimple_up`).
    pub(crate) fn insert(&mut self, value: f32, ptr: T) {
        let mut i = self.tree.len();
        self.tree.push(Node { value, ptr });
        while i > 0 {
            let p = (i - 1) >> 1;
            if value >= self.tree[p].value {
                break;
            }
            self.tree[i] = self.tree[p];
            i = p;
        }
        self.tree[i] = Node { value, ptr };
    }

    /// `BLI_heapsimple_pop_min` (`heapsimple_down` with the last node).
    pub(crate) fn pop_min(&mut self) -> T {
        let ptr = self.tree[0].ptr;
        let last = self.tree.pop().expect("pop_min on an empty heap");
        let size = self.tree.len();
        if size == 0 {
            return ptr;
        }
        let active_val = last.value;
        let mut i = 0usize;
        loop {
            let l = 2 * i + 1;
            let r = l + 1;
            let mut smallest = i;
            // `NODE(i).value` was spilled to `active_val` before the comparisons.
            let val_at = |k: usize, tree: &[Node<T>]| {
                if k == i { active_val } else { tree[k].value }
            };
            if l < size && self.tree[l].value < active_val {
                smallest = l;
            }
            if r < size && self.tree[r].value < val_at(smallest, &self.tree) {
                smallest = r;
            }
            if smallest == i {
                break;
            }
            self.tree[i] = self.tree[smallest];
            i = smallest;
        }
        self.tree[i] = Node {
            value: active_val,
            ptr: last.ptr,
        };
        ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_value_order() {
        let mut h = HeapSimple::new();
        for (i, v) in [3.0f32, 1.0, 2.0, 1.0, 5.0, 0.5, 2.0]
            .into_iter()
            .enumerate()
        {
            h.insert(v, i);
        }
        let mut out = Vec::new();
        while !h.is_empty() {
            let v = h.top_value();
            out.push((v, h.pop_min()));
        }
        let vals: Vec<f32> = out.iter().map(|x| x.0).collect();
        assert_eq!(vals, [0.5, 1.0, 1.0, 2.0, 2.0, 3.0, 5.0]);
        // Ties pop in the order the sift-down leaves them (Blender's), not insertion order.
        let idx: Vec<usize> = out.iter().map(|x| x.1).collect();
        assert_eq!(idx, [5, 3, 1, 6, 2, 0, 4]);
    }
}
