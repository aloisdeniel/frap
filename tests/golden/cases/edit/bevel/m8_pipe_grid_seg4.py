# WP 8.2: a straight run of grid edges beveled with four segments: the crossing
# vertices are pipes (`pipe_adj_vmesh`, two collinear beveled edges).
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='EDGE', indices=[6, 9, 11, 14, 17, 20, 1, 4])
bpy.ops.mesh.bevel(offset=0.1, segments=4, profile=0.3)
