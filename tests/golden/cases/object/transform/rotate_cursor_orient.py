# WP 1.21 (T0), transform.rotate orient_type CURSOR.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.rotation_euler = (0.5, 0, 0)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', orient_type='CURSOR')
