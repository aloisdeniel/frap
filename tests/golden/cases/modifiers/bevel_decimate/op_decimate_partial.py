# WP 4.5: mesh.decimate on a partial selection: only edges between selected vertices collapse
# and the ratio is rescaled to the faces touching the selection; in face select mode, with
# use_vertex_group and no vertex group (WARNING "No active vertex group": the selection is
# used).
# golden: startup=factory compare=objects,context,mesh:Grid,mesh:Suzanne tier=T1
import bpy
bpy.data.objects.remove(bpy.data.objects['Cube'])
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=6)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 7, 8, 9, 10, 14, 15, 16, 17])
bpy.ops.mesh.decimate(ratio=0.2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_monkey_add(location=(0, 4, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='FACE')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='FACE', indices=[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])
bpy.ops.mesh.decimate(ratio=0.5, use_vertex_group=True)
