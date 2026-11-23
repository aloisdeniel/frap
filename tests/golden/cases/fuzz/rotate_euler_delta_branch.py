# M8 stabilization round 3 (fuzz seed 0x675e60e10fc079b5, run 0x5eed8e90 trial 5049): rotating an
# object with an Euler delta rotation. `ElementRotation_ex` composes rot and drot as quaternions
# (`add_eul_euleul`), takes the compatible Euler of the rotated matrix and removes the delta with
# `sub_eul_euleul`, whose `quat_to_eulO` picks the branch: frap took the compatible branch of the
# composed matrix (the same rotation, another Euler triple).
# golden: startup=factory compare=objects tier=T2 tol=0.0001
import bpy
bpy.ops.object.select_pattern(pattern='Light', case_sensitive=True, extend=False)
bpy.ops.object.randomize_transform(random_seed=16, use_delta=True, use_loc=False, rot=(0.55, 0.67, 0.8), use_scale=False, scale_even=True)
bpy.ops.transform.rotate(value=1.71, orient_axis='X')
bpy.ops.object.select_pattern(pattern='Cube', case_sensitive=True, extend=False)
bpy.data.objects['Cube'].rotation_mode = 'ZXY'
bpy.data.objects['Cube'].rotation_euler = (0.4, -1.2, 2.9)
bpy.data.objects['Cube'].delta_rotation_euler = (2.1, 0.3, -1.4)
bpy.ops.transform.rotate(value=-2.6, orient_axis='Y')
bpy.ops.transform.rotate(value=0.9, orient_axis='Z')
