# WP 8.2: BEVEL vmesh_method CUTOFF with three segments (limit NONE), and an
# L-shaped prism with outer ARC and inner ARC miters (spread) and with PATCH, four segments.
# golden: startup=factory compare=evaluated:Cube,evaluated:L,evaluated:M tier=T1
import bpy
bpy.data.objects['Cube'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['Cube'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['Cube'].modifiers['Bevel'].vmesh_method = 'CUTOFF'
bpy.data.objects['Cube'].modifiers['Bevel'].segments = 3
bpy.data.objects['Cube'].modifiers['Bevel'].width = 0.2
bpy.data.meshes.new('L').from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0), (0, 0, 1), (2, 0, 1), (2, 1, 1), (1, 1, 1), (1, 2, 1), (0, 2, 1)], [], [(5, 4, 3, 2, 1, 0), (6, 7, 8, 9, 10, 11), (0, 1, 7, 6), (1, 2, 8, 7), (2, 3, 9, 8), (3, 4, 10, 9), (4, 5, 11, 10), (5, 0, 6, 11)])
bpy.data.objects.new('L', bpy.data.meshes['L'])
bpy.context.scene.collection.objects.link(bpy.data.objects['L'])
bpy.data.objects['L'].location = (3, 0, 0)
bpy.data.objects['L'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['L'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['L'].modifiers['Bevel'].segments = 4
bpy.data.objects['L'].modifiers['Bevel'].width = 0.15
bpy.data.objects['L'].modifiers['Bevel'].miter_outer = 'MITER_ARC'
bpy.data.objects['L'].modifiers['Bevel'].miter_inner = 'MITER_ARC'
bpy.data.objects['L'].modifiers['Bevel'].spread = 0.08
bpy.data.meshes.new('M').from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0), (0, 0, 1), (2, 0, 1), (2, 1, 1), (1, 1, 1), (1, 2, 1), (0, 2, 1)], [], [(5, 4, 3, 2, 1, 0), (6, 7, 8, 9, 10, 11), (0, 1, 7, 6), (1, 2, 8, 7), (2, 3, 9, 8), (3, 4, 10, 9), (4, 5, 11, 10), (5, 0, 6, 11)])
bpy.data.objects.new('M', bpy.data.meshes['M'])
bpy.context.scene.collection.objects.link(bpy.data.objects['M'])
bpy.data.objects['M'].location = (6, 0, 0)
bpy.data.objects['M'].modifiers.new(name='Bevel', type='BEVEL')
bpy.data.objects['M'].modifiers['Bevel'].limit_method = 'NONE'
bpy.data.objects['M'].modifiers['Bevel'].segments = 3
bpy.data.objects['M'].modifiers['Bevel'].width = 0.2
bpy.data.objects['M'].modifiers['Bevel'].miter_outer = 'MITER_PATCH'
