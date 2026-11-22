# WP 8.3.5 (T1), object.randomize_transform per seed (CPython's random module): location,
# rotation (a quaternion object switched to XYZ and back) and scale, scale_even, use_delta
# (read back through the channels), parts switched off (the sequence keeps its draws), and the
# Object Mode poll.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].rotation_quaternion = (0.9, 0.1, 0.3, 0.2)
bpy.ops.object.randomize_transform(loc=(1, 2, 3), rot=(0.5, 1.0, 1.5), scale=(1.5, 2, 0.8))
bpy.ops.object.randomize_transform(random_seed=7, loc=(0.5, 0.5, 0.5), use_rot=False, scale_even=True, scale=(1.2, 1, 1))
bpy.ops.object.randomize_transform(random_seed=9999, use_loc=False, rot=(3.0, 0.1, 0.2), use_scale=False)
bpy.ops.object.randomize_transform(random_seed=42, use_delta=True, loc=(2, 2, 2), rot=(0.3, 0.3, 0.3), scale=(1.4, 1.4, 1.4))
bpy.ops.object.randomize_transform(random_seed=3)
bpy.data.objects['Cube'].location = bpy.data.objects['Cube'].delta_location
bpy.data.objects['Cube'].rotation_euler = bpy.data.objects['Cube'].delta_rotation_euler
bpy.data.objects['Cube'].scale = bpy.data.objects['Cube'].delta_scale
bpy.data.objects['Camera'].location = bpy.data.objects['Camera'].delta_location
bpy.data.objects['Camera'].scale = bpy.data.objects['Camera'].delta_scale
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.object.randomize_transform(random_seed=1)
