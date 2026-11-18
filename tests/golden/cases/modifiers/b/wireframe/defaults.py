# WP 4.3: WIREFRAME with its defaults (thickness 0.02, even offset, replace) on the factory cube.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='M', type='WIREFRAME')
