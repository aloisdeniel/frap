# WP 1.20 (T0), object.empty_add: an 'Empty' without data; type and radius go to the display
# settings, scale is ignored; the cursor and align place it.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.empty_add()
bpy.ops.object.empty_add(type='CUBE', radius=3, scale=(2, 2, 2), location=(1, 1, 1))
bpy.ops.object.empty_add(type='SINGLE_ARROW', align='VIEW')
bpy.context.scene.cursor.rotation_euler = (0.3, 0.2, 0.1)
bpy.ops.object.empty_add(type='SPHERE', radius=0, align='CURSOR')
bpy.ops.object.empty_add.poll()
bpy.data.objects['Empty.001'].empty_display_size
