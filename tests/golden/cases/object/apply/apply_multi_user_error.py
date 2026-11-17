# WP 1.21 (T0), object.transform_apply on shared data with another, unselected user:
# RuntimeError "Cannot apply to a multi user: ...", nothing changes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('Cube2', bpy.data.meshes['Cube'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cube2'])
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.ops.object.transform_apply()
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Cube2'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.transform_apply()
