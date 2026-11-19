# WP 4.3: SIMPLE_DEFORM BEND about Z between 10 % and 90 % of the bounds, on a grid.
# golden: startup=empty compare=evaluated:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=10, y_subdivisions=4, size=2)
bpy.data.objects['Grid'].modifiers.new(name='M', type='SIMPLE_DEFORM')
bpy.data.objects['Grid'].modifiers['M'].deform_method = 'BEND'
bpy.data.objects['Grid'].modifiers['M'].deform_axis = 'Z'
bpy.data.objects['Grid'].modifiers['M'].angle = 1.5
bpy.data.objects['Grid'].modifiers['M'].limits = (0.1, 0.9)
