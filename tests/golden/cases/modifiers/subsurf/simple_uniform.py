# WP 4.4: SUBSURF subdivision_type SIMPLE (Blender's all-sharp Catmull-Clark workaround) with and
# without use_limit_surface, and Catmull-Clark without the limit surface (uniform refinement).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].subdivision_type = 'SIMPLE'
bpy.data.objects['Cube.001'].modifiers['Subdivision'].use_limit_surface = False
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 3
bpy.data.objects['Cube.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.002'].modifiers['Subdivision'].use_limit_surface = False
bpy.data.objects['Cube.002'].modifiers['Subdivision'].levels = 2
