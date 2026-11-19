# WP 4.3: SMOOTH with its defaults (factor 0.5, one iteration, all axes) on Suzanne: each
# vertex moves halfway towards the average of its edges' midpoints.
# golden: startup=empty compare=evaluated:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='SMOOTH')
