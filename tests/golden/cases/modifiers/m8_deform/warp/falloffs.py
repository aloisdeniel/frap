# WP 8.3.12: WARP from one empty to another (moved, rotated, scaled) with every
# `falloff_type` (`CURVE` with the default curve), `use_volume_preserve` (blended matrices), a
# negative strength, a vertex group and a falloff radius that leaves part of the grid alone.
# golden: startup=empty compare=evaluated:None,evaluated:Curve,evaluated:Smooth,evaluated:Sphere,evaluated:Root,evaluated:InvSquare,evaluated:Sharp,evaluated:Linear,evaluated:Const,evaluated:Volume,evaluated:Negative,evaluated:Group tier=T2
import bpy
bpy.ops.object.empty_add(location=(0.3, 0.2, 0))
bpy.context.object.name = 'From'
bpy.ops.object.empty_add(location=(0.6, -0.1, 0.4), rotation=(0.2, 0.1, 0.5))
bpy.context.object.name = 'To'
bpy.data.objects['To'].scale = (1.1, 0.9, 1.2)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'None'
bpy.data.objects['None'].modifiers.new(name='M', type='WARP')
bpy.data.objects['None'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['None'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['None'].modifiers['M'].falloff_type = 'NONE'
bpy.data.objects['None'].modifiers['M'].strength = 0.7
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Curve'
bpy.data.objects['Curve'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Curve'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Curve'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Curve'].modifiers['M'].falloff_type = 'CURVE'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Smooth'
bpy.data.objects['Smooth'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Smooth'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Smooth'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Sphere'
bpy.data.objects['Sphere'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Sphere'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Sphere'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Sphere'].modifiers['M'].falloff_type = 'SPHERE'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Root'
bpy.data.objects['Root'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Root'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Root'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Root'].modifiers['M'].falloff_type = 'ROOT'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'InvSquare'
bpy.data.objects['InvSquare'].modifiers.new(name='M', type='WARP')
bpy.data.objects['InvSquare'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['InvSquare'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['InvSquare'].modifiers['M'].falloff_type = 'INVERSE_SQUARE'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Sharp'
bpy.data.objects['Sharp'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Sharp'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Sharp'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Sharp'].modifiers['M'].falloff_type = 'SHARP'
bpy.data.objects['Sharp'].modifiers['M'].falloff_radius = 1.3
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Linear'
bpy.data.objects['Linear'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Linear'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Linear'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Linear'].modifiers['M'].falloff_type = 'LINEAR'
bpy.data.objects['Linear'].modifiers['M'].falloff_radius = 0.8
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Const'
bpy.data.objects['Const'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Const'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Const'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Const'].modifiers['M'].falloff_type = 'CONSTANT'
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Volume'
bpy.data.objects['Volume'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Volume'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Volume'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Volume'].modifiers['M'].use_volume_preserve = True
bpy.data.objects['Volume'].modifiers['M'].falloff_radius = 1.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2)
bpy.context.object.name = 'Negative'
bpy.data.objects['Negative'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Negative'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Negative'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Negative'].modifiers['M'].strength = -0.8
bpy.data.objects['Negative'].modifiers['M'].falloff_radius = 1.5
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=10, size=2, location=(0.1, 0, 0))
bpy.context.object.name = 'Group'
bpy.data.objects['Group'].vertex_groups.new(name='G')
bpy.data.objects['Group'].vertex_groups['G'].add([40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54], 1.0, 'REPLACE')
bpy.data.objects['Group'].vertex_groups['G'].add([55, 56, 57, 58, 59, 60, 61, 62, 63, 64], 0.4, 'REPLACE')
bpy.data.objects['Group'].modifiers.new(name='M', type='WARP')
bpy.data.objects['Group'].modifiers['M'].object_from = bpy.data.objects['From']
bpy.data.objects['Group'].modifiers['M'].object_to = bpy.data.objects['To']
bpy.data.objects['Group'].modifiers['M'].vertex_group = 'G'
bpy.data.objects['Group'].modifiers['M'].falloff_radius = 2
