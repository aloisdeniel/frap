# WP 1.21 (T1), object.transform_apply with a negative scale flips the face winding (first corner
# kept).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Cube'].scale = (-1, 1, 1)
bpy.ops.object.transform_apply()
