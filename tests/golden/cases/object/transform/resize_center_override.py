# WP 1.21 (T0), transform.resize about center_override.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.resize(value=(2, 2, 2), center_override=(1, 0, 0))
