# WP 1.21 (T0), object.parent_set on a child of another parent: without keep_transform it jumps
# to its channels, with keep_transform its channels take its world matrix first.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Camera']
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Light'].select_set(True)
bpy.ops.object.parent_set()
bpy.data.objects['Light'].parent = bpy.data.objects['Camera']
bpy.ops.object.parent_set(keep_transform=True)
