# WP 1.21 (T0), transform.rotate of several objects about their median point.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z')
