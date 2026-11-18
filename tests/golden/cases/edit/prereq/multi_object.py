# WP 2.2: multi-object Edit Mode through object.mode_set: the active object and every selected
# mesh object enter, the selection is made per object, and leaving writes every mesh back.
# golden: startup=factory compare=objects,context,mesh:Cube,mesh:Cube.001,mesh:Plane tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_plane_add(location=(-3, 0, 0))
bpy.data.objects['Cube'].select_set(True)
bpy.data.objects['Cube.001'].select_set(True)
bpy.ops.object.mode_set(mode='EDIT')
bpy.data.objects['Cube'].mode
bpy.data.objects['Cube.001'].mode
bpy.data.objects['Plane'].mode
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[1, 2])
bpy.ops.frap.select_elements(type='FACE', indices=[3], object='Cube')
bpy.ops.object.mode_set(mode='OBJECT')
