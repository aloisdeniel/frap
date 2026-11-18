# WP 4.4: SUBSURF boundary_smooth on open grids: ALL (smooth corners, which Blender's patches
# keep at their isolation-level position) and PRESERVE_CORNERS (sharp corners), and uv_smooth
# SMOOTH_ALL at level 3 (face-varying smooth corners).
# golden: startup=empty compare=evaluated:Grid,evaluated:Grid.001,evaluated:Grid.002 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=2)
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=2, location=(3, 0, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=3, y_subdivisions=2, size=2, location=(6, 0, 0))
bpy.data.objects['Grid'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Grid.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Grid.001'].modifiers['Subdivision'].boundary_smooth = 'PRESERVE_CORNERS'
bpy.data.objects['Grid.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid.002'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Grid.002'].modifiers['Subdivision'].uv_smooth = 'SMOOTH_ALL'
