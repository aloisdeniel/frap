# WP 8.3.5 (T1), transform.shear in Object Mode with every orientation: LOCAL and NORMAL from
# the active object, GIMBAL, VIEW (the Layout view), CURSOR, PARENT, an orient_matrix.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.context.scene.cursor.rotation_euler = (0.1, 0.2, 0.9)
bpy.ops.transform.shear(angle=0.4, orient_type='LOCAL')
bpy.ops.transform.shear(angle=0.3, orient_type='NORMAL', orient_axis='X', orient_axis_ortho='Z')
bpy.ops.transform.shear(angle=0.2, orient_type='GIMBAL', orient_axis='Y', orient_axis_ortho='X')
bpy.ops.transform.shear(angle=0.35, orient_type='VIEW')
bpy.ops.transform.shear(angle=-0.3, orient_type='CURSOR', orient_axis='Y', orient_axis_ortho='Z')
bpy.ops.transform.shear(angle=0.25, orient_type='PARENT')
bpy.ops.transform.shear(angle=0.2, orient_matrix=((0, 1, 0), (-1, 0, 0), (0, 0, 1)), orient_matrix_type='GLOBAL')
