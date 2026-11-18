# WP 2.7: mesh.dissolve_limited merges a flat grid into one face and leaves the cube alone
# (5 degree limit); a bent grid keeps its fold. Exact order (the
# BLI_heap port pops ties in Blender's order).
# golden: startup=factory compare=objects,mesh:Grid,mesh:Cube,mesh:Grid.001 tier=T1
import bpy
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(3, 0, 0))
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited(angle_limit=0.1)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=4, location=(6, 0, 0))
bpy.data.meshes['Grid.001'].vertices[0].co = (-1, -1, 0.3)
bpy.data.meshes['Grid.001'].vertices[1].co = (-0.5, -1, 0.3)
bpy.data.meshes['Grid.001'].vertices[2].co = (0, -1, 0.3)
bpy.data.meshes['Grid.001'].vertices[3].co = (0.5, -1, 0.3)
bpy.data.meshes['Grid.001'].vertices[4].co = (1, -1, 0.3)
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.dissolve_limited(angle_limit=0.2, use_dissolve_boundaries=True)
bpy.ops.object.mode_set(mode='OBJECT')
