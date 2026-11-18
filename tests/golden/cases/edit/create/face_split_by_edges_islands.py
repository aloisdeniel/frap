# WP 2.8: mesh.face_split_by_edges with loose wire nets (USE_NET_ISLAND_CONNECT): a floating
# loop inside a face (connected to the boundary by BM_face_split_edgenet_connect_islands), an
# edge dangling from a corner plus a free edge, two islands with an edge leaving a corner
# outwards (its free end is spliced into the closest edge of the closest face, found through
# the BVH), a floating triangle, and a triangle hanging off a chain from the boundary at one
# vertex (partial connection: split off, connected, spliced back).
# golden: startup=empty compare=objects,context,mesh:Isle,mesh:Dangle,mesh:Two,mesh:Part,mesh:Hang tier=T1
import bpy
bpy.data.meshes.new('Isle').from_pydata([(0, 0, 0), (4, 0, 0), (4, 4, 0), (0, 4, 0), (1, 1, 0), (2, 1, 0), (2, 2, 0), (1, 2.5, 0)], [(4, 5), (5, 6), (6, 7), (7, 4)], [(0, 1, 2, 3)])
bpy.data.objects.new('Isle', bpy.data.meshes['Isle'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Isle'])
bpy.context.view_layer.objects.active = bpy.data.objects['Isle']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Isle'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Dangle').from_pydata([(0, 0, 0), (4, 0, 0), (4, 4, 0), (0, 4, 0), (1.5, 1, 0), (3, 3, 0), (3.2, 0.8, 0)], [(0, 4), (5, 6)], [(0, 1, 2, 3)])
bpy.data.objects.new('Dangle', bpy.data.meshes['Dangle'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Dangle'])
bpy.context.view_layer.objects.active = bpy.data.objects['Dangle']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Dangle'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Two').from_pydata([(0, 0, 0), (6, 0, 0), (6, 3, 0), (0, 3, 0), (1, 1, 0), (2, 1, 0), (1.5, 2, 0), (4, 1, 0), (5, 1.2, 0), (4.5, 2, 0), (-1, -1, 0)], [(4, 5), (5, 6), (6, 4), (7, 8), (8, 9), (9, 7), (0, 10)], [(0, 1, 2, 3)])
bpy.data.objects.new('Two', bpy.data.meshes['Two'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Two'])
bpy.context.view_layer.objects.active = bpy.data.objects['Two']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Two'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Part').from_pydata([(0, 0, 0), (4, 0, 0), (4, 4, 0), (0, 4, 0), (2, 0, 0.0), (2, 1.5, 0), (1.5, 2.5, 0), (2.5, 2.5, 0)], [(5, 6), (6, 7), (7, 5)], [(0, 4, 1, 2, 3)])
bpy.data.objects.new('Part', bpy.data.meshes['Part'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Part'])
bpy.context.view_layer.objects.active = bpy.data.objects['Part']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Part'].total_face_sel
bpy.ops.object.mode_set(mode='OBJECT')
bpy.ops.object.select_all(action='DESELECT')
bpy.data.meshes.new('Hang').from_pydata([(0, 0, 0), (4, 0, 0), (4, 4, 0), (0, 4, 0), (2, 0, 0), (2, 1.5, 0), (1.5, 2.5, 0), (2.5, 2.5, 0)], [(4, 5), (5, 6), (6, 7), (7, 5)], [(0, 4, 1, 2, 3)])
bpy.data.objects.new('Hang', bpy.data.meshes['Hang'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Hang'])
bpy.context.view_layer.objects.active = bpy.data.objects['Hang']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.face_split_by_edges()
bpy.data.meshes['Hang'].total_face_sel
