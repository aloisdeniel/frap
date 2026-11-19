# WP 4.4: SUBSURF uv_smooth, every mode on the seamed factory cube UVs at level 2 (linear,
# face-varying corners, junctions, concave corners, boundaries, smooth everywhere).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002,evaluated:Cube.003,evaluated:Cube.004,evaluated:Cube.005 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(9, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(12, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(15, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube'].modifiers['Subdivision'].uv_smooth = 'NONE'
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.001'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS'
bpy.data.objects['Cube.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.002'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.002'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS_AND_JUNCTIONS'
bpy.data.objects['Cube.003'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.003'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.003'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_CORNERS_JUNCTIONS_AND_CONCAVE'
bpy.data.objects['Cube.004'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.004'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.004'].modifiers['Subdivision'].uv_smooth = 'PRESERVE_BOUNDARIES'
bpy.data.objects['Cube.005'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.005'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.005'].modifiers['Subdivision'].uv_smooth = 'SMOOTH_ALL'
