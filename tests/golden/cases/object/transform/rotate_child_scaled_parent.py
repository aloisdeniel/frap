# WP 1.21 (T0), transform.rotate of a child of a rotated, non-uniformly scaled parent, and of a
# ZYX child of a scaled parent.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].scale = (2, 1, 1)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
bpy.data.objects['Camera'].parent = bpy.data.objects['Light']
bpy.data.objects['Camera'].rotation_mode = 'ZYX'
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.rotate(value=0.7, orient_axis='Y', constraint_axis=(False, True, False))
