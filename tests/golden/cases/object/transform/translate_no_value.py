# WP 1.21 (T0), transform.translate without value: an unset value is zero, nothing moves.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.translate()
bpy.ops.transform.translate(constraint_axis=(True, False, False))
