# WP 8.1: SLIM with importance weights (use_weights, weight_group, weight_factor): a curved
# grid whose raised vertices are in the vertex group "Importance" (weight 1, the others 0)
# gets per-face weights factor^(2 (0.5 - mean)); a second grid with a negative weight_factor
# is unweighted (SLIM uses weights only for a positive factor), and a missing group means no
# weights either.
# golden: startup=empty compare=objects,context,mesh:Grid,mesh:Grid.001 tier=T2 tol=1e-4
import bpy
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2)
bpy.ops.object.vertex_group_add()
bpy.context.object.vertex_groups[0].name = 'Importance'
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 29, 30, 38, 39])
bpy.ops.transform.translate(value=(0, 0, 0.7))
bpy.ops.object.vertex_group_assign()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', use_weights=True, weight_group='Importance', weight_factor=2.0)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.mesh.primitive_grid_add(x_subdivisions=8, y_subdivisions=8, size=2, location=(3, 0, 0))
bpy.ops.object.vertex_group_add()
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[20, 21, 29, 30, 38, 39])
bpy.ops.transform.translate(value=(0, 0, 0.7))
bpy.ops.object.vertex_group_assign()
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', use_weights=True, weight_group='Group', weight_factor=-1.5)
bpy.ops.uv.unwrap(method='MINIMUM_STRETCH', use_weights=True, weight_group='Missing', weight_factor=2.0, iterations=5)
bpy.ops.object.mode_set(mode='OBJECT')
