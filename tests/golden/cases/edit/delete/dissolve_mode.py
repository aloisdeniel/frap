# WP 2.7: mesh.dissolve_mode dispatches on the select mode: vertices, edges (use_verts
# defaults to True there) or faces.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[6])
bpy.ops.mesh.dissolve_mode()
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[18])
bpy.ops.mesh.dissolve_mode()
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1])
bpy.ops.mesh.dissolve_mode()
