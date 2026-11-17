# WP 1.21 (T0), transform.rotate constrained to all three axes has no axis: rotations stay, the
# locations scale about the pivot by cos(angle).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', constraint_axis=(True, True, True))
