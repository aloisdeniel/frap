# WP 8.3.5 (T1), object.visual_transform_apply without constraints: the evaluated world
# matrix decomposed back into the channels (an unnormalized quaternion becomes unit, axis-angle
# re-derived, Euler compatible, delta channels kept out), through the parent; nothing selected
# is CANCELLED.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].rotation_quaternion = (2, 0.2, 0.4, 0.1)
bpy.data.objects['Cube'].delta_location = (0.5, 0, 0)
bpy.data.objects['Cube'].delta_scale = (2, 1, 1)
bpy.data.objects['Light'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Light'].rotation_axis_angle = (0.7, 1, 1, 0)
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].rotation_euler = (4.0, -3.5, 7.0)
bpy.data.objects['Camera'].delta_rotation_euler = (0.1, 0.2, 0.3)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.object.visual_transform_apply()
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.visual_transform_apply()
