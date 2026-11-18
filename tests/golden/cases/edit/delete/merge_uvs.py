# WP 2.7: mesh.merge(uvs=True) also merges the UVs: the corners of the merged vertices get
# the bounding-box centre (CENTER) or the target's averaged UV (FIRST/LAST).
# golden: startup=factory compare=objects,mesh:Cube,mesh:Cube.001,mesh:Cube.002 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.object.select_all(action='DESELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[0, 2])
bpy.ops.mesh.merge(type='CENTER', uvs=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.001']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 5])
bpy.ops.mesh.merge(type='LAST', uvs=True)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Cube.002']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_mode(type='EDGE')
bpy.ops.frap.select_elements(type='EDGE', indices=[0])
bpy.ops.mesh.merge(type='COLLAPSE', uvs=True)
bpy.ops.object.mode_set(mode='OBJECT')
