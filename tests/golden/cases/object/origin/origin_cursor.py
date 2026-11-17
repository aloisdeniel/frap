# WP 1.21 (T1), object.origin_set(ORIGIN_CURSOR) on a rotated object and on a parented one.
# golden: startup=factory compare=objects,context,mesh:Cube tier=T1
import bpy
bpy.context.scene.cursor.location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Cube'].parent = bpy.data.objects['Light']
bpy.ops.object.origin_set(type='ORIGIN_CURSOR')
