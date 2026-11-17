# WP 1.21 (T0), GIMBAL with several objects: an axis-angle object uses its axis-angle gimbal
# axes, a quaternion object its local axes, the Euler active object its gimbal axes.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_mode = 'AXIS_ANGLE'
bpy.data.objects['Cube'].rotation_axis_angle = (0.7, 0.3, 0.2, 0.9)
bpy.data.objects['Camera'].rotation_mode = 'QUATERNION'
bpy.data.objects['Camera'].select_set(True)
bpy.data.objects['Light'].select_set(True)
bpy.context.view_layer.objects.active = bpy.data.objects['Light']
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='GIMBAL')
bpy.ops.transform.rotate(value=0.5, orient_axis='Y', orient_type='GIMBAL', constraint_axis=(False, True, False))
bpy.ops.transform.resize(value=(2, 1, 1), orient_type='GIMBAL')
