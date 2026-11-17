# WP 1.21 (T0), object.location_clear / rotation_clear / scale_clear keep the delta channels,
# then clear them with clear_delta=True.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].location = (1, 2, 3)
bpy.data.objects['Cube'].rotation_euler = (0.1, 0.2, 0.3)
bpy.data.objects['Cube'].scale = (2, 3, 4)
bpy.data.objects['Cube'].delta_location = (1, 1, 1)
bpy.data.objects['Cube'].delta_rotation_euler = (0.1, 0.1, 0.1)
bpy.data.objects['Cube'].delta_scale = (2, 2, 2)
bpy.ops.object.location_clear()
bpy.ops.object.rotation_clear()
bpy.ops.object.scale_clear()
bpy.data.objects['Cube'].matrix_world
bpy.ops.object.location_clear(clear_delta=True)
bpy.ops.object.rotation_clear(clear_delta=True)
bpy.ops.object.scale_clear(clear_delta=True)
