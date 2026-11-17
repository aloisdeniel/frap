# WP 1.21 (T0), transform.translate with a parent and its child selected: the child follows its
# parent and is not moved itself.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.ops.object.select_all(action='SELECT')
bpy.ops.transform.translate(value=(1, 0, 0))
