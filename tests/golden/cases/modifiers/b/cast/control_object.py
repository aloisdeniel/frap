# WP 4.3: CAST about a control object: its position only (use_transform off), and its full
# transform (rotation and non-uniform scale) for a CUBOID on a moved cube.
# golden: startup=factory compare=evaluated:Cube,evaluated:Suzanne tier=T1
import bpy
bpy.ops.object.empty_add(location=(0.5, 0.2, 0.3), rotation=(0.3, 0, 0.5))
bpy.data.objects['Empty'].scale = (1, 2, 1)
bpy.ops.mesh.primitive_monkey_add(location=(0, 3, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='M', type='CAST')
bpy.data.objects['Suzanne'].modifiers['M'].object = bpy.data.objects['Empty']
bpy.data.objects['Suzanne'].modifiers['M'].factor = 0.9
bpy.data.objects['Cube'].location = (0.3, 0, 0.4)
bpy.data.objects['Cube'].modifiers.new(name='M', type='CAST')
bpy.data.objects['Cube'].modifiers['M'].cast_type = 'CUBOID'
bpy.data.objects['Cube'].modifiers['M'].use_transform = True
bpy.data.objects['Cube'].modifiers['M'].factor = 0.6
bpy.data.objects['Cube'].modifiers['M'].object = bpy.data.objects['Empty']
