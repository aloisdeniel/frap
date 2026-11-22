# SPDX-License-Identifier: GPL-3.0-or-later
# Writes the Blender orders the `frap_geom::bvh` tests compare against (WP 8.5):
#   Blender -b --factory-startup --python gen_bvh_fixtures.py -- <out dir>
# mathutils.bvhtree.BVHTree.FromPolygons builds BLI_bvhtree_new(n, 0.0, 4, 6) with one leaf
# per triangle, in order; overlap() returns BLI_bvhtree_overlap's pairs (filtered by a real
# triangle intersection), find_nearest_range() the callback order of BLI_bvhtree_range_query.
# The triangles come from the same LCG as bvh/tests.rs (multiples of 1/1024, exact in f32).
import sys
from mathutils.bvhtree import BVHTree

MASK = (1 << 64) - 1


class Lcg:
    def __init__(self, seed):
        self.s = seed

    def next(self):
        self.s = (self.s * 6364136223846793005 + 1442695040888963407) & MASK
        return self.s >> 33

    def coord(self):
        return (self.next() % 8192) / 1024.0 - 4.0

    def offset(self):
        return (self.next() % 1024) / 1024.0 - 0.5


def triangles(seed, n):
    r = Lcg(seed)
    out = []
    for _ in range(n):
        c = [r.coord(), r.coord(), r.coord()]
        tri = []
        for _ in range(3):
            tri.append((c[0] + r.offset(), c[1] + r.offset(), c[2] + r.offset()))
        out.append(tri)
    return out


def tree(tris):
    verts = [p for t in tris for p in t]
    polys = [(3 * i, 3 * i + 1, 3 * i + 2) for i in range(len(tris))]
    return BVHTree.FromPolygons(verts, polys, epsilon=0.0)


out_dir = sys.argv[sys.argv.index("--") + 1]


def write(name, header, pairs):
    with open(f"{out_dir}/{name}", "w") as f:
        f.write(f"# Blender 5.2.4 mathutils.bvhtree: {header}\n")
        for a, b in pairs:
            f.write(f"{a} {b}\n")


for name, (sa, na, sb, nb) in {
    "overlap_small.txt": (1, 400, 2, 300),
    "overlap_large.txt": (3, 1600, 4, 1200),
}.items():
    pairs = tree(triangles(sa, na)).overlap(tree(triangles(sb, nb)))
    write(name, f"overlap of triangles({sa}, {na}) with triangles({sb}, {nb})", pairs)

t = tree(triangles(5, 700))
rows = []
for q, query in enumerate(triangles(6, 30)):
    for (_co, _no, index, _dist) in t.find_nearest_range(query[0], 1.5):
        rows.append((q, index))
write("range.txt", "find_nearest_range(triangles(6, 30)[q][0], 1.5) on triangles(5, 700): query index", rows)
