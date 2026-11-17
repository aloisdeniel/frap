# WP 1.21 (T0), transform.rotate of objects with delta rotations (Euler and quaternion): the
# delta-rotated orientation turns and the delta is taken back out.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].delta_rotation_euler = (0.2, 0, 0)
bpy.data.objects['Cube'].rotation_euler = (0, 0.3, 0)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion = (0.9, 0.1, 0.2, 0.3)
bpy.data.objects['Light'].delta_rotation_quaternion = (0.9, 0.3, 0.2, 0.1)
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
