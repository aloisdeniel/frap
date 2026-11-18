# WP 2.4: select_edge_loop_multi on grid boundary edges with and without the OUTER_CORNERS
# delimiter, and on interior edges (straight loops across the grid), then rings.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_loop_multi()
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop=set())
bpy.ops.frap.select_elements(type='EDGE', indices=[9, 14])
bpy.ops.mesh.select_edge_loop_multi()
bpy.ops.mesh.select_edge_ring_multi()
