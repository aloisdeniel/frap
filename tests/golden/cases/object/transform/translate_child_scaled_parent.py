# WP 1.21 (T0), transform.translate of a child of a scaled parent: the global offset is converted
# to the parent space (location changes by half for a parent scaled 2).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.translate(value=(1, 0, 0))
