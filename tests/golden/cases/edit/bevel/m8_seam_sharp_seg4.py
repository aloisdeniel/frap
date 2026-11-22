# WP 8.2: mark_seam and mark_sharp with four segments on the L prism with seams
# and sharp edges on some beveled edges (marks carried across the vertex meshes).
# golden: startup=factory compare=objects,context,mesh:L tier=T1
import bpy
bpy.data.meshes.new('L').from_pydata([(0, 0, 0), (2, 0, 0), (2, 1, 0), (1, 1, 0), (1, 2, 0), (0, 2, 0), (0, 0, 1), (2, 0, 1), (2, 1, 1), (1, 1, 1), (1, 2, 1), (0, 2, 1)], [], [(5, 4, 3, 2, 1, 0), (6, 7, 8, 9, 10, 11), (0, 1, 7, 6), (1, 2, 8, 7), (2, 3, 9, 8), (3, 4, 10, 9), (4, 5, 11, 10), (5, 0, 6, 11)])
bpy.data.objects['Cube'].data = bpy.data.meshes['L']
bpy.data.meshes['L'].edges[0].use_seam = True
bpy.data.meshes['L'].edges[3].use_seam = True
bpy.data.meshes['L'].edges[5].use_edge_sharp = True
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.15, segments=4, mark_seam=True, mark_sharp=True)
