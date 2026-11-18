# WP 4.5: BEVEL with its defaults on the factory cube: limit method ANGLE (30°) bevels every
# edge by 0.1 with one segment (24 vertices, 26 faces); the UVs are interpolated.
# golden: startup=factory compare=evaluated:Cube tier=T2
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].limit_method
bpy.data.objects['Cube'].modifiers['Bevel'].width
