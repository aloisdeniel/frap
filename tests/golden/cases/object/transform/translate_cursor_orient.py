# WP 1.21 (T0), transform.translate orient_type CURSOR: the 3D cursor's rotation.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.cursor.rotation_euler = (0.5, 0, 0.3)
bpy.ops.transform.translate(value=(1, 1, 0), orient_type='CURSOR')
