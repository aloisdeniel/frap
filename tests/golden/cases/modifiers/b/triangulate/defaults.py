# WP 4.3: TRIANGULATE with its defaults (shortest diagonal, beauty n-gons) on the factory cube.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='M', type='TRIANGULATE')
