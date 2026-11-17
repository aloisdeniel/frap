# WP 1.21 (T0), transform.translate orient_type NORMAL in Object Mode: the active object's local
# axes, shared by every object (not per object, unlike LOCAL).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='NORMAL')
