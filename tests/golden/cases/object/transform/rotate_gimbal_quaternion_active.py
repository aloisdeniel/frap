# WP 1.21 (T0), transform.rotate orient_type GIMBAL with a quaternion active object and an
# Euler object also selected: the Euler object still turns about its own gimbal X.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0.3, 0.4, 0.5)
bpy.data.objects['Light'].rotation_mode = 'QUATERNION'
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.rotate(value=0.5, orient_axis='X', orient_type='GIMBAL', constraint_axis=(True, False, False))
