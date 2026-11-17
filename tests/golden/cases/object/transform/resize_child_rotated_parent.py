# WP 1.21 (T0), transform.resize of a child of a rotated parent.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.resize(value=(2, 1, 1))
