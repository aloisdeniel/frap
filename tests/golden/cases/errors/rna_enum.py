# WP 1.26: enum assignments with an unknown item (code RNA_ENUM, TypeError) list the valid items
# in Blender's order; the old 4.x engine name is refused like any other; items are
# case-sensitive. The valid writes at the end are compared.
# golden: startup=factory tier=T0 compare=objects
import bpy
bpy.data.objects['Cube'].rotation_mode = 'FOO'
bpy.data.objects['Cube'].rotation_mode = 'xyz'
bpy.data.objects['Cube'].display_type = 'FOO'
bpy.data.scenes['Scene'].render.engine = 'BLENDER_EEVEE_NEXT'
bpy.data.scenes['Scene'].render.engine = 'CYCLESX'
bpy.data.scenes['Scene'].cursor.rotation_mode = 'FOO'
bpy.data.scenes['Scene'].unit_settings.system = 'FOO'
bpy.data.objects['Cube'].rotation_mode = 'ZYX'
bpy.data.objects['Camera'].rotation_mode = 'AXIS_ANGLE'
