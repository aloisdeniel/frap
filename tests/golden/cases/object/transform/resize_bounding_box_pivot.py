# WP 1.21 (T0), transform.resize about the bounding-box center of the selection.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.tool_settings.transform_pivot_point = 'BOUNDING_BOX_CENTER'
bpy.ops.object.select_all(action='SELECT')
bpy.ops.transform.resize(value=(0.5, 0.5, 0.5))
