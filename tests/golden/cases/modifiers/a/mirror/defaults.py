# WP 4.2: MIRROR with its defaults on the factory cube: X axis about the origin, merge on (no
# vertex is within 0.001 of its image), the copy's faces flipped and its UVs copied.
# golden: startup=factory compare=evaluated:Cube tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
