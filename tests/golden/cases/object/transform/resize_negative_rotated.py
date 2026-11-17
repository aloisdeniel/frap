# WP 1.21 (T0), transform.resize with a negative factor on a rotated object.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.2, 0.3, 0.4)
bpy.ops.transform.resize(value=(-1, 2, 1))
