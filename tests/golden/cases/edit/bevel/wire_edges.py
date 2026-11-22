# WP 2.11: a box with wire edges at two corners (one to a free vertex, one between two
# beveled corners): edge bevel ignores wire edges and reattaches them to the closest new
# vertex; a vertex bevel counts them as edges.
# golden: startup=factory compare=objects,context,mesh:Wired,mesh:Wired2 tier=T1
import bpy
bpy.data.meshes.new('Wired').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1), (-2, -2, -2)], [(0, 8), (0, 6)], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.meshes.new('Wired2').from_pydata([(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1), (-2, -2, -2)], [(0, 8)], [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (3, 0, 4, 7)])
bpy.data.objects['Cube'].data = bpy.data.meshes['Wired']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=2)
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.objects['Cube'].data = bpy.data.meshes['Wired2']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.frap.select_elements(type='VERT', indices=[0])
bpy.ops.mesh.bevel(offset=0.3, segments=2, affect='VERTICES')
