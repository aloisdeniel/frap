# WP 4.2: ARRAY offsets and merge: 4 copies with a relative offset of 1 on X plus a constant
# offset on Z; with merge the touching copies weld (the chunk 2→1 mapping is the translated
# 1→0 one), and the UVs of copy c move by c·(offset_u, offset_v). A second object uses a
# relative offset on Y and Z and a merge distance that reaches the next copy.
# golden: startup=factory compare=evaluated:Cube,evaluated:Cube.001 tier=T1
import bpy
bpy.ops.object.duplicate()
bpy.data.objects['Cube'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube'].modifiers['Array'].count = 4
bpy.data.objects['Cube'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube'].modifiers['Array'].offset_u = 0.25
bpy.data.objects['Cube'].modifiers['Array'].offset_v = -0.5
bpy.data.objects['Cube.001'].modifiers.new(name='Array', type='ARRAY')
bpy.data.objects['Cube.001'].modifiers['Array'].count = 3
bpy.data.objects['Cube.001'].modifiers['Array'].relative_offset_displace = (0, 1.05, 0.2)
bpy.data.objects['Cube.001'].modifiers['Array'].use_constant_offset = True
bpy.data.objects['Cube.001'].modifiers['Array'].constant_offset_displace = (0.1, 0, -0.4)
bpy.data.objects['Cube.001'].modifiers['Array'].use_merge_vertices = True
bpy.data.objects['Cube.001'].modifiers['Array'].merge_threshold = 0.2
