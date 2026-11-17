# WP 1.19 (T0): deleting a parent clears the children's `parent` but keeps their local transform
# and parent inverse, so they move in world space; deleting the scene camera clears
# `scene.camera`; the active object becomes None.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.delete()
bpy.context.scene.camera.name
