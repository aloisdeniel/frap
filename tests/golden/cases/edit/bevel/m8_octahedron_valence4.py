# WP 8.2: an octahedron (valence-4 corners, no pipes), every edge, three segments.
# golden: startup=factory compare=objects,context,mesh:Oct tier=T1
import bpy
bpy.data.meshes.new('Oct').from_pydata([(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)], [], [(0, 2, 4), (2, 1, 4), (1, 3, 4), (3, 0, 4), (2, 0, 5), (1, 2, 5), (3, 1, 5), (0, 3, 5)])
bpy.data.objects['Cube'].data = bpy.data.meshes['Oct']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.bevel(offset=0.2, segments=3)
