# WP 2.11: WIDTH, DEPTH and ABSOLUTE offsets on a corner of three edges of a
# non-uniform box (two segments).
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0), scale=(1, 2, 0.5))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0), scale=(1, 2, 0.5))
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Cube'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2])
bpy.ops.mesh.bevel(offset=0.3, segments=2, offset_type='WIDTH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2])
bpy.ops.mesh.bevel(offset=0.3, segments=2, offset_type='DEPTH')
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.002']
bpy.data.objects['Cube.001'].select_set(False)
bpy.data.objects['Cube.002'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='EDGE', indices=[0, 1, 2])
bpy.ops.mesh.bevel(offset=0.3, segments=2, offset_type='ABSOLUTE')
