# WP 1.21 (T0), the clear operators with nothing selected: CANCELLED.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.location_clear()
bpy.ops.object.rotation_clear()
bpy.ops.object.scale_clear()
