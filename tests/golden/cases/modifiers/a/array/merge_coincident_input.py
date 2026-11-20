# M5+M6 W0 (M3+M4 leftover): ARRAY `use_merge_vertices` when the input already has exactly
# coincident vertices (a cube mirrored on X without bisect: every vertex has a twin at the
# same position), with and without `use_merge_vertices_cap`, and after a MIRROR that merges.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001,evaluated:Cube.002 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube'].modifiers['Array'].count = 3
bpy.data.objects['Cube'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube.001'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.001'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube.001'].modifiers['Array'].count = 4
bpy.data.objects['Cube.001'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube.001'].modifiers['Array'].use_merge_vertices_cap = True
bpy.data.objects['Cube.001'].modifiers['Array'].relative_offset_displace = (0, 1, 0)
bpy.data.objects['Cube.002'].modifiers.new(name='Mirror', type='MIRROR')
bpy.data.objects['Cube.002'].modifiers['Mirror'].use_axis = (False, False, True)
bpy.data.objects['Cube.002'].modifiers['Mirror'].mirror_offset_v = 0.0
bpy.data.objects['Cube.002'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube.002'].modifiers['Array'].count = 2
bpy.data.objects['Cube.002'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube.002'].modifiers['Array'].merge_threshold = 0.5
