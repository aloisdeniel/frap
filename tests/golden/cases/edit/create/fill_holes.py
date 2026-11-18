# WP 2.8: mesh.fill_holes fills holes bounded by selected edges when they have at most
# `sides` corners (0: any): a box with a square and a hexagonal hole, filled with sides=4
# (only the square), then sides=0 (the hexagon too); the faces take their neighbours' winding.
# golden: startup=empty compare=objects,context,mesh:Holes tier=T2
import bpy
bpy.data.meshes.new('Holes').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0), (0, 1, 0), (1, 1, 0), (2, 1, 0), (3, 1, 0), (0, 2, 0), (1, 2, 0), (2, 2, 0), (3, 2, 0), (0, 3, 0), (1, 3, 0), (2, 3, 0), (3, 3, 0), (5, 0, 0), (7, 0, 0), (8, 1, 0), (7, 2, 0), (5, 2, 0), (4, 1, 0), (6, -1, 0), (9, 1, 0), (6, 3, 0)], [], [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6), (4, 5, 9, 8), (6, 7, 11, 10), (8, 9, 13, 12), (9, 10, 14, 13), (10, 11, 15, 14), (16, 22, 17), (17, 23, 18), (18, 23, 19), (19, 24, 20), (21, 16, 20)])
bpy.data.objects.new('Holes', bpy.data.meshes['Holes'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Holes'])
bpy.context.view_layer.objects.active = bpy.data.objects['Holes']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill_holes(sides=4)
bpy.data.meshes['Holes'].total_face_sel
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.fill_holes(sides=0)
