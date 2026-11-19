# WP 4.2: EDGE_SPLIT by angle (default 30°): the cube splits into six quads, the cylinder only
# around its caps; Suzanne with a 20° angle; a split angle of 0 splits every edge with two
# faces, a split angle of 180° (π) none.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cylinder,evaluated:Suzanne,evaluated:Grid,evaluated:Grid.001 tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.ops.mesh.primitive_cylinder_add(vertices=12, location=(3, 0, 0))
bpy.data.objects['Cylinder'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.ops.mesh.primitive_monkey_add(location=(-3, 0, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.data.objects['Suzanne'].modifiers['EdgeSplit'].split_angle = 0.3490658503988659
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, location=(0, 4, 0))
bpy.data.objects['Grid'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.data.objects['Grid'].modifiers['EdgeSplit'].split_angle = 0
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, location=(0, -4, 0))
bpy.data.objects['Grid.001'].modifiers.new(name='EdgeSplit', type='EDGE_SPLIT')
bpy.data.objects['Grid.001'].modifiers['EdgeSplit'].split_angle = 3.141592653589793
