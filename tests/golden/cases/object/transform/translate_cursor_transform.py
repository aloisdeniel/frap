# WP 1.21 (T0), transform.translate(cursor_transform=True): moves the 3D cursor (in the active
# object's LOCAL axes here), not the objects.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.ops.transform.translate(value=(1, 0, 0), orient_type='LOCAL', cursor_transform=True)
bpy.context.scene.cursor.location
