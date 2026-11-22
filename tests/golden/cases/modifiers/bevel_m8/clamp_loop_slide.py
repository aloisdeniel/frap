# WP 8.2: BEVEL clamping with segments (a too large width clamped, and unclamped),
# and loop_slide off on a sheared box (on by default).
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:S,evaluated:T tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].width = 1.5
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.ops.mesh.primitive_cube_add(location=(3, 0, 0))
bpy.data.objects['Cube.001'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube.001'].modifiers['Bevel'].width = 0.6
bpy.data.objects['Cube.001'].modifiers['Bevel'].segments = 2
bpy.data.objects['Cube.001'].modifiers['Bevel'].use_clamp_overlap = False
bpy.data.meshes.new('S').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -0.4, 1), (1, -0.4, 1), (1, 1.6, 1), (-1, 1.6, 1)], [], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects.new('S', bpy.data.meshes['S'])
bpy.context.scene.collection.objects.link(bpy.data.objects['S'])
bpy.data.objects['S'].location = (6, 0, 0)
bpy.data.objects['S'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['S'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['S'].modifiers['Bevel'].width = 0.3
bpy.data.objects['S'].modifiers['Bevel'].segments = 3
bpy.data.meshes.new('T').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -0.4, 1), (1, -0.4, 1), (1, 1.6, 1), (-1, 1.6, 1)], [], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects.new('T', bpy.data.meshes['T'])
bpy.context.scene.collection.objects.link(bpy.data.objects['T'])
bpy.data.objects['T'].location = (9, 0, 0)
bpy.data.objects['T'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['T'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['T'].modifiers['Bevel'].width = 0.3
bpy.data.objects['T'].modifiers['Bevel'].segments = 3
bpy.data.objects['T'].modifiers['Bevel'].loop_slide = False
