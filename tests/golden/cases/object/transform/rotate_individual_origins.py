# WP 1.21 (T0), transform.rotate with pivot INDIVIDUAL_ORIGINS: each object turns in place; a
# child of a selected parent rotates too (in place, location kept) and inherits its parent's
# turn.
# golden: startup=factory compare=objects,context tier=T0
import bpy
bpy.context.scene.tool_settings.transform_pivot_point = 'INDIVIDUAL_ORIGINS'
bpy.data.objects['Light'].parent = bpy.data.objects['Cube']
bpy.data.objects['Cube'].location = (1, 0, 0)
bpy.data.objects['Light'].select_set(True)
bpy.data.objects['Camera'].select_set(True)
bpy.ops.transform.rotate(value=0.5, orient_axis='X')
