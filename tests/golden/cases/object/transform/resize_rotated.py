# WP 1.21 (T0), transform.resize of a rotated object: GLOBAL loses the shear (scale (1.8195,
# 1.2998, 1)), LOCAL scales along the object's axes; the rotation never changes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.ops.transform.resize(value=(2, 1, 1))
bpy.data.objects['Cube'].select_set(False)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.resize(value=(2, 1, 1), orient_type='LOCAL')
