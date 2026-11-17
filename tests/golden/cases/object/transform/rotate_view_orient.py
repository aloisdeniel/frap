# WP 1.21 (T0), transform.rotate orient_type VIEW (virtual 3D View, D-10).
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.ops.transform.rotate(value=0.5, orient_axis='Z', orient_type='VIEW')
