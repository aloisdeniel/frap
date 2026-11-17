# WP 1.21 (T0), object.transform_apply on an Empty (scale into empty_display_size, channels
# reset), a camera and a point light (skipped).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects.new('Empty', None)
bpy.context.scene.collection.objects.link(bpy.data.objects['Empty'])
bpy.data.objects['Empty'].location = (1, 1, 1)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Empty'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Empty']
bpy.data.objects['Empty'].scale = (2, 3, 1)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.transform_apply()
bpy.data.objects['Empty'].empty_display_size
