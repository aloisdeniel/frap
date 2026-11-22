# WP 8.3.5 (T1), transform.shear axis rules: an unset axis equal to the other one is reset
# (orient_axis='X' alone shears X-normal along Y; orient_axis_ortho='Z' alone is degenerate
# once the set axis is read back), and two equal axes given collapse onto the pivot.
# golden: startup=factory compare=objects,context tier=T1 tol=1e-5
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.shear(angle=0.3, orient_axis='X')
bpy.ops.transform.shear(angle=0.2, orient_axis='Y')
bpy.ops.transform.shear(angle=-0.4, orient_axis_ortho='Y')
bpy.data.objects['Camera'].select_set(False)
bpy.ops.transform.shear(angle=0.5, orient_axis_ortho='Z')
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.shear(angle=0.25, orient_axis='Y', orient_axis_ortho='Y')
