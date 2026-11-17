# WP 1.21 (T0), object.transform_apply poll: Object Mode only (an active object in Edit Mode is
# not reachable in M1, so the poll() call is checked in Object Mode).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.object.transform_apply.poll()
