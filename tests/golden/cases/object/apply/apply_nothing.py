# WP 1.21 (T0), object.transform_apply with only a camera selected (no data to transform) and
# with nothing selected: CANCELLED both times.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.select_all(action='DESELECT')
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.transform_apply()
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.transform_apply()
