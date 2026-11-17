# WP 1.21 (T0), transform.translate orient_type VIEW: the axes of the virtual 3D View (D-10).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='VIEW')
