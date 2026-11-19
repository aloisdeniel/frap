# WP 4.2: WELD (CONNECTED): only vertices joined by an edge shorter than the threshold merge,
# into the lower index, with the running cluster centre (`WeldVertexCluster`).
# golden: startup=empty compare=evaluated:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=3, size=2)
bpy.data.objects['Grid'].modifiers.new(name='Weld', type='WELD')
bpy.data.objects['Grid'].modifiers['Weld'].mode = 'CONNECTED'
bpy.data.objects['Grid'].modifiers['Weld'].merge_threshold = 0.45
