# WP 1.21 (T0), object.rotation_clear in quaternion and axis-angle mode (with clear_delta), on a
# parent and its child.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].rotation_quaternion = (0.5, 0.5, 0.5, 0.5)
bpy.data.objects['Cube'].delta_rotation_quaternion = (0.5, 0.5, 0.5, 0.5)
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Light'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Light'].rotation_axis_angle = (0.5, 1, 0, 0)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.rotation_clear(clear_delta=True)
bpy.data.objects['Cube'].delta_rotation_quaternion
bpy.ops.object.location_clear()
