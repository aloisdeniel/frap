// SPDX-License-Identifier: GPL-3.0-or-later
//! The A* shortest-path solver of the island handling of face-corner mapping
//! (`islands_precision > 0`): a graph of faces linked through their edges, solved with a
//! binary min-heap.
//!
//! Ported from blender/source/blender/blenlib/intern/astar.cc and
//! blender/source/blender/blenlib/intern/BLI_heap_simple.cc (Blender 5.2.4,
//! GPL-2.0-or-later): the same link order (`BLI_addtail` on both nodes), the same heap
//! sift-up/sift-down (equal costs pop in Blender's order) and the same `max_steps` rule.

/// `BLI_AStarGNLink`
#[derive(Clone, Debug)]
pub(crate) struct Link {
    /// The two nodes.
    pub nodes: [usize; 2],
    /// The cost of going from one to the other.
    pub cost: f32,
    /// `custom_data`: the edge of an inner cut, `-1` otherwise.
    pub edge: i32,
}

/// `BLI_AStarGraph`
#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    /// Each node's position (`custom_data`: the face centre), `None` until initialised.
    pub node_co: Vec<Option<[f32; 3]>>,
    /// Each node's links (`neighbor_links`), in insertion order.
    pub node_links: Vec<Vec<usize>>,
    /// Every link.
    pub links: Vec<Link>,
    /// `custom_data`: the face → island-local index map (island graphs only).
    pub face_island_index_map: Option<Vec<i32>>,
}

impl Graph {
    /// `BLI_astar_graph_init`
    pub(crate) fn new(node_num: usize) -> Graph {
        Graph {
            node_co: vec![None; node_num],
            node_links: vec![Vec::new(); node_num],
            links: Vec::new(),
            face_island_index_map: None,
        }
    }

    /// `BLI_astar_node_link_add`
    pub(crate) fn link_add(&mut self, node1: usize, node2: usize, cost: f32, edge: i32) {
        let l = self.links.len();
        self.links.push(Link {
            nodes: [node1, node2],
            cost,
            edge,
        });
        self.node_links[node1].push(l);
        self.node_links[node2].push(l);
    }
}

/// `BLI_AStarSolution`
#[derive(Clone, Debug, Default)]
pub(crate) struct Solution {
    /// `steps`: the number of nodes of the path (0 when none was found).
    pub steps: i32,
    /// `prev_nodes`
    pub prev_nodes: Vec<i32>,
    /// `prev_links`
    pub prev_links: Vec<Option<usize>>,
    /// `custom_data`: whether the explored paths crossed an inner cut.
    pub crossed_innercut: bool,
    done_nodes: Vec<bool>,
    g_costs: Vec<f32>,
    g_steps: Vec<i32>,
}

impl Solution {
    /// `BLI_astar_solution_init`
    pub(crate) fn new(graph: &Graph) -> Solution {
        let n = graph.node_co.len();
        Solution {
            steps: 0,
            prev_nodes: vec![0; n],
            prev_links: vec![None; n],
            crossed_innercut: false,
            done_nodes: vec![false; n],
            g_costs: vec![0.0; n],
            g_steps: vec![0; n],
        }
    }
}

/// `HeapSimple` of `(cost, node)`.
#[derive(Default)]
struct HeapSimple {
    tree: Vec<(f32, usize)>,
}

impl HeapSimple {
    /// `BLI_heapsimple_insert`
    fn insert(&mut self, value: f32, ptr: usize) {
        let mut i = self.tree.len();
        self.tree.push((value, ptr));
        while i > 0 {
            let p = (i - 1) >> 1;
            if value >= self.tree[p].0 {
                break;
            }
            self.tree[i] = self.tree[p];
            i = p;
        }
        self.tree[i] = (value, ptr);
    }

    /// `BLI_heapsimple_pop_min`
    fn pop_min(&mut self) -> usize {
        let ptr = self.tree[0].1;
        let last = self.tree.pop().expect("pop_min on an empty heap");
        if self.tree.is_empty() {
            return ptr;
        }
        let size = self.tree.len();
        let active = last.0;
        let mut i = 0;
        self.tree[i].0 = active;
        loop {
            let l = (i << 1) + 1;
            let r = l + 1;
            let mut smallest = i;
            if l < size && self.tree[l].0 < active {
                smallest = l;
            }
            if r < size && self.tree[r].0 < self.tree[smallest].0 {
                smallest = r;
            }
            if smallest == i {
                break;
            }
            self.tree[i] = self.tree[smallest];
            i = smallest;
            self.tree[i].0 = active;
        }
        self.tree[i].1 = last.1;
        ptr
    }
}

/// `mesh_remap_calc_loops_astar_f_cost`: the cost walked so far plus the straight distance to
/// the destination; crossing an inner-cut link tags the solution.
fn f_cost(
    graph: &Graph,
    sol: &mut Solution,
    link: Option<usize>,
    node_curr: usize,
    node_next: usize,
    node_dst: usize,
) -> f32 {
    if let Some(l) = link
        && graph.links[l].edge != -1
    {
        sol.crossed_innercut = true;
    }
    let co_next = graph.node_co[node_next].unwrap_or([0.0; 3]);
    let co_dest = graph.node_co[node_dst].unwrap_or([0.0; 3]);
    let walked = match link {
        Some(l) => sol.g_costs[node_curr] + graph.links[l].cost,
        None => 0.0,
    };
    walked + super::math::len_v3v3(co_next, co_dest)
}

/// `BLI_astar_graph_solve`: the path from `src` to `dst` within `max_steps` (0: no limit).
pub(crate) fn solve(
    graph: &Graph,
    src: usize,
    dst: usize,
    sol: &mut Solution,
    max_steps: i32,
) -> bool {
    sol.steps = 0;
    sol.prev_nodes[src] = -1;
    sol.done_nodes.fill(false);
    sol.g_costs.fill(f32::MAX);
    sol.g_costs[src] = 0.0;
    sol.g_steps[src] = 0;
    if src == dst {
        return true;
    }
    let mut todo = HeapSimple::default();
    let c = f_cost(graph, sol, None, usize::MAX, src, dst);
    todo.insert(c, src);
    while !todo.tree.is_empty() {
        let curr = todo.pop_min();
        if sol.done_nodes[curr] {
            continue;
        }
        if max_steps != 0 && sol.g_steps[curr] > max_steps {
            continue;
        }
        if curr == dst {
            sol.steps = sol.g_steps[curr] + 1;
            return true;
        }
        sol.done_nodes[curr] = true;
        for &l in &graph.node_links[curr] {
            let link = &graph.links[l];
            let next = if link.nodes[0] == curr {
                link.nodes[1]
            } else {
                link.nodes[0]
            };
            if sol.done_nodes[next] {
                continue;
            }
            let g_cst = sol.g_costs[curr] + link.cost;
            if g_cst < sol.g_costs[next] {
                sol.prev_nodes[next] = curr as i32;
                sol.prev_links[next] = Some(l);
                sol.g_costs[next] = g_cst;
                sol.g_steps[next] = sol.g_steps[curr] + 1;
                let c = f_cost(graph, sol, Some(l), curr, next, dst);
                todo.insert(c, next);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortest_path_on_a_line_and_innercut_tag() {
        // 0 - 1 - 2 - 3 along x, with a shortcut 0 - 3 that costs more.
        let mut g = Graph::new(4);
        for i in 0..4 {
            g.node_co[i] = Some([i as f32, 0.0, 0.0]);
        }
        g.link_add(0, 1, 1.0, -1);
        g.link_add(1, 2, 1.0, 7);
        g.link_add(2, 3, 1.0, -1);
        g.link_add(0, 3, 5.0, -1);
        let mut s = Solution::new(&g);
        assert!(solve(&g, 0, 3, &mut s, 0));
        assert_eq!(s.steps, 4);
        assert_eq!(s.prev_nodes[3], 2);
        assert_eq!(s.prev_nodes[2], 1);
        assert!(s.crossed_innercut);
        // Step limit: the three-link path needs 3 steps.
        let mut s = Solution::new(&g);
        assert!(solve(&g, 0, 3, &mut s, 1));
        assert_eq!(s.steps, 2, "only the direct link fits in one step");
    }
}
