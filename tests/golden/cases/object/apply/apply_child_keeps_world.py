# WP 1.21 (T1), object.transform_apply on a parent: the child keeps its world matrix (its
# channels take the decomposition, its parent inverse the inverse of the new parent matrix).
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.7)
bpy.data.objects['Cube'].scale = (2, 2, 2)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.transform_apply()
