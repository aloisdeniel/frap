# WP 1.21 (T0), transform.resize about the 3D cursor with a negative factor (mirror).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.tool_settings.transform_pivot_point = 'CURSOR'
bpy.context.scene.cursor.location = (1, 1, 1)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.resize(value=(2, 3, -1))
