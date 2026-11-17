# WP 1.21 (T0), transform.translate: an array value constrains to every axis, so
# constraint_axis is ignored (Blender moves by the whole value); several selected objects move
# together.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.translate(value=(1, 2, 3), constraint_axis=(True, False, False))
