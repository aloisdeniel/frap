# WP 4.3: NORMAL_EDIT with its defaults (RADIAL from the bounds' centre, COPY) on smooth
# Suzanne.
# golden: startup=empty compare=evaluated:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='NORMAL_EDIT')
