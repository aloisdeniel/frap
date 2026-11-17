# WP 1.21 (T0), transform.translate orient_type GIMBAL on an axis-angle object: Z is the
# rotation axis, X and Y turn with the angle.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Cube'].rotation_axis_angle = (0.7, 0.3, 0.2, 0.9)
bpy.ops.transform.translate(value=(1, 2, 3), orient_type='GIMBAL')
