# WP 1.21 (T0), transform.rotate of an axis-angle object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Cube'].rotation_axis_angle = (0.3, 1, 0, 0)
bpy.ops.transform.rotate(value=0.5, orient_axis='Y')
