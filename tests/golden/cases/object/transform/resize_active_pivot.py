# WP 1.21 (T0), transform.resize about the active object (ACTIVE_ELEMENT).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.tool_settings.transform_pivot_point = 'ACTIVE_ELEMENT'
bpy.ops.object.select_all(action='SELECT')
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
