# WP 2.4: edit/select/loop_multi_grid.py cut after its line 10,
#   bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop=set())
# so that call's effect reaches the dump (later lines of the full case replace it).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_loop_multi()
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.select_edge_loop_multi(delimit_edge_loop=set())
