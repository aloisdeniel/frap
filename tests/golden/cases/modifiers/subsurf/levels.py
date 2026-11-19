# WP 4.4: SUBSURF levels 1, 2 and 3 (every level at or below the default quality 3 lies on
# the limit surface exactly as Blender's patches place it).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002 tier=T1
import bpy
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.ops.mesh.primitive_cube_add(location=(6, 0, 0))
bpy.data.objects['Cube'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.001'].modifiers['Subdivision'].levels = 2
bpy.data.objects['Cube.002'].modifiers.new(name='Subdivision', type='SUBSURF')
bpy.data.objects['Cube.002'].modifiers['Subdivision'].levels = 3
