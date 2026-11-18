# WP 2.7: mesh.delete(type='FACE') on the grid removes the selected faces and the edges and
# vertices that only they used (edges shared with unselected faces stay).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[5, 6, 9, 10])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.delete(type='FACE')
