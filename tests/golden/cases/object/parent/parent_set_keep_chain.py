# WP 1.21 (T0), object.parent_set(keep_transform=True) of a parent and its child: each keeps the
# world matrix evaluated before the operator.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.data.objects['Light'].location = (0, 0, 9)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.ops.object.parent_set(keep_transform=True)
