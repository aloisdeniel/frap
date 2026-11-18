# WP 2.8: mesh.face_split_by_edges splits selected faces by selected wire edges touching
# their boundary: one edge across an n-gon, then a chain of two wire edges with a branch into
# another face (each net goes to the face it enters); the pieces are selected.
# golden: startup=empty compare=objects,context,mesh:Net tier=T1
import bpy
bpy.data.meshes.new('Net').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (2, 2, 0), (1, 2, 0), (0, 2, 0), (3, 0, 0), (5, 0, 0), (5, 1, 0), (5, 2, 0), (3, 2, 0), (4, 1, 0)], [(1, 4), (8, 11), (11, 10)], [(0, 1, 2, 3, 4, 5), (6, 7, 8, 9, 10)])
bpy.data.objects.new('Net', bpy.data.meshes['Net'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Net'])
bpy.context.view_layer.objects.active = bpy.data.objects['Net']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Net'].total_face_sel
