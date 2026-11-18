# WP 2.8: mesh.edge_face_add on loose vertices (no edges) makes one n-gon over the vertex
# cloud sorted around its normal (BM_verts_sort_radial_plane), creating its edges; a chain of
# edges plus one free vertex is closed by two new edges and filled; three loose vertices make
# a triangle; nothing selected answers CANCELLED.
# golden: startup=empty compare=objects,context,mesh:Cloud,mesh:Chain tier=T1
import bpy
bpy.data.meshes.new('Cloud').from_pydata([(0, 0, 0), (2, 0.1, 0), (2.2, 1.9, 0.1), (0.1, 2, 0), (1, -0.8, 0), (3, 3, 0), (4, 3, 0.2), (3.5, 4, 0)], [], [])
bpy.data.objects.new('Cloud', bpy.data.meshes['Cloud'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Cloud'])
bpy.context.view_layer.objects.active = bpy.data.objects['Cloud']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='DESELECT')
bpy.ops.mesh.edge_face_add()
bpy.ops.frap.select_elements(type='VERT', indices=[0, 1, 2, 3, 4])
bpy.ops.mesh.edge_face_add()
bpy.ops.frap.select_elements(type='VERT', indices=[5, 6, 7])
bpy.ops.mesh.edge_face_add()
bpy.ops.object.mode_set(mode='OBJECT')
bpy.data.meshes.new('Chain').from_pydata([(0, 0, 0), (1, 0, 0), (2, 0.5, 0), (2, 1.5, 0), (0.5, 2, 0)], [(0, 1), (1, 2), (2, 3)], [])
bpy.data.objects.new('Chain', bpy.data.meshes['Chain'])
bpy.context.scene.collection.objects.link(bpy.data.objects['Chain'])
bpy.context.view_layer.objects.active = bpy.data.objects['Chain']
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.select_all(action='SELECT')
bpy.ops.mesh.edge_face_add()
