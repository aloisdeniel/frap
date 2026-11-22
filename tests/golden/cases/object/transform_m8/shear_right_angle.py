# WP 8.3.5 (T1), transform.shear near a right angle in Object Mode (tan(angle) up to 3.7e7) in
# the global and a rotated LOCAL frame: the result shows the rounding of Blender's
# mul_m3_series order, (axismat_inv · smat) · axismat.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-6
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, -0.7, 1.1)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.shear(angle=1.5707963)
bpy.ops.transform.shear(angle=-1.5707, orient_axis='X', orient_axis_ortho='Z')
bpy.ops.transform.shear(angle=1.57, orient_axis='Y', orient_type='LOCAL')
bpy.ops.transform.shear(angle=1.5707963, orient_type='LOCAL')
bpy.ops.transform.shear(angle=-1.5707, orient_axis='X', orient_axis_ortho='Z', orient_type='LOCAL')
