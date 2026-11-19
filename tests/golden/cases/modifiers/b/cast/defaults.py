# WP 4.3: CAST with its defaults (SPHERE, factor 0.5, size from the average distance to the
# origin) on the factory cube.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='M', type='CAST')
