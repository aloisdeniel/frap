# WP 2.7: every delete type on the top face of the default cube (research App. B.6): VERT
# leaves V4 E4 F1, EDGE V8 E8 F1, FACE V8 E12 F5, EDGE_FACE V8 E8 F1, ONLY_FACE V8 E12 F5.
# golden: startup=factory compare=objects,mesh:Cube,mesh:Cube.001,mesh:Cube.002,mesh:Cube.003,mesh:Cube.004 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='VERT')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='EDGE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.002']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='FACE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.003']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='EDGE_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.004']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='FACE', indices=[5])
bpy.ops.mesh.delete(type='ONLY_FACE')
bpy.ops.object.mode_set(mode='OBJECT')
