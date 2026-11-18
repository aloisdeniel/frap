# WP 4.2: WELD (ALL) with a threshold above the grid spacing: kd-tree clusters
# (`kdtree_calc_duplicates_fast`, index order), collapsed and split faces, duplicate edges and
# faces, mixed positions and UVs.
# golden: startup=empty compare=evaluated:Grid tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=5, y_subdivisions=4, size=2)
bpy.data.objects['Grid'].modifiers.new(name='Weld', type='WELD')
bpy.data.objects['Grid'].modifiers['Weld'].merge_threshold = 0.6
