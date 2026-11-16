# WP 1.10: writing `matrix_world` decomposes into location, rotation (current mode) and scale
# relative to the parent; negative scale and shear as Blender (BKE_object_apply_mat4). A nested
# sequence assigned to a matrix property fills it column by column (memory order, verified on
# 5.2.4), so each inner tuple below is a column.
# golden: startup=factory tier=T0 compare=objects tol=1e-5
import bpy
bpy.data.objects['Cube'].parent = bpy.data.objects['Camera']
bpy.data.objects['Cube'].matrix_world = ((1.4486002, -0.9910405, -0.9588511, 0.0), (-0.6741579, -0.6547673, -0.3417467, 0.0), (0.0722849, -0.2853679, 0.4041535, 0.0), (1.0, 2.0, 3.0, 1.0))
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].matrix_world = ((1.0, 0.0, 0.0, 0.0), (0.5, 1.0, 0.0, 0.0), (0.0, 0.0, 1.0, 0.0), (1.0, 2.0, 3.0, 1.0))
bpy.data.objects['Camera'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Camera'].delta_location = (0.5, 0.0, 0.0)
bpy.data.objects['Camera'].matrix_world = ((1.0, 0.0, 0.0, 0.0), (0.0, 0.0, 1.0, 0.0), (0.0, -1.0, 0.0, 0.0), (0.0, 0.0, 4.0, 1.0))
