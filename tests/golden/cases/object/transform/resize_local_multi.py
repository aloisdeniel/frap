# WP 1.21 (T0), transform.resize orient_type LOCAL with several objects: each object scales
# along its own axes, locations scale in each object's axes about the median.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.data.objects['Cube'].rotation_euler = (0, 0, 0.5)
bpy.data.objects['Light'].select_set(True)
bpy.ops.transform.resize(value=(2, 1, 1), orient_type='LOCAL')
