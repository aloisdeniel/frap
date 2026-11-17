# WP 1.21 (T0), transform.rotate: without a constraint Blender turns about the negated
# orient_axis (z -0.5); with a one-axis constraint about the axis itself; a two-axis
# constraint turns about the remaining axis.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', constraint_axis=(False, False, True))
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', constraint_axis=(True, False, False))
bpy.ops.transform.rotate(value=0.25, orient_axis='Z', constraint_axis=(True, True, False))
