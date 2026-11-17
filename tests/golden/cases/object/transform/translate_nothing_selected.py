# WP 1.21 (T0), transform.translate with no visible selected object: CANCELLED (a hidden
# selected object does not count and does not move).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Light'].hide_set(True)
bpy.ops.transform.translate(value=(1, 0, 0))
bpy.data.objects['Cube'].select_set(False)
bpy.ops.transform.translate(value=(1, 2, 3))
