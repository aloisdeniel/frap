# WP 2.8: mesh.beautify_fill rotates edges between selected triangles towards better shapes
# (a fan of slivers over a flat strip), then with an angle limit on a folded pair (the edge
# between faces bent more than the limit stays).
# golden: startup=empty compare=objects,context,mesh:Strip tier=T1
import bpy
bpy.data.meshes.new('Strip').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (4, 0, 0), (0, 1, 0), (4, 1, 0), (0, 0, 2), (1, 0, 2), (0.5, 1, 2), (0.5, -1, 3)], [], [(0, 1, 5), (1, 2, 5), (2, 3, 5), (3, 4, 5), (4, 6, 5), (7, 8, 9), (8, 7, 10)])
bpy.data.objects.new('Strip', bpy.data.meshes['Strip'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Strip'])
bpy.context.view_layer.objects.active = bpy.data.objects['Strip']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.beautify_fill()
bpy.data.meshes['Strip'].total_edge_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.beautify_fill(angle_limit=0.5)
