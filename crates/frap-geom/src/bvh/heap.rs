// SPDX-License-Identifier: GPL-3.0-or-later
//! `HeapSimple`: the binary min-heap of `BVH_NEAREST_OPTIMAL_ORDER`.
//!
//! Ported from blender/source/blender/blenlib/intern/BLI_heap_simple.cc (Blender 5.2.4,
//! GPL-2.0-or-later): the same sift-up/sift-down, so nodes of equal distance pop in Blender's
//! order.

#[derive(Copy, Clone, Debug)]
struct HeapNode {
    value: f32,
    ptr: u32,
}

/// `HeapSimple` over BVH arena nodes.
#[derive(Clone, Debug, Default)]
pub(super) struct HeapSimple {
    tree: Vec<HeapNode>,
}

impl HeapSimple {
    /// `BLI_heapsimple_new_ex(reserve_num)`
    pub(super) fn with_capacity(reserve: usize) -> HeapSimple {
        HeapSimple {
            tree: Vec::with_capacity(reserve.max(1)),
        }
    }

    /// `BLI_heapsimple_is_empty`
    pub(super) fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    /// `BLI_heapsimple_top_value`
    pub(super) fn top_value(&self) -> f32 {
        self.tree[0].value
    }

    /// `BLI_heapsimple_insert` (`heapsimple_up`).
    pub(super) fn insert(&mut self, value: f32, ptr: u32) {
        let mut i = self.tree.len();
        self.tree.push(HeapNode { value, ptr });
        while i > 0 {
            let p = (i - 1) >> 1;
            if value >= self.tree[p].value {
                break;
            }
            self.tree[i] = self.tree[p];
            i = p;
        }
        self.tree[i] = HeapNode { value, ptr };
    }

    /// `BLI_heapsimple_pop_min` (`heapsimple_down` from the last node).
    pub(super) fn pop_min(&mut self) -> u32 {
        let ptr = self.tree[0].ptr;
        let last = self.tree.pop().expect("pop_min on an empty heap");
        if self.tree.is_empty() {
            return ptr;
        }
        let size = self.tree.len();
        let active = last.value;
        let mut i = 0;
        self.tree[i].value = active;
        loop {
            let l = (i << 1) + 1;
            let r = l + 1;
            let mut smallest = i;
            if l < size && self.tree[l].value < active {
                smallest = l;
            }
            if r < size && self.tree[r].value < self.tree[smallest].value {
                smallest = r;
            }
            if smallest == i {
                break;
            }
            self.tree[i] = self.tree[smallest];
            i = smallest;
            self.tree[i].value = active;
        }
        self.tree[i].ptr = last.ptr;
        ptr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_value_order() {
        let mut h = HeapSimple::with_capacity(1);
        for (k, v) in [5.0f32, 1.0, 3.0, 1.0, 4.0, 0.5, 9.0]
            .into_iter()
            .enumerate()
        {
            h.insert(v, k as u32);
        }
        let mut out = Vec::new();
        while !h.is_empty() {
            let v = h.top_value();
            out.push((v, h.pop_min()));
        }
        let values: Vec<f32> = out.iter().map(|x| x.0).collect();
        assert_eq!(values, [0.5, 1.0, 1.0, 3.0, 4.0, 5.0, 9.0]);
        let mut ids: Vec<u32> = out.iter().map(|x| x.1).collect();
        ids.sort_unstable();
        assert_eq!(ids, [0, 1, 2, 3, 4, 5, 6]);
    }
}
