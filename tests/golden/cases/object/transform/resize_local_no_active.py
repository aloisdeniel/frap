# WP 1.21 (T0), transform.resize orient_type LOCAL with several objects and no active object:
# the orientation matrix is the identity but each object still uses its own axes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = None
bpy.ops.transform.resize(value=(2, 1, 1), orient_type='LOCAL')
