# WP 2.2: mesh.select_all INVERT in edge and in face select mode (the elements of the mode
# are toggled, then the selection is flushed), and DESELECT.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=3, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 4])
bpy.ops.mesh.select_all(action='INVERT')
bpy.data.meshes['Grid'].total_vert_sel
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.frap.select_elements(type='FACE', indices=[4])
bpy.ops.mesh.select_all(action='INVERT')
bpy.data.meshes['Grid'].total_edge_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[1])
bpy.ops.mesh.select_all(action='INVERT')
