# WP 1.21 (T0), transform.rotate of a quaternion object about center_override.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.ops.transform.rotate(value=0.5, orient_axis='Y', center_override=(0, 0, 0))
