# WP 8.11: SUBSURF UV maps without the limit surface (uniform refinement to `levels`, bilinear
# face-varying patches over the last level's values, Blender's evaluator skipping the coarse
# values), with semi-sharp creases (fractional face-varying weights), and with
# PRESERVE_CORNERS boundaries.
# golden: startup=empty compare=evaluated:Suzanne,evaluated:Cube,evaluated:Cube.001,evaluated:Grid tier=T1 tol=1e-4
import bpy
bpy.ops.mesh.primitive_monkey_add()
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_grid_add(x_subdivisions=4, y_subdivisions=3, location=(9, 0, 0))
bpy.data.objects['Suzanne'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Suzanne'].modifiers['Subdivision'].use_limit_surface = False
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube'].modifiers['Subdivision'].use_limit_surface = False
bpy.data.meshes['Cube.001'].attributes.new(name='crease_edge', type='FLOAT', domain='EDGE')
bpy.data.meshes['Cube.001'].attributes['crease_edge'].data[0].value = 0.5
bpy.data.meshes['Cube.001'].attributes['crease_edge'].data[2].value = 0.3
bpy.data.meshes['Cube.001'].attributes['crease_edge'].data[5].value = 0.4
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Grid'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Grid'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Grid'].modifiers['Subdivision'].boundary_smooth = 'PRESERVE_CORNERS'
bpy.data.objects['Grid'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS'
