# WP 1.21 (T0), transform.rotate orient_type LOCAL with several objects: with a constraint each
# object turns about its own axis (around the median), without one about the active object's
# (negated) axis.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='X', orient_type='LOCAL', constraint_axis=(True, False, False))
bpy.ops.transform.rotate(value=0.3, orient_axis='Y', orient_type='LOCAL')
