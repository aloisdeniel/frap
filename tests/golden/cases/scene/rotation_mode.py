# WP 1.10: assigning `rotation_mode` converts the stored rotation (BKE_rotMode_change_values).
# golden: startup=factory tier=T0 compare=objects tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, -0.7, 1.9)
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Camera'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion = (2.0, 0.5, 0.1, -0.3)
bpy.data.objects['Light'].rotation_mode = 'ZXY'
