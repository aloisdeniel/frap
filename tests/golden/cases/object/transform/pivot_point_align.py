# WP 1.21 (T0), tool_settings.use_transform_pivot_point_align: rotate and resize move
# locations only.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.tool_settings.use_transform_pivot_point_align = True
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.resize(value=(2, 2, 2))
bpy.ops.transform.rotate(value=0.5)
