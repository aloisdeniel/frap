# WP 1.21 (T0), transform.rotate without value changes nothing.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate()
