# WP 1.21 (T1), object.transform_apply(scale only).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.3, 0, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 0.5)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
bpy.ops.object.transform_apply(location=False, rotation=False, scale=False)
