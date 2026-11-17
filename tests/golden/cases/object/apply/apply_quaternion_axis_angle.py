# WP 1.21 (T1), object.transform_apply of quaternion and axis-angle objects (delta Euler
# rotation ignored in axis-angle mode).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].rotation_quaternion = (0.9, 0.3, 0.1, 0.2)
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.ops.object.transform_apply()
bpy.data.objects['Cube'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Cube'].rotation_axis_angle = (0.5, 1, 0, 0)
bpy.data.objects['Cube'].delta_rotation_euler = (0, 0, 0.3)
bpy.ops.object.transform_apply()
