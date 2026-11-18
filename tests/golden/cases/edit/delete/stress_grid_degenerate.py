# WP 2.7: mesh.dissolve_degenerate with a threshold above the grid spacing: every edge
# collapses (one weld per connected group of short edges).
# golden: startup=empty compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T1
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=1)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_degenerate(threshold=0.25)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=6, y_subdivisions=5, size=1, location=(3, 0, 0))
bpy.data.meshes['Grid.001'].vertices[8].co = (-0.3, -0.3, 0)
bpy.data.meshes['Grid.001'].vertices[9].co = (-0.29, -0.3, 0)
bpy.data.meshes['Grid.001'].vertices[15].co = (-0.3, -0.29999, 0)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_degenerate(threshold=0.02)
