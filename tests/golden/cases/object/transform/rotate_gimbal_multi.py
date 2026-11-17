# WP 1.21 (T0), transform.rotate orient_type GIMBAL constrained to Y with several objects: each
# object turns about its own gimbal Y axis.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='Y', orient_type='GIMBAL', constraint_axis=(False, True, False))
