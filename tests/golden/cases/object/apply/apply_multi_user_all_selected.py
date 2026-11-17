# WP 1.21 (T1), object.transform_apply when every selected object shares the active object's
# mesh: the mesh is transformed once, the other users get the inverse transform.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube2'].select_set(True)
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.data.objects['Cube2'].location = (0, 3, 0)
bpy.data.objects['Cube2'].rotation_euler = (0, 0, 0.5)
bpy.ops.object.transform_apply()
