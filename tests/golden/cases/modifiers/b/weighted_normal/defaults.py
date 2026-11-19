# WP 4.3: WEIGHTED_NORMAL with its defaults (face area, weight 50) on smooth Suzanne: custom
# normals set from the vertex normals.
# golden: startup=empty compare=evaluated:Suzanne tier=T1
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.object.shade_smooth()
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='WEIGHTED_NORMAL')
