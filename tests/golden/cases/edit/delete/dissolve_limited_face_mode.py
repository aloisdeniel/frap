# WP 2.7: mesh.dissolve_limited in face select mode only dissolves the edges and vertices
# that no unselected face uses; the merged faces are selected.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 4, 5, 15])
bpy.ops.mesh.dissolve_limited(delimit={'NORMAL', 'MATERIAL'})
