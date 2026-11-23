# M8 stabilization round 2 (fuzz seed 0x8be79e98a2cec9f3): `select_nth` leaves `BM_ELEM_TAG` on
# the vertices its walker visited (`walker_deselect_nth`), and `edge_split(type='VERT')` runs
# `split_edges(use_verts=True)`, which only adds tags: every vertex of the cube is split (24
# vertices), not only the four selected ones.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T2 tol=0.0001
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_nth(nth=1, offset=1)
bpy.ops.mesh.edge_split(type='VERT')
