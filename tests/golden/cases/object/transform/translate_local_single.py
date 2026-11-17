# WP 1.21 (T0), transform.translate orient_type LOCAL on one rotated object: along its own X.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='LOCAL')
