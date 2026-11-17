# WP 1.21 (T0), transform.translate with snap options: a non-interactive call ignores them.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.translate(value=(1, 0, 0), snap=True, snap_point=(5, 5, 5))
