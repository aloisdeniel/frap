# WP 8.2: BEVEL on Suzanne with the angle limit of 1 rad (57°), three segments and a
# profile of 0.3 (irregular corners, UV islands).
# golden: startup=factory compare=evaluated:Suzanne tier=T1
import bpy
bpy.ops.object.delete()
bpy.ops.mesh.primitive_monkey_add()
bpy.data.objects['Suzanne'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Suzanne'].modifiers['Bevel'].width = 0.02
bpy.data.objects['Suzanne'].modifiers['Bevel'].segments = 3
bpy.data.objects['Suzanne'].modifiers['Bevel'].profile = 0.3
bpy.data.objects['Suzanne'].modifiers['Bevel'].angle_limit = 1.0
