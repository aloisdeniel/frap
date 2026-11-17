# WP 1.21 (T0), transform.translate orient_type GIMBAL: the Euler gimbal axes of the active
# object; with several objects each uses its own; a quaternion object falls back to GLOBAL.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.ops.transform.translate(value=(1, 1, 1), orient_type='GIMBAL')
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='GIMBAL')
bpy.data.objects['Cube'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].select_set(False)
bpy.ops.transform.translate(value=(0, 0, 1), orient_type='GIMBAL')
