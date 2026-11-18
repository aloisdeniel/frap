# WP 2.7: mesh.delete(type='ONLY_FACE') removes only the faces; type='EDGE_FACE' removes the
# selected edges (with their faces) and the selected faces but keeps every vertex.
# golden: startup=factory compare=objects,context,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 5, 15])
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.frap.select_elements(type='EDGE', indices=[16, 19])
bpy.ops.frap.select_elements(type='FACE', indices=[2], action='ADD')
bpy.ops.mesh.delete(type='EDGE_FACE')
