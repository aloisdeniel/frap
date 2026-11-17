# WP 1.21 (T0), transform.rotate keeps Euler angles compatible with the current ones (3 + 1 = 4,
# not 4 - 2 pi).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 3.0)
bpy.ops.transform.rotate(value=1.0, constraint_axis=(False, False, True))
