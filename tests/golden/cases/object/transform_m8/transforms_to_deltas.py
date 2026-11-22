# WP 8.3.5 (T1), object.transforms_to_deltas: every mode, reset_values on and off, Euler (the
# old delta rotation applied), quaternion and axis-angle (skipped) rotations; the deltas are
# read back through location, rotation and scale assignments so that the dump shows them;
# nothing selected warns and cancels.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.2, 0.1)
bpy.data.objects['Cube'].delta_rotation_euler = (0.1, 0.5, -0.2)
bpy.data.objects['Cube'].scale = (2, 3, 0.5)
bpy.data.objects['Cube'].delta_scale = (1.5, 1, 1)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion = (0.9, 0.1, 0.3, 0.2)
bpy.data.objects['Light'].delta_rotation_quaternion = (0.8, 0.4, 0.1, 0.2)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Camera'].rotation_mode = 'AXIS_ANGLE'
bpy.ops.object.transforms_to_deltas(mode='LOC', reset_values=False)
bpy.ops.object.transforms_to_deltas(mode='ROT')
bpy.ops.object.transforms_to_deltas(mode='SCALE', reset_values=False)
bpy.ops.object.transforms_to_deltas()
bpy.data.objects['Cube'].rotation_mode = 'ZXY'
bpy.data.objects['Cube'].rotation_euler = (0.4, 0.1, 0.2)
bpy.ops.object.transforms_to_deltas(mode='ROT')
bpy.data.objects['Cube'].location = bpy.data.objects['Cube'].delta_location
bpy.data.objects['Cube'].rotation_euler = bpy.data.objects['Cube'].delta_rotation_euler
bpy.data.objects['Cube'].scale = bpy.data.objects['Cube'].delta_scale
bpy.data.objects['Light'].location = bpy.data.objects['Light'].delta_location
bpy.data.objects['Light'].rotation_quaternion = bpy.data.objects['Light'].delta_rotation_quaternion
bpy.data.objects['Light'].scale = bpy.data.objects['Light'].delta_scale
bpy.data.objects['Camera'].location = bpy.data.objects['Camera'].delta_location
bpy.data.objects['Camera'].rotation_axis_angle = (bpy.data.objects['Camera'].delta_rotation_euler[0], 1, 0, 0)
bpy.data.objects['Camera'].scale = bpy.data.objects['Camera'].delta_scale
bpy.ops.object.select_all(action='DESELECT')
bpy.ops.object.transforms_to_deltas()
