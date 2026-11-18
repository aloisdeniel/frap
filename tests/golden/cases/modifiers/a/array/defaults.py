# WP 4.2: ARRAY with its defaults (2 copies, relative offset 1 on X, no merge) on the factory
# cube.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Array', type='ARRAY')
