# WP 1.21 (T0), transform.translate orient_type PARENT: the active object's parent's axes; an
# object without parent falls back to GLOBAL.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='PARENT')
bpy.data.objects['Light'].select_set(False)
bpy.data.objects['Camera'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Camera']
bpy.ops.transform.translate(value=(0, 1, 0), orient_type='PARENT')
